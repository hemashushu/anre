// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{
    anre::macro_expander::expand,
    ast::{
        BackReference, CharRange, CharSet, CharSetElement, Expression, FunctionArgument,
        FunctionCall, FunctionName, Literal, PresetCharSetName, Program,
    },
    error::AnreError,
    peekable_iter::PeekableIter,
    range::Range,
};

use super::{
    lexer::lex_from_str,
    token::{Token, TokenWithRange},
};

pub fn parse_from_str(s: &str) -> Result<Program, AnreError> {
    let tokens = lex_from_str(s)?;
    let expanded_tokens = expand(tokens)?;
    let mut token_iter = expanded_tokens.into_iter();
    let mut peekable_token_iter = PeekableIter::new(&mut token_iter);
    let mut parser = Parser::new(&mut peekable_token_iter);
    parser.parse_program()
}

pub struct Parser<'a> {
    upstream: &'a mut PeekableIter<'a, TokenWithRange>,

    /// Range of the most recently consumed token.
    pub last_range: Range,
}

impl<'a> Parser<'a> {
    fn new(upstream: &'a mut PeekableIter<'a, TokenWithRange>) -> Self {
        Self {
            upstream,
            last_range: Range::default(),
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        match self.next_token_with_range() {
            Some(TokenWithRange { token, range }) => {
                self.last_range = range;
                Some(token)
            }
            None => None,
        }
    }

    fn next_token_with_range(&mut self) -> Option<TokenWithRange> {
        match self.upstream.next() {
            Some(token_with_range) => {
                self.last_range = token_with_range.range;
                Some(token_with_range)
            }
            None => None,
        }
    }

    fn peek_token(&self, offset: usize) -> Option<&Token> {
        match self.upstream.peek(offset) {
            Some(TokenWithRange { token, .. }) => Some(token),
            None => None,
        }
    }

    fn peek_token_with_range(&self, offset: usize) -> Option<&TokenWithRange> {
        self.upstream.peek(offset)
    }

    fn peek_range(&self, offset: usize) -> Option<&Range> {
        match self.upstream.peek(offset) {
            Some(TokenWithRange { range, .. }) => Some(range),
            None => None,
        }
    }

    // Returns `true` when the token at `offset` matches `expected_token`.
    fn peek_token_and_equals(&self, offset: usize, expected_token: &Token) -> bool {
        matches!(
            self.peek_token(offset),
            Some(token) if token == expected_token)
    }

    fn consume_identifier(&mut self) -> Result<String, AnreError> {
        match self.next_token() {
            Some(Token::Identifier(id)) => Ok(id),
            Some(_) => Err(AnreError::MessageWithPosition(
                "Expect an identifier.".to_owned(),
                self.last_range.start,
            )),
            None => Err(AnreError::UnexpectedEndOfDocument(
                "Expect an identifier.".to_owned(),
            )),
        }
    }

    /// Consumes the next token and requires it to be the exact identifier.
    fn consume_identifier_and_assert(&mut self, identifier: &str) -> Result<String, AnreError> {
        match self.next_token() {
            Some(Token::Identifier(id)) if id == identifier => Ok(id),
            Some(_) => Err(AnreError::MessageWithPosition(
                format!("Expect identifier \"{}\".", identifier),
                self.last_range.start,
            )),
            None => Err(AnreError::UnexpectedEndOfDocument(format!(
                "Expect identifier \"{}\".",
                identifier
            ))),
        }
    }

    fn consume_char(&mut self) -> Result<char, AnreError> {
        match self.next_token() {
            Some(Token::Char(c)) => Ok(c),
            Some(_) => Err(AnreError::MessageWithPosition(
                "Expect a character.".to_owned(),
                self.last_range.start,
            )),
            None => Err(AnreError::UnexpectedEndOfDocument(
                "Expect a character.".to_owned(),
            )),
        }
    }

    fn consume_string(&mut self) -> Result<String, AnreError> {
        match self.next_token() {
            Some(Token::String(s)) => Ok(s),
            Some(_) => Err(AnreError::MessageWithPosition(
                "Expect a string.".to_owned(),
                self.last_range.start,
            )),
            None => Err(AnreError::UnexpectedEndOfDocument(
                "Expect a string.".to_owned(),
            )),
        }
    }

    fn consume_number(&mut self) -> Result<usize, AnreError> {
        match self.next_token() {
            Some(Token::Number(i)) => Ok(i),
            Some(_) => Err(AnreError::MessageWithPosition(
                "Expect a number.".to_owned(),
                self.last_range.start,
            )),
            None => Err(AnreError::UnexpectedEndOfDocument(
                "Expect a number.".to_owned(),
            )),
        }
    }

    // Consumes one token and requires it to match `expected_token`.
    fn consume_token_and_assert(
        &mut self,
        expected_token: &Token,
        token_description: &str,
    ) -> Result<(), AnreError> {
        match self.next_token() {
            Some(token) => {
                if &token == expected_token {
                    Ok(())
                } else {
                    Err(AnreError::MessageWithRange(
                        format!("Expect token: {}.", token_description),
                        self.last_range,
                    ))
                }
            }
            None => Err(AnreError::UnexpectedEndOfDocument(format!(
                "Expect token: {}.",
                token_description
            ))),
        }
    }

    // Consumes `(`.
    fn consume_opening_parenthesis(&mut self) -> Result<(), AnreError> {
        self.consume_token_and_assert(&Token::ParenthesisOpen, "opening parenthesis")
    }

    // Consumes `)`.
    fn consume_closing_parenthesis(&mut self) -> Result<(), AnreError> {
        self.consume_token_and_assert(&Token::ParenthesisClose, "closing parenthesis")
    }

    // Consumes `]`.
    fn consume_closing_bracket(&mut self) -> Result<(), AnreError> {
        self.consume_token_and_assert(&Token::BracketClose, "closing bracket")
    }

    // Consumes `}`.
    fn consume_closing_brace(&mut self) -> Result<(), AnreError> {
        self.consume_token_and_assert(&Token::BraceClose, "closing brace")
    }
}

impl Parser<'_> {
    pub fn parse_program(&mut self) -> Result<Program, AnreError> {
        let expression = self.parse_expression()?;

        if self.peek_token(0).is_some() {
            return Err(AnreError::MessageWithRange(
                "Only one expression is allowed at ANRE root, consider wrapping multiple expressions in a group.".to_owned(),
                *self.peek_range(0).unwrap()
            ));
        }

        Ok(Program { expression })
    }

