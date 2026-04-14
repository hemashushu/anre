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
     * Represents a function call, which can be a quantifier (for example, `optional()`, `one_or_more()`)
     * or an assertion (for example, `is_before()`, `is_after()`,
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
    // except for line terminators (for example, `\n`, `\r`).
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
    RepeatFrom,  // `repeat_from(expression, n)->expression`, n >= 0
    RepeatRange, // `repeat_range(expression, m, n)->expression`, m >= 0, n >= m (internally, function `repeat` is used if m == n)

    // Lazy Quantifier
    LazyOptional,    // `lazy_optional(expression)->expression`
    LazyOneOrMore,   // `lazy_one_or_more(expression)->expression`
    LazyZeroOrMore,  // `lazy_zero_or_more(expression)->expression`
    LazyRepeat,      // `lazy_repeat(expression, n)->expression`, n >= 0
    LazyRepeatFrom,  // `lazy_repeat_from(expression, n)->expression`, n >= 0
    LazyRepeatRange, // `lazy_repeat_range(expression, m, n)->expression`, m >= 0, n >= m (error is occurred if m == n)

    // Note that `LazyRepeat` is semantically equivalent to `Repeat`
    // because the laziness of a fixed repetition has no effect.

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
    // - 'a'.is_before('b'.is_after('c'))` always fails because it is
    //   impossible for 'a' to follow 'c' and for 'b' to follow 'a' at the same time.
    // - 'c'.is_after('a'.is_before('b'))` always fails because it is
    //   impossible for 'c' to precede 'a' and for 'b' to precede 'c' at the same time.

    // `is_before(expression, next_expression)->expression`
    // lookahead `A(?=B)`: `is_before(A, B)` or `A.is_before(B)`
    IsBefore,

    // `is_not_before(expression, next_expression)->expression`
    // negative lookahead `A(?!B)`: `is_not_before(A, B)` or `A.is_not_before(B)`
    IsNotBefore,

    // `is_after(expression, previous_expression)->expression`
    // lookbehind `(?<=B)A`: `is_after(A, B)` or `A.is_after(B)`
    IsAfter,

    // `is_not_after(expression, previous_expression)->expression`
    // negative lookbehind `(?<!B)A`: `is_not_after(A, B)` or `A.is_not_after(B)`
    IsNotAfter,
}
