// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::ast::{
    BackReference, CharSet, CharSetElement, Expression, FunctionArgument, FunctionName, Literal,
    PresetCharSetName,
};

/// `table[b]` is true if a match may start with the UTF-8 byte `b`.
pub type FirstByteTable = [bool; 256];

/// Finds the single-character literal `S` of a leading `S+`, `S*`, or `S{n,}`.
///
/// If a match attempt at position `p` fails, every later start inside the same
/// run of `S` characters retries only a subset of the continuations already tried
/// from `p`, so the search may skip to the end of the run.
/// Backreferences can make the continuation depend on where the run started,
/// so patterns containing them are excluded.
pub fn find_leading_run(exp: &Expression) -> Option<&Literal> {
    if contains_backreference(exp) {
        return None;
    }
    leading_run(exp)
}

fn leading_run(exp: &Expression) -> Option<&Literal> {
    match exp {
        Expression::Group(exps) => exps.first().and_then(leading_run),
        Expression::FunctionCall(function_call) => {
            let Some(FunctionArgument::Expression(inner)) = function_call.args.first() else {
                return None;
            };
            match function_call.name {
                FunctionName::Index | FunctionName::Name => leading_run(inner),
                FunctionName::OneOrMore
                | FunctionName::ZeroOrMore
                | FunctionName::RepeatFrom
                | FunctionName::LazyOneOrMore
                | FunctionName::LazyZeroOrMore
                | FunctionName::LazyRepeatFrom => match inner {
                    Expression::Literal(literal @ (Literal::Char(_)
                    | Literal::AnyChar
                    | Literal::CharSet(_)
                    | Literal::PresetCharSet(_))) => Some(literal),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

fn contains_backreference(exp: &Expression) -> bool {
    match exp {
        Expression::BackReference(BackReference::Index(_) | BackReference::Name(_)) => true,
        Expression::Group(exps) => exps.iter().any(contains_backreference),
        Expression::Or(left, right) => contains_backreference(left) || contains_backreference(right),
        Expression::FunctionCall(function_call) => function_call.args.iter().any(|arg| {
            matches!(arg, FunctionArgument::Expression(exp) if contains_backreference(exp))
        }),
        Expression::Literal(_) => false,
    }
}

/// Calculates the bytes that can begin a match, so the search can skip
/// positions where no match is possible.
///
/// Returns `None` if the expression may match empty text or its first
/// character cannot be determined.
pub fn calculate_first_bytes(exp: &Expression) -> Option<FirstByteTable> {
    match first_bytes(exp) {
        FirstBytes::Bytes(table) => Some(*table),
        FirstBytes::ZeroWidth | FirstBytes::Unknown => None,
    }
}

enum FirstBytes {
    // The expression always consumes a first character from this table.
    Bytes(Box<FirstByteTable>),
    // The expression always succeeds or fails without consuming input,
    // so the next expression in a sequence determines the first character.
    ZeroWidth,
    // The expression may match empty text, or the analysis is too complex.
    Unknown,
}

fn first_bytes(exp: &Expression) -> FirstBytes {
    match exp {
        Expression::Literal(literal) => first_bytes_of_literal(literal),
        Expression::BackReference(_) => FirstBytes::Unknown,
        Expression::Group(exps) => {
            for exp in exps {
                match first_bytes(exp) {
                    FirstBytes::ZeroWidth => continue,
                    other => return other,
                }
            }
            FirstBytes::ZeroWidth
        }
        Expression::Or(left, right) => match (first_bytes(left), first_bytes(right)) {
            (FirstBytes::Bytes(mut table), FirstBytes::Bytes(other)) => {
                for (byte, possible) in table.iter_mut().enumerate() {
                    *possible |= other[byte];
                }
                FirstBytes::Bytes(table)
            }
            _ => FirstBytes::Unknown,
        },
        Expression::FunctionCall(function_call) => {
            let inner = || match &function_call.args[0] {
                FunctionArgument::Expression(exp) => first_bytes(exp),
                _ => unreachable!(),
            };
            let min_count = match function_call.args.get(1) {
                Some(FunctionArgument::Number(n)) => *n,
                _ => 1,
            };

            match function_call.name {
                FunctionName::IsStart
                | FunctionName::IsEnd
                | FunctionName::IsBound
                | FunctionName::IsNotBound => FirstBytes::ZeroWidth,
                FunctionName::Optional
                | FunctionName::ZeroOrMore
                | FunctionName::LazyOptional
                | FunctionName::LazyZeroOrMore => FirstBytes::Unknown,
                FunctionName::Repeat
                | FunctionName::RepeatFrom
                | FunctionName::RepeatRange
                | FunctionName::LazyRepeat
                | FunctionName::LazyRepeatFrom
                | FunctionName::LazyRepeatRange
                    if min_count == 0 =>
                {
                    FirstBytes::Unknown
                }
                // Lookaround matches its first argument; the asserted part is zero-width.
                _ => inner(),
            }
        }
    }
}

fn first_bytes_of_literal(literal: &Literal) -> FirstBytes {
    let mut table = [false; 256];
    match literal {
        Literal::Char(c) => add_char(&mut table, *c),
        Literal::String(s) => match s.chars().next() {
            Some(c) => add_char(&mut table, c),
            None => return FirstBytes::ZeroWidth,
        },
        Literal::AnyChar => {
            add_all_chars(&mut table);
            table[b'\n' as usize] = false;
            table[b'\r' as usize] = false;
        }
        Literal::PresetCharSet(name) => add_preset_charset(&mut table, name),
        Literal::CharSet(charset) => add_charset(&mut table, charset),
    }
    FirstBytes::Bytes(Box::new(table))
}

fn add_charset(table: &mut FirstByteTable, charset: &CharSet) {
    let mut positive = [false; 256];
    for element in &charset.elements {
        match element {
            CharSetElement::Char(c) => add_char(&mut positive, *c),
            CharSetElement::CharRange(range) => {
                add_range(&mut positive, range.start, range.end_inclusive)
            }
            CharSetElement::PresetCharSet(name) => add_preset_charset(&mut positive, name),
            CharSetElement::CharSet(nested) => add_charset(&mut positive, nested),
        }
    }

    if charset.negative {
        add_negated(table, &positive);
    } else {
        for (byte, possible) in table.iter_mut().enumerate() {
            *possible |= positive[byte];
        }
    }
}

fn add_preset_charset(table: &mut FirstByteTable, name: &PresetCharSetName) {
    let mut positive = [false; 256];
    let negative = match name {
        PresetCharSetName::CharWord | PresetCharSetName::CharNotWord => {
            add_range(&mut positive, 'A', 'Z');
            add_range(&mut positive, 'a', 'z');
            add_range(&mut positive, '0', '9');
            add_char(&mut positive, '_');
            *name == PresetCharSetName::CharNotWord
        }
        PresetCharSetName::CharDigit | PresetCharSetName::CharNotDigit => {
            add_range(&mut positive, '0', '9');
            *name == PresetCharSetName::CharNotDigit
        }
        PresetCharSetName::CharSpace | PresetCharSetName::CharNotSpace => {
            for c in [' ', '\t', '\r', '\n'] {
                add_char(&mut positive, c);
            }
            *name == PresetCharSetName::CharNotSpace
        }
    };

    if negative {
        add_negated(table, &positive);
    } else {
        for (byte, possible) in table.iter_mut().enumerate() {
            *possible |= positive[byte];
        }
    }
}

// Non-ASCII entries are conservative lead bytes, so a negated set can only exclude ASCII bytes.
fn add_negated(table: &mut FirstByteTable, positive: &FirstByteTable) {
    for byte in 0..0x80 {
        table[byte] |= !positive[byte];
    }
    for possible in &mut table[0xC2..=0xF4] {
        *possible = true;
    }
}

fn add_all_chars(table: &mut FirstByteTable) {
    add_range(table, '\0', char::MAX);
}

fn add_char(table: &mut FirstByteTable, c: char) {
    table[lead_byte(c) as usize] = true;
}

fn add_range(table: &mut FirstByteTable, start: char, end_inclusive: char) {
    // UTF-8 lead bytes increase monotonically with the code point.
    for byte in lead_byte(start)..=lead_byte(end_inclusive) {
        if !(0x80..0xC2).contains(&byte) {
            table[byte as usize] = true;
        }
    }
}

fn lead_byte(c: char) -> u8 {
    let mut buffer = [0; 4];
    c.encode_utf8(&mut buffer);
    buffer[0]
}

#[cfg(test)]
mod tests {
    use crate::{anre, traditional};

    use super::{calculate_first_bytes, find_leading_run};

    fn first_bytes_of(regex: &str) -> Option<Vec<u8>> {
        let program = traditional::parse_from_str(regex).unwrap();
        calculate_first_bytes(&program.expression)
            .map(|table| (0..=255u8).filter(|b| table[*b as usize]).collect())
    }

    #[test]
    fn test_first_bytes() {
        assert_eq!(first_bytes_of("abc"), Some(vec![b'a']));
        assert_eq!(first_bytes_of("a|b"), Some(vec![b'a', b'b']));
        assert_eq!(first_bytes_of(r"\b[xy]+"), Some(vec![b'x', b'y']));
        assert_eq!(first_bytes_of(r"(?:\d\.){3}"), Some(b"0123456789".to_vec()));
        assert_eq!(first_bytes_of("(?<=a)b"), Some(vec![b'b']));
        assert_eq!(first_bytes_of("文"), Some(vec![0xE6]));

        // Lead bytes for U+4E00..=U+9FA5 are 0xE4..=0xE9.
        assert_eq!(first_bytes_of("[一-龥]"), Some((0xE4..=0xE9).collect()));

        let negated = first_bytes_of("[^>]").unwrap();
        assert!(!negated.contains(&b'>'));
        assert!(negated.contains(&b'a') && negated.contains(&0xE4));
        assert!(!negated.iter().any(|b| (0x80..0xC2).contains(b)));
    }

    #[test]
    fn test_first_bytes_unknown() {
        for regex in ["a?b", "a*", r"\b", "a{0,2}b"] {
            assert_eq!(first_bytes_of(regex), None, "{}", regex);
        }

        let program = anre::parse_from_str("\"\"").unwrap();
        assert!(calculate_first_bytes(&program.expression).is_none());
    }

    #[test]
    fn test_leading_run() {
        let has_leading_run = |regex: &str| {
            let program = traditional::parse_from_str(regex).unwrap();
            find_leading_run(&program.expression).is_some()
        };

        for regex in ["[a-z]+ing", r"\w*@", "(a+)b", "x{2,}y", "[a-z]+?q"] {
            assert!(has_leading_run(regex), "{}", regex);
        }

        for regex in [r"\b[a-z]+", "a{1,3}b", "a+b|c", r"(a+)b\1", "(?:ab)+c", "(?<=x)a+"] {
            assert!(!has_leading_run(regex), "{}", regex);
        }
    }
}
