// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::position::Position;

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct Range {
    pub start: Position,
    pub end_inclusive: Position,
}

impl Range {
    pub fn new(start: &Position, end_inclusive: &Position) -> Self {
        Self {
            start: *start,
            end_inclusive: *end_inclusive,
        }
    }

    /// Creates an inclusive range containing a single source position.
    pub fn from_single_position(pos: &Position) -> Self {
        Self {
            start: *pos,
            end_inclusive: *pos,
        }
    }

    /// Creates a same-line range with a nonzero length in characters.
    pub fn from_position_and_length(pos: &Position, length: usize) -> Self {
        let inc = length - 1;
        Self {
            start: *pos,
            end_inclusive: Position::new(pos.index + inc, pos.line, pos.column + inc),
        }
    }

    /// Creates a same-line range from character coordinates and a nonzero length.
    pub fn from_detail(index: usize, line: usize, column: usize, length: usize) -> Self {
        let inc = length - 1;
        let start = Position::new(index, line, column);
        let end_inclusive = Position::new(index + inc, line, column + inc);
        Self {
            start,
            end_inclusive,
        }
    }

    /// Spans from the start of `left` to the end of `right`, preserving their order.
    pub fn merge(left: &Self, right: &Self) -> Self {
        Self {
            start: left.start,
            end_inclusive: right.end_inclusive,
        }
    }
}
