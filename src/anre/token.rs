// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::range::Range;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Reserved words in ANRE. Currently `define` and `as`.
    Keyword(String),

    // User-defined or built-in names.
    //
    // The lexer accepts ASCII word characters plus Unicode scalar values outside
    // the surrogate range. Identifiers are used for function names, macro names,
    // preset charset names, named captures, and backreferences.
    Identifier(String),

    // Decimal integer literal used by repetition counts and numeric arguments.
    Number(usize),

    // Character literal, for example `'a'`.
    Char(char),

    // String literal, for example `"abc"`.
    String(String),

    // Greedy zero-or-one quantifier postfix `?`.
    Optional,

    // Greedy one-or-more quantifier postfix `+`.
    OneOrMore,

    // Greedy zero-or-more quantifier postfix `*`.
    ZeroOrMore,

    // Lazy zero-or-one quantifier postfix `??`.
    LazyOptional,

    // Lazy one-or-more quantifier postfix `+?`.
    LazyOneOrMore,

    // Lazy zero-or-more quantifier postfix `*?`.
    LazyZeroOrMore,

    // Opens a repetition specifier such as `{3}`, `{3..}`, or `{3..5}`.
    BraceOpen,

    // Closes a repetition specifier.
    BraceClose,

    // Negates a character set when it appears immediately before `[`, as in `![...]`.
    Not,

    // Shared `..` operator used in char ranges and repetition ranges.
    Range,

    // Prefix operator for index capture such as `#(...)`.
    Hash,

    // Numeric backreference prefix such as `^1`, `^2`, etc.
    Caret,

    // Method-call separator in expression chains such as `'a'.optional()`.
    Dot,

    // Alternation operator `||`.
    LogicOr,

    // Opens a character set literal.
    BracketOpen,

    // Closes a character set literal.
    BracketClose,

    // Opens a group or function-call argument list.
    ParenthesisOpen,

    // Closes a group or function-call argument list.
    ParenthesisClose,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TokenWithRange {
    // The token produced by the lexer.
    pub token: Token,
    // Source range covering the original lexeme.
    pub range: Range,
}

impl TokenWithRange {
    pub fn new(token: Token, range: Range) -> Self {
        Self { token, range }
    }
}
