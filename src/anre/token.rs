// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::range::Range;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // Represents keyword.
    // - `define`
    // - `as`
    //
    // e.g.
    // - `define identifier ( ... )` represents a macro definition.
    // - `#![char_word, '-']+ as foo` represents `index(name(![char_word, '-'], foo))`.
    Keyword(String),

    // Represents an identifier, which includes alphanumeric characters and underscores.
    // [a-zA-Z0-9_] and '\u{a0}' - '\u{d7ff}' and '\u{e000}' - '\u{10ffff}'
    //
    // Identifiers include:
    // - Named capturing groups (e.g., `(...) as identifier`).
    // - Function names (e.g., `optional()`, `one_or_more()`).
    // - Anchor assertion function names (e.g., `is_start()`, `is_end()`).
    // - Boundary assertion function names (e.g., `is_bound()`, `is_not_bound()`).
    // - Macro names (e.g., `define identifier (...)`).
    // - Preset character set names (e.g., `char_word`).
    Identifier(String),

    // Represents a numeric value.
    Number(usize),

    // Represents a single character.
    Char(char),

    // Represents a string literal.
    String(String),

    // Represents a question mark (`?`).
    // e.g. `'a'?` matches 'a' zero or one time.
    Question,

    // Represents a lazy question mark (`??`).
    // e.g. `'a'??` matches 'a' zero or one time, but prefers zero.
    QuestionLazy,

    // Represents a plus sign (`+`).
    // e.g. `'a'+` matches 'a' one or more times.
    Plus,

    // Represents a lazy plus sign (`+?`).
    // e.g. `'a'+?` matches 'a' one or more times, but prefers fewer matches.
    PlusLazy,

    // Represents an asterisk (`*`).
    // e.g. `'a'*` matches 'a' zero or more times.
    Asterisk,

    // Represents a lazy asterisk (`*?`).
    // e.g. `'a'*?` matches 'a' zero or more times, but prefers fewer matches.
    AsteriskLazy,

    // Represents a left curly brace (`{`).
    // e.g.
    // - `{3}` matches exactly three occurrences of the preceding element.
    // - `{3..}` matches three or more occurrences of the preceding element.
    // - `{3..5}` matches between three and five occurrences of the preceding element.
    BraceOpen,

    // Represents a right curly brace (`}`).
    BraceClose,

    // Represents an exclamation mark (`!`).
    // e.g. `![abc]` represents a negative character set that matches any character except 'a', 'b', or 'c'.
    Exclamation,

    // Represents a range operator (`..`).
    // e.g. `'a'..'z'` represents a character range that matches any character from 'a' to 'z' inclusive.
    Range,

    // Represents a hash sign (`#`).
    // e.g. `#![char_word, '-']+ as foo` represents `index(name(![char_word, '-'], foo))`.
    Hash,

    // Represents a dot (`.`).
    Dot,

    // Represents a logical OR operator (`||`).
    // e.g. `'a' || 'b'` matches either 'a' or 'b'.
    LogicOr,

    // Represents a left square bracket (`[`).
    // e.g. `['a' 'b' 'c']` represents a character set that matches
    // any character 'a', 'b', or 'c'.
    BracketOpen,

    // Represents a right square bracket (`]`).
    BracketClose,

    // Represents a left parenthesis (`(`).
    // e.g.
    // - `('a' 'b')` represents a group that matches 'a' followed by 'b'.
    // - `name()` represents a function call to a function named `name`.
    ParenthesisOpen,

    // Represents a right parenthesis (`)`).
    ParenthesisClose,
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
