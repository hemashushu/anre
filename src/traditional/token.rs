// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::range::Range;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    CharSetStart,         // [
    CharSetStartNegative, // [^
    CharSetEnd,           // ]

    ZeroOrMore,         // *
    ZeroOrMoreLazy,     // *?
    OneOrMore,          // +
    OneOrMoreLazy,      // +?
    Optional,           // ?
    OptionalLazy,       // ??
    LogicOr,            // `|`
    LineAssertionStart, // ^
    LineAssertionEnd,   // $
    Dot,                // .

    Char(char),
    CharRange(/* start */ char, /* end_inclusive */ char), // e.g. a-zA-Z0-9
    PresetCharSet(char),                                   // e.g. \d, \w, \s
    BoundaryAssertion(/* negative */ bool),                // e.g. \b, \B
    Repetition(Repetition, /* is_lazy */ bool),            // e.g. {N}, {M,}, {M,N}

    GroupStart,                     // (
    NonCaptureGroupStart,           // (?...
    LookAheadGroupStart,            // (?=...
    LookAheadNegativeGroupStart,    // (?!...
    NamedCaptureGroupStart(String), // (?<...
    LookBehindGroupStart,           // (?<=...
    LookBehindNegativeGroupStart,   // (?<!...
    GroupEnd,                       // )

    BackReferenceNumber(usize), // \number
    BackReferenceName(String),  // \k<name>
}

#[derive(Debug, PartialEq, Clone)]
pub enum Repetition {
    Repeat(usize),
    RepeatFrom(usize),
    RepeatRange(usize, usize),
}

#[derive(Debug, PartialEq, Clone)]
pub struct TokenWithRange {
    pub token: Token,
    pub range: Range,
}

impl TokenWithRange {
    pub fn new(token: Token, range: Range) -> Self {
        Self { token, range }
    }
}