    fn parse_expression(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // token ...
        // -----
        // ^
        // |__ current, None or Some(...)
        // ```

        // Parsing proceeds from the lowest-precedence form to the highest.
        // Each layer delegates to the next tighter layer and then folds its own
        // operator on top.
        //
        // Lower precedence, parsed later:
        // 1. binary expressions (logic or)
        // 2. name capture
        // 3. index capture
        // 4. method call
        // 5. quantifier
        // 6. primary expressions (literal, group, identifier, function call)
        // Higher precedence, parsed first.

        self.parse_logic_or()
    }

    fn parse_logic_or(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // expression || expression
        // ```

        let mut left = self.parse_named_capture()?;

        while self.peek_token_and_equals(0, &Token::LogicOr) {
            self.next_token(); // consume "||"

            // Operator associativity:
            //
            // - https://en.wikipedia.org/wiki/Operator_associativity
            // - https://en.wikipedia.org/wiki/Operators_in_C_and_C%2B%2B#Operator_precedence
            //
            // Representation:
            // - left-associative (left-to-right associative)
            //   `a || b || c -> (a || b) || c`
            // - right-associative (right-to-left associative)
            //   `a || b || c -> a || (b || c)`
            //
            // call `parse_expression` for right-to-left associative, e.g.
            // `let right = self.parse_expression()?;`
            // or call `parse_named_capture` for left-to-right associative, e.g.
            // `let right = self.parse_named_capture()?;`
            //
            // currently right-associative is adopted for efficiency.

            let right = self.parse_expression()?;
            let expression = Expression::Or(Box::new(left), Box::new(right));
            left = expression;
        }

        Ok(left)
    }

    fn parse_named_capture(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // expression as identifier
        // ```

        let expression = self.parse_index_capture()?;
        if self.peek_token_and_equals(0, &Token::Keyword("as".to_owned())) {
            self.next_token(); // consume "as"

            let name = self.consume_identifier()?;

            if let Expression::IndexCapture(exp) = expression {
                // Naming an index capture should not produce `Name(Index(expr))`.
                // Name capture already implies index capture semantics.
                Ok(Expression::NameCapture(name, exp))
            } else {
                // Otherwise, we wrap the expression in a new name capture.
                Ok(Expression::NameCapture(name, Box::new(expression)))
            }
        } else {
            Ok(expression)
        }
    }

    fn parse_index_capture(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // # expression
        // ```

        if self.peek_token_and_equals(0, &Token::Hash) {
            self.next_token(); // consume '#'

            let expression = self.parse_method_call()?;

            if matches!(expression, Expression::NameCapture(_, _)) {
                // Name capture already records both the span and the name, so an
                // extra index-capture wrapper would be redundant.
                Ok(expression)
            } else {
                Ok(Expression::IndexCapture(Box::new(expression)))
            }
        } else {
            self.parse_method_call()
        }
    }

    fn parse_method_call(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // expression.identifier(arguments)
        // ```

        let mut expression = self.parse_quantifier()?;

        while self.peek_token_and_equals(0, &Token::Dot)
            && matches!(self.peek_token(1), Some(Token::Identifier(_)))
            && matches!(self.peek_token(2), Some(Token::ParenthesisOpen))
        {
            let function_call = self.continue_parse_method_call(expression)?;
            expression = Expression::FunctionCall(Box::new(function_call));
        }

        Ok(expression)
    }

