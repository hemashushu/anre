// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use std::ops::{Add, BitOr, Mul};

use crate::ast::{Expression, FunctionArgument, FunctionName, Literal};

/// Calculate the match length of an expression.
///
/// The match length is the number of characters that the expression can match.
/// The "look-behind assertion" requires the match length to be fixed.
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
            .reduce(|acc, item| acc + item)
            .unwrap(),
        Expression::FunctionCall(function_call) => match function_call.name {
            FunctionName::IsStart | FunctionName::IsEnd => MatchLength::Fixed(0),
            FunctionName::IsBound | FunctionName::IsNotBound => MatchLength::Fixed(0),
            FunctionName::Optional => MatchLength::Variable,
            FunctionName::OneOrMore => MatchLength::Variable,
            FunctionName::ZeroOrMore => MatchLength::Variable,
            FunctionName::Repeat => {
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
            FunctionName::LazyRepeat => MatchLength::Variable,
            FunctionName::LazyRepeatRange => MatchLength::Variable,
            FunctionName::LazyRepeatFrom => MatchLength::Variable,
            FunctionName::IsBefore => {
                let FunctionArgument::Expression(base_exp) = &function_call.args[0] else {
                    unreachable!()
                };
                calculate_match_length(base_exp)
            }
            FunctionName::IsAfter => {
                let FunctionArgument::Expression(base_exp) = &function_call.args[0] else {
                    unreachable!()
                };
                let FunctionArgument::Expression(ref_exp) = &function_call.args[1] else {
                    unreachable!()
                };
                calculate_match_length(base_exp) + calculate_match_length(ref_exp)
            }
            FunctionName::IsNotBefore => {
                let FunctionArgument::Expression(base_exp) = &function_call.args[0] else {
                    unreachable!()
                };
                calculate_match_length(base_exp)
            }
            FunctionName::IsNotAfter => {
                let FunctionArgument::Expression(base_exp) = &function_call.args[0] else {
                    unreachable!()
                };
                let FunctionArgument::Expression(ref_exp) = &function_call.args[1] else {
                    unreachable!()
                };
                calculate_match_length(base_exp) + calculate_match_length(ref_exp)
            }
        },
        Expression::Or(left_exp, right_exp) => {
            calculate_match_length(left_exp) | calculate_match_length(right_exp)
        }
        Expression::IndexCapture(exp) => calculate_match_length(exp),
        Expression::NameCapture(_, exp) => calculate_match_length(exp),
    }
}

pub enum MatchLength {
    // Variable length
    Variable,

    // length by char (unicode char codepoint)
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
