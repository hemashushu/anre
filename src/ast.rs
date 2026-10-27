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

    // ANRE groups are non-capturing; `index` and `name` explicitly add captures.
    Group(Vec<Expression>),

    FunctionCall(Box<FunctionCall>),

    // Alternation tries the left branch before the right branch.
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
    Number(usize),      // repetition arguments
    Identifier(String), // Named capture group identifier
}

#[derive(Debug, PartialEq)]
pub enum Literal {
    // `char_any` and `.` exclude LF and CR, but not other Unicode line separators.
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

    // Only non-negated sets may be nested, as in `['a', ['b', 'c']]`.
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
    // Greedy quantifiers
    Optional,    // `optional(expression)->expression`
    OneOrMore,   // `one_or_more(expression)->expression`
    ZeroOrMore,  // `zero_or_more(expression)->expression`
    Repeat,      // `repeat(expression, n)->expression`, n >= 0
    RepeatFrom,  // `repeat_from(expression, n)->expression`, n >= 0
    RepeatRange, // `repeat_range(expression, m, n)->expression`, m >= 0, n >= m

    // Lazy quantifiers
    LazyOptional,    // `lazy_optional(expression)->expression`
    LazyOneOrMore,   // `lazy_one_or_more(expression)->expression`
    LazyZeroOrMore,  // `lazy_zero_or_more(expression)->expression`
    LazyRepeat,      // `lazy_repeat(expression, n)->expression`, n >= 0
    LazyRepeatFrom,  // `lazy_repeat_from(expression, n)->expression`, n >= 0
    LazyRepeatRange, // `lazy_repeat_range(expression, m, n)->expression`, m >= 0, n >= m

    // Laziness has no effect on a fixed repetition count.

    // Boundary assertions match positions without consuming characters.
    IsStart,    // `is_start()`
    IsEnd,      // `is_end()`
    IsBound,    // `is_bound()`
    IsNotBound, // `is_not_bound()`

    // Lookahead and Lookbehind Assertions
    //
    // Some combinations of lookahead and lookbehind assertions are
    // logically impossible and will always fail:
    // - `('a', 'c'.is_after('b'))` always fails because it is
    //   impossible for 'a' and 'b' to both precede 'c'.
    // - `('c'.is_before('a'), 'b')` always fails because it is
    //   impossible for 'a' and 'b' to both follow 'c'.
    // - `'a'.is_before('b'.is_after('c'))` requires both 'a' and 'c' immediately before 'b'.
    // - `'c'.is_after('a'.is_before('b'))` requires both 'b' and 'c' immediately after 'a'.

    // `is_before(expression, next_expression)->expression`
    //
    // lookahead assertion:
    // - `A(?=B)`
    // - `is_before(A, B)`
    // - `A.is_before(B)`
    IsBefore,

    // `is_not_before(expression, next_expression)->expression`
    //
    // negative lookahead assertion:
    // - `A(?!B)`
    // - `is_not_before(A, B)`
    // - `A.is_not_before(B)`
    IsNotBefore,

    // `is_after(expression, previous_expression)->expression`
    //
    // lookbehind assertion:
    // - `(?<=B)A`
    // - `is_after(A, B)`
    // - `A.is_after(B)`
    IsAfter,

    // `is_not_after(expression, previous_expression)->expression`
    //
    // negative lookbehind assertion:
    // - `(?<!B)A`
    // - `is_not_after(A, B)`
    // - `A.is_not_after(B)`
    IsNotAfter,

    // `index(expression)->expression`
    //
    // indexed capturing group:
    // - `(\w+)`
    // - `index(char_word+)`
    // - `char_word+.index()`
    Index,

    // `name(expression, NAME)->expression`
    //
    // named capturing group:
    // - `(?<foo>\w+)`
    // - `name(char_word+, foo)`
    // - `char_word+.name(foo)`
    Name,
}