    fn continue_parse_method_call(
        &mut self,
        expression: Expression,
    ) -> Result<FunctionCall, AnreError> {
        // ```diagram
        // "." identifier "(" {args} ")" ?
        // --- ---------- ---            -
        // ^   ^          ^__ validated  ^__ to here
        // |   |__ validated
        // |__ current, validated
        // ```

        self.next_token(); // consume '.'

        let identifier = self.consume_identifier()?; // consume function name
        let name: FunctionName = identifier.as_str().try_into().map_err(|_| {
            AnreError::MessageWithRange(
                format!("Unsupported function \"{}\"", identifier),
                self.last_range,
            )
        })?;

        self.next_token(); // consume '('

        let mut args = vec![];
        args.push(FunctionArgument::Expression(expression));

        while let Some(token) = self.peek_token(0) {
            if token == &Token::ParenthesisClose {
                break;
            }

            if matches!(self.peek_token(0), Some(Token::Number(_))) {
                let number = self.consume_number()?;
                args.push(FunctionArgument::Number(number));
            } else {
                let expression = self.parse_expression()?;
                args.push(FunctionArgument::Expression(expression));
            }
        }

        self.consume_closing_parenthesis()?; // consume ')'

        let function_call = FunctionCall { name, args };

        Ok(function_call)
    }

    fn parse_quantifier(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // expression [ "?" | "+" | "*" | "{N}" | "{N..}" | "{N..M}" ]
        // expression [ "??" | "+?" | "*?" | "{N..}?" | "{N..M}?" ]
        // ```

        let mut expression = self.parse_primary_expression()?;

        if let Some(token) = self.peek_token(0) {
            match token {
                Token::Question
                | Token::Plus
                | Token::Asterisk
                | Token::QuestionLazy
                | Token::PlusLazy
                | Token::AsteriskLazy => {
                    let name = match token {
                        // Greedy quantifier
                        Token::Question => FunctionName::Optional,
                        Token::Plus => FunctionName::OneOrMore,
                        Token::Asterisk => FunctionName::ZeroOrMore,
                        // Lazy quantifier
                        Token::QuestionLazy => FunctionName::OptionalLazy,
                        Token::PlusLazy => FunctionName::OneOrMoreLazy,
                        Token::AsteriskLazy => FunctionName::ZeroOrMoreLazy,
                        _ => unreachable!(),
                    };

                    let function_call = FunctionCall {
                        name,
                        args: vec![FunctionArgument::Expression(expression)],
                    };
                    expression = Expression::FunctionCall(Box::new(function_call));

                    self.next_token(); // consume notation
                }
                Token::BraceOpen => {
                    let (repetition, lazy) = self.continue_parse_repetition()?;

                    let mut args = vec![];
                    args.push(FunctionArgument::Expression(expression));

                    let name = match repetition {
                        Repetition::Repeat(n) => {
                            if lazy {
                                return Err(AnreError::MessageWithRange(
                                    format!(
                                        "Specified repetition does not support lazy mode, \"{{{}}}?\" is not allowed.",
                                        n
                                    ),
                                    self.last_range,
                                ));
                            }

                            args.push(FunctionArgument::Number(n));
                            FunctionName::Repeat
                        }
                        Repetition::RepeatFrom(n) => {
                            args.push(FunctionArgument::Number(n));

                            if lazy {
                                FunctionName::RepeatFromLazy
                            } else {
                                FunctionName::RepeatFrom
                            }
                        }
                        Repetition::RepeatRange(m, n) => {
                            // `{m..m}` is equivalent to a fixed repetition, so it reuses
                            // the same AST form as `{m}`.
                            if m == n {
                                if lazy {
                                    return Err(AnreError::MessageWithRange(
                                        format!(
                                            "Specified repetition does not support lazy mode, \"{{{},{}}}?\" is not allowed.",
                                            m, n
                                        ),
                                        self.last_range,
                                    ));
                                }

                                args.push(FunctionArgument::Number(n));
                                FunctionName::Repeat
                            } else {
                                args.push(FunctionArgument::Number(m));
                                args.push(FunctionArgument::Number(n));

                                if lazy {
                                    FunctionName::RepeatRangeLazy
                                } else {
                                    FunctionName::RepeatRange
                                }
                            }
                        }
                    };

                    let function_call = FunctionCall { name, args };
                    expression = Expression::FunctionCall(Box::new(function_call));
                }
                _ => {
                    // not a quantifier, do nothing
                }
            }
        }

        Ok(expression)
    }

