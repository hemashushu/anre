// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

#[derive(Debug, PartialEq)]
pub struct Program {
    pub expression: Expression,
}

#[derive(Debug, PartialEq)]
pub enum Expression {
    Literal(Literal),

    BackReference(BackReference),

    /**
     * The "group" in ANRE differs from the "group" in traditional regular expressions.
     * In ANRE, a "group" is a series of parenthesized patterns that are not captured
     * unless explicitly referenced by the `name` or `index` function.
     * In terms of results, an ANRE "group" is equivalent to a "non-capturing group"
     * in traditional regular expressions.
     *
     * Example:
     *
     * ANRE: `('a', 'b', char_word+)`
     * Equivalent regex: `ab\w+`
     *
     * Groups in ANRE are used to group patterns and modify operator precedence
     * and associativity.
     */
    Group(Vec<Expression>),

    /**
     * Represents a function call, which can be a quantifier (e.g., `optional()`, `one_or_more()`)
     * or an assertion (e.g., `is_before()`, `is_after()`,
     */
    FunctionCall(Box<FunctionCall>),

    IndexCapture(Box<Expression>),
    NameCapture(String, Box<Expression>),

    /**
     * Represents a disjunction (logical OR) between two expressions.
     * For example, `a|b` matches either 'a' or 'b'.
     * Reference: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Disjunction
     */
    Or(Box<Expression>, Box<Expression>),
}

#[derive(Debug, PartialEq)]
pub struct FunctionCall {
    pub name: FunctionName,
    pub args: Vec<FunctionArgument>,
}

#[derive(Debug, PartialEq)]
pub enum FunctionArgument {
    Expression(Expression),
    Number(usize),
}

#[derive(Debug, PartialEq)]
pub enum Literal {
    // The "any character" literal `.` matches any single character
    // except for line terminators (e.g., `\n`, `\r`).
    AnyChar,

    // A character literal represents a single character.
    // For example, the character literal `'a'` matches the character 'a'.
    Char(char),

    // A string literal represents a sequence of characters.
    String(String),

    // A preset character set represents a predefined set of characters,
    // such as `char_word` or `char_digit`.
    PresetCharSet(PresetCharSetName),

    // A character set represents a set of characters defined by the user.
    CharSet(CharSet),
}

#[derive(Debug, PartialEq)]
pub struct CharSet {
    pub negative: bool,
    pub elements: Vec<CharSetElement>,
}

#[derive(Debug, PartialEq)]
pub enum CharSetElement {
    Char(char),
    CharRange(CharRange),
    PresetCharSet(PresetCharSetName),

    // Nested charsets are allowed in ANRE, but only as positive charsets.
    // A nested charset is a charset that is included as an element within another charset.
    // For example, `['a', ['b', 'c']]` represents a charset that includes 'a', 'b', and 'c'.
    // However, `['a', !['b', 'c']]` is not allowed because the inner charset is negative.
    CharSet(Box<CharSet>),
}

#[derive(Debug, PartialEq)]
pub struct CharRange {
    pub start: char,
    pub end_inclusive: char,
}

#[derive(Debug, PartialEq)]
pub enum BackReference {
    Index(usize),
    Name(String),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PresetCharSetName {
    CharWord,
    CharNotWord,
    CharDigit,
    CharNotDigit,
    CharSpace,
    CharNotSpace,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FunctionName {
    // Greedy Quantifier
    Optional,    // `optional(expression)->expression`
    OneOrMore,   // `one_or_more(expression)->expression`
    ZeroOrMore,  // `zero_or_more(expression)->expression`
    Repeat,      // `repeat(expression, n)->expression`, n >= 0
    RepeatRange, // `repeat_range(expression, m, n)->expression`, m >= 0, n >= m (internally, function `repeat` is used if m == n)
    RepeatFrom,  // `repeat_from(expression, n)->expression`, n >= 0

    // Lazy Quantifier
    OptionalLazy,    // `optional_lazy(expression)->expression`
    OneOrMoreLazy,   // `one_or_more_lazy(expression)->expression`
    ZeroOrMoreLazy,  // `zero_or_more_lazy(expression)->expression`
    RepeatRangeLazy, // `repeat_range_lazy(expression, m, n)->expression`, m >= 0, n >= m (error is occurred if m == n)
    RepeatFromLazy,  // `repeat_from_lazy(expression, n)->expression`, n >= 0

    // Boundary Assertions (i.e., "判定")
    IsStart,    // `is_start()->()`
    IsEnd,      // `is_end()->()`
    IsBound,    // `is_bound()->()`
    IsNotBound, // `is_not_bound()->()`

    // Lookahead and Lookbehind Assertions
    //
    // Some combinations of lookahead and lookbehind assertions are
    // logically impossible and will always fail:
    // - `('a', 'c'.is_after('b'))` always fails because it is
    //   impossible for 'a' and 'b' to both precede 'c'.
    // - `('c'.is_before('a'), 'b')` always fails because it is
    //   impossible for 'a' and 'b' to both follow 'c'.
    IsBefore,    // `is_before(expression, expression)->expression` (lookahead)
    IsAfter,     // `is_after(expression, expression)->expression` (lookbehind)
    IsNotBefore, // `is_not_before(expression, expression)->expression` (negative lookahead)
    IsNotAfter,  // `is_not_after(expression, expression)->expression` (negative lookbehind)
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
