// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::ops::{Add, BitOr, Mul};

use crate::ast::{Expression, FunctionArgument, FunctionName, Literal};

/// Computes consumed length in Unicode scalar values to validate fixed-length lookbehind.
pub fn calculate_match_length(exp: &Expression) -> MatchLength {
    match exp {
        Expression::Literal(literal) => match literal {
            Literal::Char(_) => MatchLength::Fixed(1),
            Literal::String(s) => MatchLength::Fixed(s.chars().count()),
            Literal::AnyChar => MatchLength::Fixed(1),
            Literal::CharSet(_) => MatchLength::Fixed(1),
            Literal::PresetCharSet(_) => MatchLength::Fixed(1),
        },
        Expression::BackReference(_) => MatchLength::Variable,
        Expression::Group(exps) => exps
            .iter()
            .map(calculate_match_length)
            .fold(MatchLength::Fixed(0), |acc, item| acc + item),
        Expression::FunctionCall(function_call) => match function_call.name {
            FunctionName::IsStart | FunctionName::IsEnd => MatchLength::Fixed(0),
            FunctionName::IsBound | FunctionName::IsNotBound => MatchLength::Fixed(0),
            FunctionName::Optional => MatchLength::Variable,
            FunctionName::OneOrMore => MatchLength::Variable,
            FunctionName::ZeroOrMore => MatchLength::Variable,
            FunctionName::Repeat | FunctionName::LazyRepeat => {
                let FunctionArgument::Expression(base_exp) = &function_call.args[0] else {
                    unreachable!()
                };
                let FunctionArgument::Number(factor) = &function_call.args[1] else {
                    unreachable!()
                };

                calculate_match_length(base_exp) * (*factor)
            }
            FunctionName::RepeatRange => MatchLength::Variable,
            FunctionName::RepeatFrom => MatchLength::Variable,
            FunctionName::LazyOptional => MatchLength::Variable,
            FunctionName::LazyOneOrMore => MatchLength::Variable,
            FunctionName::LazyZeroOrMore => MatchLength::Variable,
            FunctionName::LazyRepeatRange => MatchLength::Variable,
            FunctionName::LazyRepeatFrom => MatchLength::Variable,
            // Lookaround parts are assertions and do not consume characters.
            FunctionName::IsBefore
            | FunctionName::IsNotBefore
            | FunctionName::IsAfter
            | FunctionName::IsNotAfter
            | FunctionName::Index
            | FunctionName::Name => {
                let FunctionArgument::Expression(base_exp) = &function_call.args[0] else {
                    unreachable!()
                };
                calculate_match_length(base_exp)
            }
        },
        Expression::Or(left_exp, right_exp) => {
            calculate_match_length(left_exp) | calculate_match_length(right_exp)
        }
    }
}

pub enum MatchLength {
    // Variable length
    Variable,

    // Length in Unicode scalar values, not bytes.
    Fixed(usize),
}

impl Add for MatchLength {
    type Output = MatchLength;

    fn add(self, rhs: Self) -> Self::Output {
        match self {
            MatchLength::Variable => MatchLength::Variable,
            MatchLength::Fixed(v0) => match rhs {
                MatchLength::Variable => MatchLength::Variable,
                MatchLength::Fixed(v1) => MatchLength::Fixed(v0 + v1),
            },
        }
    }
}

impl Mul<usize> for MatchLength {
    type Output = MatchLength;

    fn mul(self, rhs: usize) -> Self::Output {
        match self {
            MatchLength::Variable => MatchLength::Variable,
            MatchLength::Fixed(v) => MatchLength::Fixed(v * rhs),
        }
    }
}

impl BitOr for MatchLength {
    type Output = MatchLength;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            MatchLength::Variable => MatchLength::Variable,
            MatchLength::Fixed(v0) => match rhs {
                MatchLength::Variable => MatchLength::Variable,
                MatchLength::Fixed(v1) => {
                    if v0 == v1 {
                        MatchLength::Fixed(v0)
                    } else {
                        MatchLength::Variable
                    }
                }
            },
        }
    }
}