    fn continue_parse_repetition(&mut self) -> Result<(Repetition, /* is_lazy */ bool), AnreError> {
        // ```diagram
        // {m..n}? ?
        // -       -
        // ^       ^__ to here
        // | current, validated
        // ```

        self.next_token(); // consume '{'

        let from = self.consume_number()?;

        let repetition = if self.peek_token_and_equals(0, &Token::Range) {
            // Example:
            // - `{m..}`
            // - `{m..n}`

            self.next_token(); // consume '..'

            if let Some(Token::Number(v)) = self.peek_token(0) {
                let to = *v;
                self.next_token(); // consume number
                Repetition::RepeatRange(from, to)
            } else {
                Repetition::RepeatFrom(from)
            }
        } else {
            // Example:
            // `{m}`
            Repetition::Repeat(from)
        };

        self.consume_closing_brace()?; // consume '}'

        let lazy = if self.peek_token_and_equals(0, &Token::Question) {
            self.next_token(); // consume trailing '?'
            true
        } else {
            false
        };

        Ok((repetition, lazy))
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, AnreError> {
        // primary expressions:
        // - literal
        // - group
        // - identifier (for named backreference)
        // - indexed backreference
        // - function call

        let expression = match self.peek_token(0) {
            Some(token) => {
                match token {
                    Token::ParenthesisOpen => {
                        // group
                        self.parse_group()?
                    }
                    Token::Identifier(_)
                        if self.peek_token_and_equals(1, &Token::ParenthesisOpen) =>
                    {
                        // function call
                        self.parse_function_call()?
                    }
                    Token::Identifier(id)
                        if id != "char_any"
                            && PresetCharSetName::try_from(id.as_str()).is_err() =>
                    {
                        // A bare identifier that is neither a function call nor a
                        // literal name is treated as a named backreference.
                        let name = id.to_owned();
                        self.next_token(); // consume identifier
                        Expression::BackReference(BackReference::Name(name))
                    }
                    Token::Caret if matches!(self.peek_token(1), Some(Token::Number(_))) => {
                        // numeric backreference, e.g. `^1`, `^2`, etc.
                        self.next_token(); // consume '^'
                        let index = self.consume_number()?;
                        Expression::BackReference(BackReference::Index(index))
                    }
                    _ => {
                        let literal = self.parse_literal()?;
                        Expression::Literal(literal)
                    }
                }
            }
            None => {
                return Err(AnreError::UnexpectedEndOfDocument(
                    "Expect an expression.".to_owned(),
                ));
            }
        };

        Ok(expression)
    }

    fn parse_group(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // (expression, ...) ?
        // -                 -
        // ^                 ^__ to here
        // |__ current, validated
        // ```

        self.next_token(); // consume "("
        let mut expressions: Vec<Expression> = vec![];

        while let Some(token) = self.peek_token(0) {
            if token == &Token::ParenthesisClose {
                break;
            }

            let expression = self.parse_expression()?;
            expressions.push(expression);
        }

        self.consume_closing_parenthesis()?; // consume ")"

        // Collapse single-element groups. This keeps macro expansion from
        // introducing extra group nodes that do not affect semantics.
        //
        // - `(foo)` -> `foo`
        // - `((foo, bar))` -> `(foo, bar)`
        if expressions.len() == 1 {
            let expression = expressions.remove(0);
            Ok(expression)
        } else {
            Ok(Expression::Group(expressions))
        }
    }

    fn parse_function_call(&mut self) -> Result<Expression, AnreError> {
        // ```diagram
        // identifier ( args... ) ?
        // ---------- -           -
        // ^          ^           ^__ to here
        // |          |__ validated
        // |__ current, validated
        // ```

        let identifier = self.consume_identifier()?;

        let name = identifier.as_str().try_into().map_err(|_| {
            AnreError::MessageWithRange(
                format!("Unsupported function \"{}\"", identifier),
                self.last_range,
            )
        })?;

        self.consume_opening_parenthesis()?; // consume '('

        let mut args = vec![];

        while let Some(token) = self.peek_token(0) {
            if token == &Token::ParenthesisClose {
                break;
            }

            if matches!(self.peek_token(0), Some(Token::Number(_))) {
                let number = self.consume_number()?;
                args.push(FunctionArgument::Number(number));
            } else {
                let expression = self.parse_expression()?;
                args.push(FunctionArgument::Expression(expression));
            }
        }

        self.consume_closing_parenthesis()?; // consume ')'

        let function_call = FunctionCall { name, args };

        Ok(Expression::FunctionCall(Box::new(function_call)))
    }

    fn parse_literal(&mut self) -> Result<Literal, AnreError> {
        // literals:
        // - `char_any` (any character)
        // - char
        // - string
        // - charset
        // - preset charset

        let literal = match self.peek_token(0).unwrap() {
            Token::BracketOpen => {
                // charset
                let elements = self.parse_charset_elements()?;
                Literal::CharSet(CharSet {
                    negative: false,
                    elements,
                })
            }
            Token::Exclamation if self.peek_token_and_equals(1, &Token::BracketOpen) => {
                // negative charset
                self.next_token();

                let elements = self.parse_charset_elements()?;
                Literal::CharSet(CharSet {
                    negative: true,
                    elements,
                })
            }
            Token::Char(char_ref) => {
                let c = *char_ref;
                self.next_token(); // consume char
                Literal::Char(c)
            }
            Token::String(string_ref) => {
                let string = string_ref.to_owned();
                self.next_token(); // consume string
                Literal::String(string)
            }
            Token::Identifier(id) if id == "char_any" => {
                self.next_token(); // consume "char_any"
                Literal::AnyChar
            }
            Token::Identifier(preset_charset_name_ref) => {
                let preset_charset_name =
                    PresetCharSetName::try_from(preset_charset_name_ref.as_str()).unwrap();
                self.next_token(); // consume preset charset
                Literal::PresetCharSet(preset_charset_name)
            }
            _ => {
                return Err(AnreError::MessageWithRange(
                    "Expect a literal.".to_owned(),
                    self.last_range,
                ));
            }
        };

        Ok(literal)
    }

    fn parse_charset_elements(&mut self) -> Result<Vec<CharSetElement>, AnreError> {
        // ```diagram
        // [ {char | char_range | preset_charset | char_set} ] ?
        // -                                                   -
        // ^                                                   ^__ to here
        // |__ current, validated
        // ```

        self.next_token(); // consume '['

        let mut elements = vec![];
        while let Some(token) = self.peek_token(0) {
            if token == &Token::BracketClose {
                break;
            }

            let start_range = *self.peek_range(0).unwrap();
            let expression = self.parse_expression()?;

            match expression {
                Expression::Literal(Literal::Char(c)) => {
                    // char
                    if self.peek_token_and_equals(0, &Token::Range) {
                        // char range, e.g. `['a'..'z']`
                        self.next_token(); // consume '..'

                        if self.peek_token(0).is_none() {
                            return Err(AnreError::UnexpectedEndOfDocument(
                                "Expect a char literal after '..' in char range.".to_owned(),
                            ));
                        }

                        let end_range = *self.peek_range(0).unwrap();
                        let end_expression = self.parse_expression()?;

                        if let Expression::Literal(Literal::Char(end_char)) = end_expression {
                            let char_range = CharRange {
                                start: c,
                                end_inclusive: end_char,
                            };
                            elements.push(CharSetElement::CharRange(char_range));
                        } else {
                            let range = Range::merge(&end_range, &self.last_range);
                            return Err(AnreError::MessageWithRange(
                                "Expect a char literal.".to_owned(),
                                range,
                            ));
                        }
                    } else {
                        elements.push(CharSetElement::Char(c));
                    }
                }
                Expression::Literal(Literal::PresetCharSet(preset_charset_name)) => {
                    // preset char set
                    elements.push(CharSetElement::PresetCharSet(preset_charset_name));
                }
                Expression::Literal(Literal::CharSet(char_set)) => {
                    // nested char set, e.g. `['_', ['a'..'f'], ['0'..'9']]`
                    elements.push(CharSetElement::CharSet(Box::new(char_set)));
                }
                _ => {
                    let range = Range::merge(&start_range, &self.last_range);
                    return Err(AnreError::MessageWithRange(
                        "Unsupported char set element.".to_owned(),
                        range,
                    ));
                }
            }
        }

        self.consume_closing_bracket()?; // consume ']'
        Ok(elements)
    }
}

enum Repetition {
    Repeat(usize),
    RepeatFrom(usize),
    RepeatRange(usize, usize),
}

impl TryFrom<&str> for PresetCharSetName {
    type Error = ();

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        match name {
            "char_word" => Ok(Self::CharWord),
            "char_not_word" => Ok(Self::CharNotWord),
            "char_digit" => Ok(Self::CharDigit),
            "char_not_digit" => Ok(Self::CharNotDigit),
            "char_space" => Ok(Self::CharSpace),
            "char_not_space" => Ok(Self::CharNotSpace),
            _ => Err(()),
        }
    }
}

impl TryFrom<&str> for FunctionName {
    type Error = ();

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        match name {
            // Greedy Quantifier
            "optional" => Ok(Self::Optional),
            "one_or_more" => Ok(Self::OneOrMore),
            "zero_or_more" => Ok(Self::ZeroOrMore),
            "repeat" => Ok(Self::Repeat),
            "repeat_range" => Ok(Self::RepeatRange),
            "repeat_from" => Ok(Self::RepeatFrom),

            // Lazy Quantifier
            "optional_lazy" => Ok(Self::OptionalLazy),
            "one_or_more_lazy" => Ok(Self::OneOrMoreLazy),
            "zero_or_more_lazy" => Ok(Self::ZeroOrMoreLazy),
            "repeat_range_lazy" => Ok(Self::RepeatRangeLazy),
            "repeat_from_lazy" => Ok(Self::RepeatFromLazy),

            // Boundary Assertions
            "is_start" => Ok(Self::IsStart),
            "is_end" => Ok(Self::IsEnd),
            "is_bound" => Ok(Self::IsBound),
            "is_not_bound" => Ok(Self::IsNotBound),

            // Lookahead and Lookbehind Assertions
            "is_before" => Ok(Self::IsBefore),        // lookahead
            "is_after" => Ok(Self::IsAfter),          // lookbehind
            "is_not_before" => Ok(Self::IsNotBefore), // negative lookahead
            "is_not_after" => Ok(Self::IsNotAfter),   // negative lookbehind

            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use crate::{
        ast::{
            CharRange, CharSet, CharSetElement, Expression, Literal, PresetCharSetName, Program,
        },
        error::AnreError,
    };

    use super::parse_from_str;

    #[test]
    fn test_parse_literal() {
        let program = parse_from_str(
            r#"
(char_any, 'a', "foo")
    "#,
        )
        .unwrap();

        assert_eq!(
            program,
            Program {
                expression: Expression::Group(vec![
                    Expression::Literal(Literal::AnyChar),
                    Expression::Literal(Literal::Char('a')),
                    Expression::Literal(Literal::String("foo".to_owned())),
                ])
            }
        );

        assert_eq!(program.to_string(), r#"(char_any, 'a', "foo")"#);
    }

    #[test]
    fn test_parse_literal_preset_charset() {
        let program = parse_from_str(
            r#"
(
    char_word
    char_not_word
    char_digit
    char_not_digit
    char_space
    char_not_space
)"#,
        )
        .unwrap();

        assert_eq!(
            program,
            Program {
                expression: Expression::Group(vec![
                    Expression::Literal(Literal::PresetCharSet(PresetCharSetName::CharWord)),
                    Expression::Literal(Literal::PresetCharSet(PresetCharSetName::CharNotWord)),
                    Expression::Literal(Literal::PresetCharSet(PresetCharSetName::CharDigit)),
                    Expression::Literal(Literal::PresetCharSet(PresetCharSetName::CharNotDigit)),
                    Expression::Literal(Literal::PresetCharSet(PresetCharSetName::CharSpace)),
                    Expression::Literal(Literal::PresetCharSet(PresetCharSetName::CharNotSpace)),
                ])
            }
        );

        assert_eq!(
            program.to_string(),
            r#"(char_word, char_not_word, char_digit, char_not_digit, char_space, char_not_space)"#
        );
    }

    #[test]
    fn test_parse_literal_charset() {
        let program = parse_from_str(
            r#"
['a', '0'..'9', char_word]
    "#,
        )
        .unwrap();

        assert_eq!(
            program,
            Program {
                expression: Expression::Literal(Literal::CharSet(CharSet {
                    negative: false,
                    elements: vec![
                        CharSetElement::Char('a'),
                        CharSetElement::CharRange(CharRange {
                            start: '0',
                            end_inclusive: '9'
                        }),
                        CharSetElement::PresetCharSet(PresetCharSetName::CharWord),
                    ]
                }))
            }
        );

        assert_eq!(program.to_string(), r#"['a', '0'..'9', char_word]"#);

        // negative charset
        assert_eq!(
            parse_from_str(
                r#"
!['a'..'z', char_space]
    "#,
            )
            .unwrap()
            .to_string(),
            r#"!['a'..'z', char_space]"#
        );

        // nested charset
        assert_eq!(
            parse_from_str(
                r#"
['-', ['a'..'f'], ['0'..'9']]
    "#,
            )
            .unwrap()
            .to_string(),
            r#"['-', ['a'..'f'], ['0'..'9']]"#
        );
    }

    #[test]
    fn test_parse_function_call() {
        assert_eq!(
            parse_from_str(
                r#"
(
    optional('a')
    one_or_more('b')
    zero_or_more_lazy('c')
)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(optional('a'), one_or_more('b'), zero_or_more_lazy('c'))"#
        );

        // multiple args
        assert_eq!(
            parse_from_str(
                r#"
is_after("bar", "foo")
                "#,
            )
            .unwrap()
            .to_string(),
            r#"is_after("bar", "foo")"#
        );

        // numeric args
        assert_eq!(
            parse_from_str(
                r#"
(
    repeat('a' 3)
    repeat_range('b', 5, 7)
    repeat_from(
    'c' 11)
)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(repeat('a', 3), repeat_range('b', 5, 7), repeat_from('c', 11))"#
        );

        // nested
        assert_eq!(
            parse_from_str(r#"optional(one_or_more('a'))"#)
                .unwrap()
                .to_string(),
            r#"optional(one_or_more('a'))"#
        );
    }

    #[test]
    fn test_parse_method_call() {
        assert_eq!(
            parse_from_str(
                r#"
(
    'a'.optional()
    'b'.one_or_more()
    'c'.zero_or_more_lazy()
)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(optional('a'), one_or_more('b'), zero_or_more_lazy('c'))"#
        );

        // multiple args
        assert_eq!(
            parse_from_str(
                r#"
"bar".is_after("foo")
    "#,
            )
            .unwrap()
            .to_string(),
            r#"is_after("bar", "foo")"#
        );

        // numeric args
        assert_eq!(
            parse_from_str(
                r#"
(
    'a'.repeat(3)
    'b'.repeat_range(5, 7)
    'c'.repeat_from(11)
)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(repeat('a', 3), repeat_range('b', 5, 7), repeat_from('c', 11))"#
        );

        // chain method call
        assert_eq!(
            parse_from_str(
                r#"
'a'.one_or_more().optional()
    "#
            )
            .unwrap()
            .to_string(),
            r#"optional(one_or_more('a'))"#
        );
    }

    #[test]
    fn test_parse_quantifier() {
        assert_eq!(
            parse_from_str(
                r#"
(
    'a'?
    'b'+
    'c'*
    'x'??
    'y'+?
    'z'*?
)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(optional('a'), one_or_more('b'), zero_or_more('c'), optional_lazy('x'), one_or_more_lazy('y'), zero_or_more_lazy('z'))"#
        );

        assert_eq!(
            parse_from_str(
                r#"
(
    'a'{3}
    'b'{5..7}
    'c'{11..}
    'y'{5..7}?
    'z'{11..}?
)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(repeat('a', 3), repeat_range('b', 5, 7), repeat_from('c', 11), repeat_range_lazy('y', 5, 7), repeat_from_lazy('z', 11))"#
        );

        // err: '{m}?' is not allowed
        assert!(matches!(
            parse_from_str(
                r#"
'a'{3}?
"#,
            ),
            Err(AnreError::MessageWithRange(_, _))
        ));

        // err: '{m,m}?' is not allowed
        assert!(matches!(
            parse_from_str(
                r#"
'a'{3..3}?
"#,
            ),
            Err(AnreError::MessageWithRange(_, _))
        ));
    }

    #[test]
    fn test_parse_index_capture_and_backreference() {
        assert_eq!(
            parse_from_str(
                r#"
#'a'
    "#,
            )
            .unwrap()
            .to_string(),
            r#"#'a'"#
        );

        assert_eq!(
            parse_from_str(
                r#"
#('a', char_digit)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"#('a', char_digit)"#
        );

        assert_eq!(
            parse_from_str(
                r#"
(#char_digit+ '.' ^1)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(#one_or_more(char_digit), '.', ^1)"#
        );
    }

    #[test]
    fn test_parse_name_capture_and_backreference() {
        assert_eq!(
            parse_from_str(
                r#"
'a' as x
    "#,
            )
            .unwrap()
            .to_string(),
            r#"'a' as x"#
        );

        assert_eq!(
            parse_from_str(
                r#"
('a', char_digit) as x
    "#,
            )
            .unwrap()
            .to_string(),
            r#"('a', char_digit) as x"#
        );

        // name capture implies index capture
        assert_eq!(
            parse_from_str(
                r#"
#'a' as x
    "#,
            )
            .unwrap()
            .to_string(),
            r#"'a' as x"#
        );

        // name capture implies index capture, another case
        assert_eq!(
            parse_from_str(
                r#"
#('a' as x)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"'a' as x"#
        );

        assert_eq!(
            parse_from_str(
                r#"
(char_digit as a, 'x', a)
    "#,
            )
            .unwrap()
            .to_string(),
            r#"(char_digit as a, 'x', a)"#
        );
    }

    #[test]
    fn test_parse_logic_or() {
        {
            let program = parse_from_str(
                r#"
'a' || 'b'
    "#,
            )
            .unwrap();

            assert_eq!(
                program,
                Program {
                    expression: Expression::Or(
                        Box::new(Expression::Literal(Literal::Char('a'))),
                        Box::new(Expression::Literal(Literal::Char('b'))),
                    )
                }
            );

            assert_eq!(program.to_string(), r#"'a' || 'b'"#);
        }

        // two operands
        {
            let program = parse_from_str(
                r#"
'a' || 'b' || 'c'
"#,
            )
            .unwrap();

            assert_eq!(
                program,
                Program {
                    expression: Expression::Or(
                        Box::new(Expression::Literal(Literal::Char('a'))),
                        Box::new(Expression::Or(
                            Box::new(Expression::Literal(Literal::Char('b'))),
                            Box::new(Expression::Literal(Literal::Char('c'))),
                        )),
                    )
                }
            );

            assert_eq!(program.to_string(), r#"'a' || ('b' || 'c')"#);
        }

        // logic or + group
        assert_eq!(
            parse_from_str(
                r#"
'a' || ('b' || 'c')
"#,
            )
            .unwrap()
            .to_string(),
            r#"'a' || ('b' || 'c')"#
        );

        // group + logic or
        assert_eq!(
            parse_from_str(
                r#"
('a' || 'b') || 'c'
"#,
            )
            .unwrap()
            .to_string(),
            r#"('a' || 'b') || 'c'"#
        );

        // group + logic or + group
        assert_eq!(
            parse_from_str(
                r#"
('a', char_word) || ('b', char_digit)
"#,
            )
            .unwrap()
            .to_string(),
            r#"('a', char_word) || ('b', char_digit)"#
        );

        // string + logic or
        assert_eq!(
            parse_from_str(
                r#"
"ab" || "cd"
"#,
            )
            .unwrap()
            .to_string(),
            r#""ab" || "cd""#
        );

        // expressions as operands
        assert_eq!(
            parse_from_str(
                r#"
char_digit.one_or_more() || [char_word, '-']+
"#,
            )
            .unwrap()
            .to_string(),
            r#"one_or_more(char_digit) || one_or_more([char_word, '-'])"#
        );
    }

    #[test]
    fn test_parse_group() {
        assert_eq!(
            parse_from_str(
                r#"
(
    ("foo", char_digit)
    ('b', ("bar", char_digit))
    is_end()
)
"#,
            )
            .unwrap()
            .to_string(),
            r#"(("foo", char_digit), ('b', ("bar", char_digit)), is_end())"#
        );

        // function call + group
        assert_eq!(
            parse_from_str(
                r#"
(
    repeat(("foo", char_digit), 3)
    ('b', repeat("bar", 5))
    is_end()
)
"#,
            )
            .unwrap()
            .to_string(),
            r#"(repeat(("foo", char_digit), 3), ('b', repeat("bar", 5)), is_end())"#
        );

        // escape nested group
        assert_eq!(
            parse_from_str(
                r#"
(((('a', char_digit, 'b'))))
"#,
            )
            .unwrap()
            .to_string(),
            r#"('a', char_digit, 'b')"#
        );
    }

    #[test]
    fn test_parse_macro() {
        assert_eq!(
            parse_from_str(
                r#"
define A ("abc")

(is_start(), A, is_end())
"#,
            )
            .unwrap()
            .to_string(),
            r#"(is_start(), "abc", is_end())"#
        );

        assert_eq!(
            parse_from_str(
                r#"
define A ('a')
define B (A, 'b')
define C ([A, 'c'], optional(B), B.one_or_more())
define D (A || B || 'd')

(is_start(), A, B, C, D, is_end())
"#,
            )
            .unwrap()
            .to_string(),
            r#"(is_start(), 'a', ('a', 'b'), (['a', 'c'], optional(('a', 'b')), one_or_more(('a', 'b'))), 'a' || (('a', 'b') || 'd'), is_end())"#
        );
    }

    #[test]
    fn test_parse_examples() {
        assert_eq!(
            parse_from_str(
                r#"
/**
 * Decimal Numbers Regular Expression
 * e.g.
 * - "0"
 * - "123"
 */

char_digit.one_or_more()
"#,
            )
            .unwrap()
            .to_string(),
            "one_or_more(char_digit)"
        );

        assert_eq!(
            parse_from_str(
                r#"
/**
 * Hex Numbers Regular Expression
 * e.g.
 * - "0x0"
 * - "0x123"
 * - "0xabc"
 * - "0xDEADBEEF"
 */

(
    // The prefix "0x"
    "0x"

    // The hex digits
    ['0'..'9', 'a'..'f', 'A'..'F'].one_or_more()
)
"#,
            )
            .unwrap()
            .to_string(),
            "(\"0x\", one_or_more(['0'..'9', 'a'..'f', 'A'..'F']))"
        );

        assert_eq!(
            parse_from_str(
                r#"
/**
 * Email Address Validated Regular Expression
 *
 * e.g.
 * - "abc@example.domain"
 * - "john-smith.new+mailbox-department@example.com"
 *
 * Ref:
 * https://en.wikipedia.org/wiki/Email_address
 */

(
    // Asserts that the current is the first character
    is_start()

    // User name
    [char_word, '.', '-'].one_or_more()

    // Sub-address
    ('+', [char_word, '-'].one_or_more()).optional()

    // The separator
    '@'

    // Domain name
    (
        ['a'..'z', 'A'..'Z', '0'..'9', '-'].one_or_more()
        '.'
    ).one_or_more()

    // Top-level domain
    ['a'..'z'].repeat_from(2)

    // Asserts that the current is the last character
    is_end()
)
"#,
            )
            .unwrap()
            .to_string(),
            "(is_start(), \
one_or_more([char_word, '.', '-']), \
optional(('+', one_or_more([char_word, '-']))), \
'@', \
one_or_more((one_or_more(['a'..'z', 'A'..'Z', '0'..'9', '-']), '.')), \
repeat_from(['a'..'z'], 2), \
is_end())"
        );

        let ipv4_regex = parse_from_str(
            r#"
/**
 * IPv4 Address Validated Regular Expression
 */

define num_25x ("25", ['0'..'5'])
define num_2xx ('2', ['0'..'4'], char_digit)
define num_1xx ('1', char_digit, char_digit)
define num_xx (['1'..'9'], char_digit)
define num_x (char_digit)
define part (num_25x || num_2xx || num_1xx || num_xx || num_x)

(is_start(), (part, '.').repeat(3), part, is_end())
"#,
        )
        .unwrap()
        .to_string();

        let part_str = r#"("25", ['0'..'5']) || (('2', ['0'..'4'], char_digit) || (('1', char_digit, char_digit) || ((['1'..'9'], char_digit) || char_digit)))"#;
        let expected_ipv4_regex = format!(
            "(is_start(), repeat(({}, '.'), 3), {}, is_end())",
            part_str, part_str
        );

        assert_eq!(ipv4_regex, expected_ipv4_regex);

        assert_eq!(
            parse_from_str(
                r#"
/**
 * Simple HTML tag Regular Expression
 */

(
    '<'                                                                 // opening tag
    char_word+ as tag_name                                              // tag name
    (char_space, char_word+, ('=', '"', char_word+, '"').optional())*   // attributes
    '>'
    char_any+?                                                          // text content
    '<', '/', tag_name, '>'                                             // closing tag
)
"#,
            )
            .unwrap()
            .to_string(),
            "(\
'<', \
one_or_more(char_word) as tag_name, \
zero_or_more((char_space, one_or_more(char_word), optional(('=', '\\\"', one_or_more(char_word), '\\\"')))), \
'>', \
one_or_more_lazy(char_any), \
'<', '/', tag_name, '>'\
)"
        );
    }
}
