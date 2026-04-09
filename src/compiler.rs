// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{
    ast::{
        BackReference, CharRange, CharSet, CharSetElement, Expression, FunctionArgument,
        FunctionCall, FunctionName, Literal, PresetCharSetName, Program,
    },
    error::AnreError,
    match_length_calculator::{MatchLength, calculate_match_length},
    object_file::{Component, Map, Route},
    transition::{
        AnyCharTransition, BackReferenceTransition, CaptureEndTransition, CaptureStartTransition,
        CharSetItem, CharSetTransition, CharTransition, CounterIncTransition,
        CounterResetTransition, CounterSaveTransition, JumpTransition,
        LineBoundaryAssertionTransition, LookAheadAssertionTransition,
        LookBehindAssertionTransition, RepetitionBackTransition, RepetitionForwardTransition,
        RepetitionType, StringTransition, Transition, WordBoundaryAssertionTransition, add_char,
        add_preset_digit, add_preset_space, add_preset_word, add_range,
    },
};

/// Compile from traditional regular expression.
pub fn compile_from_regex(s: &str) -> Result<Map, AnreError> {
    let program = crate::traditional::parse_from_str(s)?;
    compile(&program)
}

/// Compile from ANRE regular expression.
pub fn compile_from_anre(s: &str) -> Result<Map, AnreError> {
    let program = crate::anre::parse_from_str(s)?;
    compile(&program)
}

/// Compile from AST `Program`.
pub fn compile(program: &Program) -> Result<Map, AnreError> {
    let mut map = Map::new();
    let mut compiler = Compiler::new(program, &mut map);
    compiler.compile()?;
    Ok(map)
}

pub struct Compiler<'a> {
    // The AST
    program: &'a Program,

    // The compilation target
    map: &'a mut Map,

    // Index of the current route
    current_route_index: usize,
}

impl<'a> Compiler<'a> {
    fn new(program: &'a Program, map: &'a mut Map) -> Self {
        let current_route_index = map.create_route();
        Compiler {
            program,
            map,
            current_route_index,
        }
    }

    fn get_current_route_index(&self) -> usize {
        self.current_route_index
    }

    fn set_current_route_index(&mut self, index: usize) {
        self.current_route_index = index;
    }

    // Get a mutable reference to the current route in the object file.
    fn get_current_route_ref_mut(&mut self) -> &mut Route {
        &mut self.map.routes[self.current_route_index]
    }

    // Start the compilation process by emitting the main program.
    fn compile(&mut self) -> Result<(), AnreError> {
        self.emit_program(self.program)
    }

    // Compile the main route of the program.
    fn emit_program(&mut self, program: &Program) -> Result<(), AnreError> {
        // Create an index capture group to wrap for the root expression component.
        //
        // ```diagram
        //   /---------------------------------------------------------\
        //   |                                                         |
        //   |  capture start |                     capture end |      |
        //   |     transition |                      transition |      |
        //   |                V    /-------------\              v      |
        // =====o==-------------===o in      out o===-------------==o=====
        //   |  in                 \-------------/                 out |
        //   |  node               root expression                node |
        //   |                        component                        |
        //   |                                                         |
        //   \------------------- program component--------------------/
        // ```

        let capture_group_index = self.map.create_capture_group(None);
        let root_expression_component = self.emit_expression(&program.expression)?;

        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        let capture_start_transition = CaptureStartTransition::new(capture_group_index);
        let capture_end_transition = CaptureEndTransition::new(capture_group_index);

        // connect wrapper 'in' node to the program 'in' node.
        route.create_path(
            in_node_index,
            root_expression_component.in_node_index,
            Transition::CaptureStart(capture_start_transition),
        );

        // connect the program 'out' node to wrapper 'out' node.
        route.create_path(
            root_expression_component.out_node_index,
            out_node_index,
            Transition::CaptureEnd(capture_end_transition),
        );

        let is_fixed_matching_begin_point =
            check_first_expression_is_start_assertion(&program.expression);

        // update the program ports
        route.entry_node_index = in_node_index;
        route.exit_node_index = out_node_index;
        route.is_fixed_matching_begin_point = is_fixed_matching_begin_point;

        Ok(())
    }

    /// Compile an expression to a component
    fn emit_expression(&mut self, expression: &Expression) -> Result<Component, AnreError> {
        let result = match expression {
            Expression::Literal(literal) => self.emit_literal(literal)?,
            Expression::BackReference(back_reference) => self.emit_backreference(back_reference)?,
            Expression::Group(expressions) => self.emit_group(expressions)?,
            Expression::FunctionCall(function_call) => self.emit_function_call(function_call)?,
            Expression::Or(left, right) => self.emit_logic_or(left, right)?,
            Expression::IndexCapture(expression) => self.emit_indexed_capture(expression)?,
            Expression::NameCapture(name, expression) => {
                self.emit_named_capture(name, expression)?
            }
        };

        Ok(result)
    }

    fn emit_group(&mut self, expressions: &[Expression]) -> Result<Component, AnreError> {
        // Group component:
        //
        // ```diagram
        //   /-----------------------------------------------\
        //   |                                               |
        //   |    component        jump         component    |
        //   |  /-----------\   transition    /-----------\  |
        // =====o in    out o==-------------==o in    out o=====
        //   |  \-----------/                 \-----------/  |
        //   |                                               |
        //   \--------------- group component ---------------/
        // ```
        //
        // The "group" in ANRE is different from the "group" in traditional regular expressions.
        // In ANRE, a "group" is a series of expressions, it is used to group patterns and
        // modify operator precedence and associativity.
        // In terms of functionality, the "group" in ANRE is equivalent to the "non-capturing group"
        // in traditional regular expressions.

        let mut components = vec![];
        for expression in expressions {
            components.push(self.emit_expression(expression)?);
        }

        let compontent = if components.is_empty() {
            // empty expression
            self.emit_empty()?
        } else if components.len() == 1 {
            // single expression.
            // maybe a group also, so return the underlay component directly
            // to eliminates the nested group, e.g. '(((...)))'.
            components.pop().unwrap()
        } else {
            // multiple expressions, wrap them with a group component
            let route = self.get_current_route_ref_mut();
            for idx in 0..(components.len() - 1) {
                let current_out_node_index = components[idx].out_node_index;
                let next_in_node_index = components[idx + 1].in_node_index;
                let transition = Transition::Jump(JumpTransition);
                route.create_path(current_out_node_index, next_in_node_index, transition);
            }

            Component::new(
                components.first().unwrap().in_node_index,
                components.last().unwrap().out_node_index,
            )
        };

        Ok(compontent)
    }

    fn emit_logic_or(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> Result<Component, AnreError> {
        // Logic OR component:
        //
        // ```diagram
        //
        //   /-------------------------------------------------------\
        //   |                                                       |
        //   |            jump         left          jump            |
        //   |      transition |     component     | transition      |
        //   |                 v   /-----------\   v                 |
        //   |  in     /---------==o in    out o==--------\      out |
        //   |  node   |           \-----------/          |     node |
        // =====o|o==--/                                  |-----==o=====
        //   |   |o==--\                                  |          |
        //   |         |           /-----------\          |          |
        //   |         \---------==o in    out o==--------/          |
        //   |                 ^   \-----------/   ^                 |
        //   |            jump |      right        | jump            |
        //   |      transition |    component      | transition      |
        //   |                                                       |
        //   \------------------- logic or component ----------------/
        // ```

        let left_component = self.emit_expression(left)?;
        let right_component = self.emit_expression(right)?;

        let route = self.get_current_route_ref_mut();

        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        route.create_path(
            in_node_index,
            left_component.in_node_index,
            Transition::Jump(JumpTransition),
        );

        route.create_path(
            in_node_index,
            right_component.in_node_index,
            Transition::Jump(JumpTransition),
        );

        route.create_path(
            left_component.out_node_index,
            out_node_index,
            Transition::Jump(JumpTransition),
        );

        route.create_path(
            right_component.out_node_index,
            out_node_index,
            Transition::Jump(JumpTransition),
        );

        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_function_call(&mut self, function_call: &FunctionCall) -> Result<Component, AnreError> {
        match function_call.name {
            FunctionName::Optional
            | FunctionName::OneOrMore
            | FunctionName::ZeroOrMore
            | FunctionName::Repeat
            | FunctionName::RepeatRange
            | FunctionName::RepeatFrom
            | FunctionName::OptionalLazy
            | FunctionName::OneOrMoreLazy
            | FunctionName::ZeroOrMoreLazy
            | FunctionName::RepeatRangeLazy
            | FunctionName::RepeatFromLazy => {
                let args = &function_call.args;
                let FunctionArgument::Expression(expression) = &args[0] else {
                    unreachable!()
                };
                let numbers = args
                    .iter()
                    .skip(1)
                    .map(|arg| {
                        let FunctionArgument::Number(n) = arg else {
                            unreachable!()
                        };
                        *n
                    })
                    .collect::<Vec<_>>();

                self.emit_repetition_function_call(function_call.name, expression, &numbers)
            }
            FunctionName::IsBefore | FunctionName::IsNotBefore => {
                // look-ahead assertion
                // `A(?=B)` is equivalent to `is_before(A, B)` or `A.is_before(B)`.
                let args = &function_call.args;
                let FunctionArgument::Expression(expression) = &args[0] else {
                    unreachable!()
                };

                if args.len() < 2 {
                    return Err(AnreError::SyntaxIncorrect(
                        "Missing argument for look-ahead assertion.".to_owned(),
                    ));
                }
                let FunctionArgument::Expression(next_expression) = &args[1] else {
                    unreachable!()
                };

                let negative = function_call.name == FunctionName::IsNotBefore;
                self.emit_lookahead_assertion(expression, next_expression, negative)
            }
            FunctionName::IsAfter | FunctionName::IsNotAfter => {
                // look-behind assertion
                // `(?<=B)A` is equivalent to `is_after(A, B)` or `A.is_after(B)`.
                let args = &function_call.args;
                let FunctionArgument::Expression(expression) = &args[0] else {
                    unreachable!()
                };

                if args.len() < 2 {
                    return Err(AnreError::SyntaxIncorrect(
                        "Missing argument for look-behind assertion.".to_owned(),
                    ));
                }
                let FunctionArgument::Expression(previous_expression) = &args[1] else {
                    unreachable!()
                };

                let negative = function_call.name == FunctionName::IsNotAfter;
                self.emit_lookbehind_assertion(expression, previous_expression, negative)
            }
            FunctionName::IsStart => self.emit_line_boundary_assertion(false),
            FunctionName::IsEnd => self.emit_line_boundary_assertion(true),
            FunctionName::IsBound => self.emit_word_boundary_assertion(false),
            FunctionName::IsNotBound => self.emit_word_boundary_assertion(true),
        }
    }

    fn emit_repetition_function_call(
        &mut self,
        function_name: FunctionName,
        expression: &Expression,
        numbers: &[usize],
    ) -> Result<Component, AnreError> {
        let is_lazy = matches!(
            function_name,
            FunctionName::OptionalLazy
                | FunctionName::OneOrMoreLazy
                | FunctionName::ZeroOrMoreLazy
                | FunctionName::RepeatRangeLazy
                | FunctionName::RepeatFromLazy
        );

        match function_name {
            // Quantifier
            FunctionName::Optional | FunctionName::OptionalLazy => {
                // optional = {0,1}
                self.emit_optional(expression, is_lazy)
            }
            FunctionName::OneOrMore | FunctionName::OneOrMoreLazy => {
                // one_or_more = {1..}
                self.emit_repeat_from(expression, 1, is_lazy)
            }
            FunctionName::ZeroOrMore | FunctionName::ZeroOrMoreLazy => {
                // zero_or_more == optional(one_or_more) = {0..}
                let component = self.emit_repeat_from(expression, 1, is_lazy)?;
                self.continue_emit_optional(component, is_lazy)
            }
            FunctionName::Repeat => {
                let times = numbers[0];
                if times == 0 {
                    // {0} = shortcut
                    self.emit_empty()
                } else if times == 1 {
                    // {1} = non-repetition
                    self.emit_expression(expression)
                } else {
                    // {m} = repeat(m)
                    self.emit_repeat(expression, times)
                }
            }
            FunctionName::RepeatFrom | FunctionName::RepeatFromLazy => {
                let from = numbers[0];

                if from == 0 {
                    // {0..} == optional(one_or_more)
                    let component = self.emit_repeat_from(expression, 1, is_lazy)?;
                    self.continue_emit_optional(component, is_lazy)
                } else {
                    // {m..} == repeat_from(m)
                    self.emit_repeat_from(expression, from, is_lazy)
                }
            }
            FunctionName::RepeatRange | FunctionName::RepeatRangeLazy => {
                let from = numbers[0];
                let to = numbers[1];

                if from > to {
                    return Err(AnreError::SyntaxIncorrect(
                        "Repeated range values should be from small to large.".to_owned(),
                    ));
                }

                if from == 0 {
                    if to == 0 {
                        // {0..0} = {0} = shortcut
                        self.emit_empty()
                    } else if to == 1 {
                        // {0..1} = optional
                        self.emit_optional(expression, is_lazy)
                    } else {
                        // {0..m} = optional(repeat_range)
                        let component = self.emit_repeat_range(expression, 1, to, is_lazy)?;
                        self.continue_emit_optional(component, is_lazy)
                    }
                } else if to == 1 {
                    // {1..1} = {1} = non-repetition
                    self.emit_expression(expression)
                } else if from == to {
                    // {m..m} = {m} = repeat(m)
                    self.emit_repeat(expression, from)
                } else {
                    // {m..n} = repeat_range(m, n)
                    self.emit_repeat_range(expression, from, to, is_lazy)
                }
            }
            _ => {
                unreachable!()
            }
        }
    }

    /// Empty component only contains a unconditional jump transition.
    /// It is introduced by expressions such as `{0}`, `{0..0}`, etc.
    fn emit_empty(&mut self) -> Result<Component, AnreError> {
        // Empty component:
        //
        // ```diagram
        //   /-----------------------------\
        //   |          jump               |
        //   |        | transition         |
        //   |        v                    |
        // =====o==-------------------==o=====
        //   | in node            out node |
        //   |                             |
        //   \------ empty component ------/
        // ```

        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        route.create_path(
            in_node_index,
            out_node_index,
            Transition::Jump(JumpTransition),
        );
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_literal(&mut self, literal: &Literal) -> Result<Component, AnreError> {
        // Literal component:
        //
        // ```diagram
        //   /-----------------------------\
        //   |          literal            |
        //   |        | transition         |
        //   |        v                    |
        // =====o==-------------------==o=====
        //   | in node            out node |
        //   |                             |
        //   \----- literal component -----/
        // ```
        match literal {
            Literal::AnyChar => self.emit_literal_any_char(),
            Literal::Char(character) => self.emit_literal_char(*character),
            Literal::String(s) => self.emit_literal_string(s),
            Literal::CharSet(charset) => self.emit_literal_charset(charset),
            Literal::PresetCharSet(name) => self.emit_literal_preset_charset(name),
        }
    }

    fn emit_literal_char(&mut self, character: char) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let transition = Transition::Char(CharTransition::new(character));

        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_literal_any_char(&mut self) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let transition = Transition::AnyChar(AnyCharTransition);

        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_literal_string(&mut self, s: &str) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let transition = Transition::String(StringTransition::new(s));

        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_literal_preset_charset(
        &mut self,
        name: &PresetCharSetName,
    ) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        let charset_transition = match name {
            PresetCharSetName::CharWord => CharSetTransition::new_preset_charset_word(),
            PresetCharSetName::CharNotWord => CharSetTransition::new_preset_charset_not_word(),
            PresetCharSetName::CharSpace => CharSetTransition::new_preset_charset_space(),
            PresetCharSetName::CharNotSpace => CharSetTransition::new_preset_charset_not_space(),
            PresetCharSetName::CharDigit => CharSetTransition::new_preset_charset_digit(),
            PresetCharSetName::CharNotDigit => CharSetTransition::new_preset_charset_not_digit(),
        };

        let transition = Transition::CharSet(charset_transition);
        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_literal_charset(&mut self, charset: &CharSet) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        let mut items: Vec<CharSetItem> = vec![];
        append_charset(charset, &mut items)?;

        let transition = Transition::CharSet(CharSetTransition::new(items, charset.negative));
        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_line_boundary_assertion(&mut self, is_end: bool) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let transition =
            Transition::LineBoundaryAssertion(LineBoundaryAssertionTransition::new(is_end));

        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_word_boundary_assertion(&mut self, is_negative: bool) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let transition =
            Transition::WordBoundaryAssertion(WordBoundaryAssertionTransition::new(is_negative));

        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_backreference(
        &mut self,
        back_reference: &BackReference,
    ) -> Result<Component, AnreError> {
        match back_reference {
            BackReference::Index(index) => self.emit_backreference_by_index(*index),
            BackReference::Name(name) => self.emit_backreference_by_name(name),
        }
    }

    fn emit_backreference_by_index(
        &mut self,
        capture_group_index: usize,
    ) -> Result<Component, AnreError> {
        if capture_group_index >= self.map.capture_groups.len() {
            return Err(AnreError::SyntaxIncorrect(format!(
                "The group index ({}) of back-reference is out of range, the max index is: {}.",
                capture_group_index,
                self.map.capture_groups.len() - 1
            )));
        }

        self.continue_emit_backreference(capture_group_index)
    }

    fn emit_backreference_by_name(&mut self, name: &str) -> Result<Component, AnreError> {
        let capture_group_index_option = self.map.get_capture_group_index_by_name(name);
        let capture_group_index = if let Some(i) = capture_group_index_option {
            i
        } else {
            return Err(AnreError::SyntaxIncorrect(format!(
                "Cannot find the capture group with name: \"{}\".",
                name
            )));
        };

        self.continue_emit_backreference(capture_group_index)
    }

    fn continue_emit_backreference(
        &mut self,
        capture_group_index: usize,
    ) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let transition =
            Transition::BackReference(BackReferenceTransition::new(capture_group_index));

        route.create_path(in_node_index, out_node_index, transition);
        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_named_capture(
        &mut self,
        name: &str,
        expression: &Expression,
    ) -> Result<Component, AnreError> {
        self.continue_emit_capture(expression, Some(name.to_owned()))
    }

    fn emit_indexed_capture(&mut self, expression: &Expression) -> Result<Component, AnreError> {
        self.continue_emit_capture(expression, None)
    }

    fn continue_emit_capture(
        &mut self,
        expression: &Expression,
        name_option: Option<String>,
    ) -> Result<Component, AnreError> {
        let capture_group_index = self.map.create_capture_group(name_option);
        let component = self.emit_expression(expression)?;

        // Capture group:
        //
        // ```diagram
        //   /-------------------------------------------------\
        //   |                                                 |
        //   |  capture start                   capture end    |
        //   |  transition          inner       transition     |
        //   |       |            component       |            |
        //   |       v         /-------------\    V            |
        // =====o==----------==o in      out o==----------==o=====
        //   | in              \-------------/            out  |
        //   | node                                      node  |
        //   |                                                 |
        //   \---------------- capture component---------------/
        // ```

        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();
        let capture_start_transition = CaptureStartTransition::new(capture_group_index);
        let capture_end_transition = CaptureEndTransition::new(capture_group_index);

        route.create_path(
            in_node_index,
            component.in_node_index,
            Transition::CaptureStart(capture_start_transition),
        );

        route.create_path(
            component.out_node_index,
            out_node_index,
            Transition::CaptureEnd(capture_end_transition),
        );

        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_optional(
        &mut self,
        expression: &Expression,
        is_lazy: bool,
    ) -> Result<Component, AnreError> {
        // Greedy optional:
        //
        // ```diagram
        //   /-----------------------------------------------------\
        //   |                                                     |
        //   |              jump |       inner       | jump        |
        //   |  in    transition |     component     | transition  |
        //   |  node             v   /-----------\   v             |
        // =====o|o==--------------==o in    out o==----------==o=====
        //   |   |o==--\             \-----------/          out ^  |
        //   |         |                                   node |  |
        //   |         \----------------------------------------/  |
        //   |                       jump transition               |
        //   |                                                     |
        //   \--------------- greedy optional component -----------/
        // ```
        //
        // Lazy optional:
        //
        // ```diagram
        //   /-----------------------------------------------------\
        //   |                       jump transition               |
        //   |         /----------------------------------------\  |
        //   |  in     |                            jump        |  |
        //   |  node   |                          | transition  |  |
        // =====o|o==--/         /-----------\    v             v  |
        //   |   |o==----------==o in    out o==--------------==o=====
        //   |               ^   \-----------/                out  |
        //   |          jump |       inner                   node  |
        //   |    transition |     component                       |
        //   |                                                     |
        //   \---------------- lazy optional component ------------/
        // ```
        let component = self.emit_expression(expression)?;
        self.continue_emit_optional(component, is_lazy)
    }

    fn emit_repeat(
        &mut self,
        expression: &Expression,
        times: usize,
    ) -> Result<Component, AnreError> {
        // require `times > 1`
        self.continue_emit_repetition(expression, RepetitionType::Repeat(times), true)
    }

    fn emit_repeat_from(
        &mut self,
        expression: &Expression,
        from: usize,
        is_lazy: bool,
    ) -> Result<Component, AnreError> {
        // require `from > 0`
        self.continue_emit_repetition(expression, RepetitionType::RepeatFrom(from), is_lazy)
    }

    fn emit_repeat_range(
        &mut self,
        expression: &Expression,
        from: usize,
        to: usize,
        is_lazy: bool,
    ) -> Result<Component, AnreError> {
        // require `from > 0` and `to > from`
        self.continue_emit_repetition(expression, RepetitionType::RepeatRange(from, to), is_lazy)
    }

    fn continue_emit_optional(
        &mut self,
        component: Component,
        is_lazy: bool,
    ) -> Result<Component, AnreError> {
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        if is_lazy {
            route.create_path(
                in_node_index,
                out_node_index,
                Transition::Jump(JumpTransition),
            );
        }

        route.create_path(
            in_node_index,
            component.in_node_index,
            Transition::Jump(JumpTransition),
        );

        route.create_path(
            component.out_node_index,
            out_node_index,
            Transition::Jump(JumpTransition),
        );

        if !is_lazy {
            route.create_path(
                in_node_index,
                out_node_index,
                Transition::Jump(JumpTransition),
            );
        }

        Ok(Component::new(in_node_index, out_node_index))
    }

    fn continue_emit_repetition(
        &mut self,
        expression: &Expression,
        repetition_type: RepetitionType,
        is_lazy: bool,
    ) -> Result<Component, AnreError> {
        // Greedy repetition:
        //
        // ```diagram
        //    /-------------------------------------------------------------------------\
        //    |                                                                         |
        //    |                      repetition back transition                         |
        //    |              /--------------------------------------------\             |
        //    |              |                                            |             |
        //    |              |     | counter             | counter        |             |
        //    |              |     | save                | inc            |             |
        //    |              |     | transition          | transition     |             |
        //    |  in          |     |                     |                |             |
        //    |  node        v     v     /-----------\   v  right node    |       out   |
        //  =====o==-------==o==-------==o in    out o==------==o|o==-----/       node  |
        //    |          ^   left        \-----------/           |o==--------------==o=====
        //    |  counter |   node       inner component                   ^             |
        //    |  reset   |                                                | repetition  |
        //    |  transition                                               | forward     |
        //    |                                                           | transition  |
        //    |                                                                         |
        //    \--------------------- lazy repetition component -------------------------/
        // ```
        //
        // Lazy repetition:
        //
        // ```diagram
        //    /-------------------------------------------------------------------------\
        //    |                                                                         |
        //    |                    | counter             | counter                      |
        //    |                    | save                | inc                          |
        //    |                    | transition          | transition                   |
        //    |  in        left    |                     |                        out   |
        //    |  node      node    v     /-----------\   v  right node            node  |
        //  =====o==-------==o==-------==o in    out o==------==o|o==--------------==o=====
        //    |          ^   ^           \-----------/           |o==--\  ^             |
        //    |  counter |   |          inner component                |  | repetition  |
        //    |  reset   |   |                                         |  | forward     |
        //    |  transition  \-----------------------------------------/  | transition  |
        //    |                      repetition back transition                         |
        //    |                                                                         |
        //    \--------------------- lazy repetition component -------------------------/
        // ```

        let component = self.emit_expression(expression)?;

        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let left_node_index = route.create_node();

        route.create_path(
            in_node_index,
            left_node_index,
            Transition::CounterReset(CounterResetTransition),
        );

        route.create_path(
            left_node_index,
            component.in_node_index,
            Transition::CounterSave(CounterSaveTransition),
        );

        let right_node_index = route.create_node();

        route.create_path(
            component.out_node_index,
            right_node_index,
            Transition::CounterInc(CounterIncTransition),
        );

        let out_node_index = route.create_node();

        if is_lazy {
            route.create_path(
                right_node_index,
                out_node_index,
                Transition::RepetitionForward(RepetitionForwardTransition::new(
                    repetition_type.clone(),
                )),
            );

            route.create_path(
                right_node_index,
                left_node_index,
                Transition::RepetitionBack(RepetitionBackTransition::new(repetition_type)),
            );
        } else {
            route.create_path(
                right_node_index,
                left_node_index,
                Transition::RepetitionBack(RepetitionBackTransition::new(repetition_type.clone())),
            );

            route.create_path(
                right_node_index,
                out_node_index,
                Transition::RepetitionForward(RepetitionForwardTransition::new(repetition_type)),
            );
        }

        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_lookahead_assertion(
        &mut self,
        current_expression: &Expression,
        next_expression: &Expression,
        negative: bool,
    ) -> Result<Component, AnreError> {
        // Look ahead assertions:
        //
        // - `A(?=B)`: `is_before(A, B)` or `A.is_before(B)`
        // - `A(?!B)`: `is_not_before(A, B)` or `A.is_not_before(B)`,
        //
        // ```diagram
        //   /----------------------------------------------\
        //   |                                              |
        //   |                  inner         | lookahead   |
        //   |  in             component      | transition  |
        //   |  node         /-----------\    v             |
        // =====o==--------==o in  out   o==-----------==o=====
        //   |       jump    \-----------/             out  |
        //   |    transition                          node  |
        //   |                                              |
        //   \------------- lookahead assertion component --/
        // ```

        let component = self.emit_expression(current_expression)?;

        // 1. save the current route index
        let saved_route_index = self.get_current_route_index();

        // 2. create new route
        let sub_route_index = self.map.create_route();

        // 3. switch to the new route
        self.set_current_route_index(sub_route_index);

        // 4. compile the next expression in the new route
        let sub_component = self.emit_expression(next_expression)?;

        let sub_route = self.get_current_route_ref_mut();
        sub_route.entry_node_index = sub_component.in_node_index;
        sub_route.exit_node_index = sub_component.out_node_index;
        sub_route.is_fixed_matching_begin_point = true;

        // 5. restore to the previous route
        self.set_current_route_index(saved_route_index);

        // 6. join the sub_route to the current route by
        //    appending jump transitions around the sub-component.
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        route.create_path(
            in_node_index,
            component.in_node_index,
            Transition::Jump(JumpTransition),
        );

        route.create_path(
            component.out_node_index,
            out_node_index,
            Transition::LookAheadAssertion(LookAheadAssertionTransition::new(
                sub_route_index,
                negative,
            )),
        );

        Ok(Component::new(in_node_index, out_node_index))
    }

    fn emit_lookbehind_assertion(
        &mut self,
        current_expression: &Expression,
        previous_expression: &Expression,
        negative: bool,
    ) -> Result<Component, AnreError> {
        // Look behind assertions:
        //
        // - `is_after(A, B)`, `A.is_after(B)`, `(?<=B)A`
        // - `is_not_after(A, B)`, `A.is_not_after(B)`, `(?<!B)A`
        //
        // ```diagram
        //   /-----------------------------------------------\
        //   |                                               |
        //   |       | lookbehind                            |
        //   |  in   | transition                jump        |
        //   |  node v           /-----------\   transition  |
        // =====o==------------==o in  out   o==--------==o=====
        //   |                   \-----------/          out  |
        //   |                  inner component        node  |
        //   |                                               |
        //   \-------- lookbehind assertion component -------/
        // ```

        // 1. save the current route index
        let saved_route_index = self.get_current_route_index();

        // 2. create new route
        let sub_route_index = self.map.create_route();

        // 3. switch to the new route
        self.set_current_route_index(sub_route_index);

        // 4. calculate the total length (in char) of previous expression
        let match_length_enum = calculate_match_length(previous_expression);
        let MatchLength::Fixed(match_length) = match_length_enum else {
            return Err(AnreError::SyntaxIncorrect(
                "Look behind assertion (is_after, is_not_after) requires a fixed length pattern."
                    .to_owned(),
            ));
        };

        // 5. compile the previous expression in the new route
        let sub_component = self.emit_expression(previous_expression)?;
        let sub_route = self.get_current_route_ref_mut();

        sub_route.entry_node_index = sub_component.in_node_index;
        sub_route.exit_node_index = sub_component.out_node_index;
        sub_route.is_fixed_matching_begin_point = true;

        // 6. restore to the previous route
        self.set_current_route_index(saved_route_index);

        // 7. join the sub_route to the current route by
        // appending jump transitions around the sub-component.
        let component = self.emit_expression(current_expression)?;
        let route = self.get_current_route_ref_mut();
        let in_node_index = route.create_node();
        let out_node_index = route.create_node();

        route.create_path(
            in_node_index,
            component.in_node_index,
            Transition::LookBehindAssertion(LookBehindAssertionTransition::new(
                sub_route_index,
                negative,
                match_length,
            )),
        );

        route.create_path(
            component.out_node_index,
            out_node_index,
            Transition::Jump(JumpTransition),
        );

        Ok(Component::new(in_node_index, out_node_index))
    }
}

fn check_first_expression_is_start_assertion(expression: &Expression) -> bool {
    match expression {
        Expression::Group(exps) => {
            if let Some(first_exp) = exps.first()
                && check_first_expression_is_start_assertion(first_exp)
            {
                return true;
            } else {
                return false;
            }
        }
        Expression::NameCapture(_, exp) => {
            if check_first_expression_is_start_assertion(exp) {
                return true;
            }
            false
        }
        Expression::IndexCapture(exp) => {
            if check_first_expression_is_start_assertion(exp) {
                return true;
            }
            false
        }
        Expression::FunctionCall(func) if func.name == FunctionName::IsStart => true,
        _ => false,
    }
}

fn append_charset(charset: &CharSet, items: &mut Vec<CharSetItem>) -> Result<(), AnreError> {
    for element in &charset.elements {
        match element {
            CharSetElement::Char(c) => add_char(items, *c),
            CharSetElement::CharRange(CharRange {
                start,
                end_inclusive,
            }) => add_range(items, *start, *end_inclusive),
            CharSetElement::PresetCharSet(name) => {
                append_preset_charset(name, items)?;
            }
            CharSetElement::CharSet(custom_charset) => {
                if custom_charset.negative {
                    return Err(AnreError::SyntaxIncorrect(
                        "Negative custom charset cannot be nested in another charset.".to_owned(),
                    ));
                }
                append_charset(custom_charset, items)?;
            }
        }
    }

    Ok(())
}

fn append_preset_charset(
    name: &PresetCharSetName,
    items: &mut Vec<CharSetItem>,
) -> Result<(), AnreError> {
    match name {
        PresetCharSetName::CharWord => {
            add_preset_word(items);
        }
        PresetCharSetName::CharSpace => {
            add_preset_space(items);
        }
        PresetCharSetName::CharDigit => {
            add_preset_digit(items);
        }
        _ => {
            return Err(AnreError::SyntaxIncorrect(format!(
                "Can not append negative preset charset \"{}\" into charset.",
                name
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;

    use crate::{
        error::AnreError,
        object_file::{MAIN_ROUTE_INDEX, Map},
    };

    use super::{compile_from_anre, compile_from_regex};

    fn generate_routes(anre: &str, regex: &str) -> [Map; 2] {
        [
            compile_from_anre(anre).unwrap(),
            compile_from_regex(regex).unwrap(),
        ]
    }

    #[test]
    fn test_compile_char() {
        // single char
        for route in generate_routes(r#"'a'"#, r#"a"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // sequence chars
        {
            let route = compile_from_anre(r#"('a', 'b', 'c')"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 4, Jump
- 4
  -> 5, Char 'c'
- 5
  -> 7, Capture end {0}
> 6
  -> 0, Capture start {0}
< 7
# {0}"
            );
        }

        // group
        // note: the group in ANRE is different from it is in traditional regex,
        // it is only a sequence pattern.
        {
            let route = compile_from_anre(r#"('a',('b','c'), 'd')"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 4, Jump
- 4
  -> 5, Char 'c'
- 5
  -> 6, Jump
- 6
  -> 7, Char 'd'
- 7
  -> 9, Capture end {0}
> 8
  -> 0, Capture start {0}
< 9
# {0}"
            );
        }

        // nested groups
        {
            let route = compile_from_anre(r#"('a',('b', ('c', 'd'), 'e'), 'f')"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 4, Jump
- 4
  -> 5, Char 'c'
- 5
  -> 6, Jump
- 6
  -> 7, Char 'd'
- 7
  -> 8, Jump
- 8
  -> 9, Char 'e'
- 9
  -> 10, Jump
- 10
  -> 11, Char 'f'
- 11
  -> 13, Capture end {0}
> 12
  -> 0, Capture start {0}
< 13
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_string() {
        for route in generate_routes(r#""文✨🦛""#, r#"文✨🦛"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, String \"文✨🦛\"
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_special_char() {
        for route in generate_routes(r#"('a', char_any)"#, r#"a."#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Any char
- 3
  -> 5, Capture end {0}
> 4
  -> 0, Capture start {0}
< 5
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_preset_charset() {
        // positive preset charset
        for route in generate_routes(r#"('a', char_word, char_space, char_digit)"#, r#"a\w\s\d"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Charset ['A'..'Z', 'a'..'z', '0'..'9', '_']
- 3
  -> 4, Jump
- 4
  -> 5, Charset [' ', '\\t', '\\r', '\\n']
- 5
  -> 6, Jump
- 6
  -> 7, Charset ['0'..'9']
- 7
  -> 9, Capture end {0}
> 8
  -> 0, Capture start {0}
< 9
# {0}"
            );
        }

        // negative preset charset
        for route in generate_routes(
            r#"('a', char_not_word, char_not_space, char_not_digit)"#,
            r#"a\W\S\D"#,
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Charset !['A'..'Z', 'a'..'z', '0'..'9', '_']
- 3
  -> 4, Jump
- 4
  -> 5, Charset ![' ', '\\t', '\\r', '\\n']
- 5
  -> 6, Jump
- 6
  -> 7, Charset !['0'..'9']
- 7
  -> 9, Capture end {0}
> 8
  -> 0, Capture start {0}
< 9
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_charset() {
        // contains char and range
        for route in generate_routes(r#"['a', '0'..'7']"#, r#"[a0-7]"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset ['a', '0'..'7']
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // negative charset
        for route in generate_routes(r#"!['a','0'..'7']"#, r#"[^a0-7]"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset !['a', '0'..'7']
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // contains preset charset
        for route in generate_routes(r#"[char_word, char_space]"#, r#"[\w\s]"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset ['A'..'Z', 'a'..'z', '0'..'9', '_', ' ', '\\t', '\\r', '\\n']
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // nested charset
        {
            let route = compile_from_anre(r#"['a', ['x'..'z']]"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset ['a', 'x'..'z']
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // nested charset 2
        {
            let route = compile_from_anre(r#"[['+', '-'], ['0'..'9', ['a'..'f']]]"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset ['+', '-', '0'..'9', 'a'..'f']
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // marcos
        {
            let route = compile_from_anre(
                r#"
define prefix (['+', '-'])
define letter (['a'..'f'])
[prefix, ['0'..'9'], letter]"#,
            )
            .unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset ['+', '-', '0'..'9', 'a'..'f']
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // err: negative preset charset in charset
        {
            assert!(matches!(
                compile_from_anre(r#"[char_not_word]"#),
                Err(AnreError::SyntaxIncorrect(_))
            ));
        }

        // err: negative charset in charset
        {
            assert!(matches!(
                compile_from_anre(r#"['+', !['a'..'f']]"#),
                Err(AnreError::SyntaxIncorrect(_))
            ));
        }
    }

    #[test]
    fn test_compile_logic_or() {
        for route in generate_routes(r#"'a' || 'b'"#, r#"a|b"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 5, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 5, Jump
- 4
  -> 0, Jump
  -> 2, Jump
- 5
  -> 7, Capture end {0}
> 6
  -> 4, Capture start {0}
< 7
# {0}"
            );
        }

        // multiple operands
        //
        // Note: "'a' || 'b' || 'c'" => "'a' || ('b' || 'c')"
        for route in generate_routes(r#"'a' || 'b' || 'c'"#, r#"a|b|c"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 9, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 7, Jump
- 4
  -> 5, Char 'c'
- 5
  -> 7, Jump
- 6
  -> 2, Jump
  -> 4, Jump
- 7
  -> 9, Jump
- 8
  -> 0, Jump
  -> 6, Jump
- 9
  -> 11, Capture end {0}
> 10
  -> 8, Capture start {0}
< 11
# {0}"
            );
        }

        // group and logic or (change precedence)
        for route in generate_routes(r#"('a' || 'b') || 'c'"#, r#"(?:a|b)|c"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 5, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 5, Jump
- 4
  -> 0, Jump
  -> 2, Jump
- 5
  -> 9, Jump
- 6
  -> 7, Char 'c'
- 7
  -> 9, Jump
- 8
  -> 4, Jump
  -> 6, Jump
- 9
  -> 11, Capture end {0}
> 10
  -> 8, Capture start {0}
< 11
# {0}"
            );
        }

        // group and logic or
        {
            let route = compile_from_anre(r#"('a', 'b') || 'c'"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 7, Jump
- 4
  -> 5, Char 'c'
- 5
  -> 7, Jump
- 6
  -> 0, Jump
  -> 4, Jump
- 7
  -> 9, Capture end {0}
> 8
  -> 6, Capture start {0}
< 9
# {0}"
            );
        }

        // operator precedence
        //
        // Note: "'a', 'b' || 'c', 'd'" => "'a', ('b' || 'c'), 'd'"
        for route in generate_routes(r#"('a', 'b' || 'c', 'd')"#, r#"a(?:b|c)d"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 6, Jump
- 2
  -> 3, Char 'b'
- 3
  -> 7, Jump
- 4
  -> 5, Char 'c'
- 5
  -> 7, Jump
- 6
  -> 2, Jump
  -> 4, Jump
- 7
  -> 8, Jump
- 8
  -> 9, Char 'd'
- 9
  -> 11, Capture end {0}
> 10
  -> 0, Capture start {0}
< 11
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_boundary_assertion() {
        for route in generate_routes(r#"(is_start(), is_bound(), 'a')"#, r#"^\ba"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Line boundary assertion is_start()
- 1
  -> 2, Jump
- 2
  -> 3, Word boundary assertion is_bound()
- 3
  -> 4, Jump
- 4
  -> 5, Char 'a'
- 5
  -> 7, Capture end {0}
> 6
  -> 0, Capture start {0}
< 7
# {0}"
            );

            // check 'is_fixed_matching_begin_point'
            assert!(route.routes[MAIN_ROUTE_INDEX].is_fixed_matching_begin_point);
        }

        for route in generate_routes(r#"(is_not_bound(), 'a', is_end())"#, r#"\Ba$"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Word boundary assertion is_not_bound()
- 1
  -> 2, Jump
- 2
  -> 3, Char 'a'
- 3
  -> 4, Jump
- 4
  -> 5, Line boundary assertion is_end()
- 5
  -> 7, Capture end {0}
> 6
  -> 0, Capture start {0}
< 7
# {0}"
            );

            // check the 'fixed_start_position' property
            assert!(!route.routes[MAIN_ROUTE_INDEX].is_fixed_matching_begin_point);
        }
    }

    #[test]
    fn test_compile_capture_group_by_name() {
        for route in generate_routes(r#"('a' as foo, 'b' as bar)"#, r#"(?<foo>a)(?<bar>b)"#) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Capture end {1}
- 2
  -> 0, Capture start {1}
- 3
  -> 6, Jump
- 4
  -> 5, Char 'b'
- 5
  -> 7, Capture end {2}
- 6
  -> 4, Capture start {2}
- 7
  -> 9, Capture end {0}
> 8
  -> 2, Capture start {0}
< 9
# {0}
# {1}, foo
# {2}, bar"
            );
        }

        for route in generate_routes(
            r#"(('a', char_digit) as foo, ('x' || 'y') as bar)"#,
            r#"(?<foo>a\d)(?<bar>(?:x|y))"#,
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 2, Jump
- 2
  -> 3, Charset ['0'..'9']
- 3
  -> 5, Capture end {1}
- 4
  -> 0, Capture start {1}
- 5
  -> 12, Jump
- 6
  -> 7, Char 'x'
- 7
  -> 11, Jump
- 8
  -> 9, Char 'y'
- 9
  -> 11, Jump
- 10
  -> 6, Jump
  -> 8, Jump
- 11
  -> 13, Capture end {2}
- 12
  -> 10, Capture start {2}
- 13
  -> 15, Capture end {0}
> 14
  -> 4, Capture start {0}
< 15
# {0}
# {1}, foo
# {2}, bar"
            );
        }

        {
            let route = compile_from_anre(r#"('a' as foo) as bar"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Capture end {2}
- 2
  -> 0, Capture start {2}
- 3
  -> 5, Capture end {1}
- 4
  -> 2, Capture start {1}
- 5
  -> 7, Capture end {0}
> 6
  -> 4, Capture start {0}
< 7
# {0}
# {1}, bar
# {2}, foo"
            );
        }
    }

    #[test]
    fn test_compile_capture_group_by_index() {
        for route in generate_routes(
            r#"(#'a', #('b', char_digit))"#, // anre
            r#"(a)(b\d)"#,                   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Capture end {1}
- 2
  -> 0, Capture start {1}
- 3
  -> 8, Jump
- 4
  -> 5, Char 'b'
- 5
  -> 6, Jump
- 6
  -> 7, Charset ['0'..'9']
- 7
  -> 9, Capture end {2}
- 8
  -> 4, Capture start {2}
- 9
  -> 11, Capture end {0}
> 10
  -> 2, Capture start {0}
< 11
# {0}
# {1}
# {2}"
            );
        }
    }

    #[test]
    fn test_compile_backreference() {
        for route in generate_routes(
            r#"('a' as foo, 'b', foo)"#, // anre
            r#"(?<foo>a)b\k<foo>"#,      // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Capture end {1}
- 2
  -> 0, Capture start {1}
- 3
  -> 4, Jump
- 4
  -> 5, Char 'b'
- 5
  -> 6, Jump
- 6
  -> 7, Back reference {1}
- 7
  -> 9, Capture end {0}
> 8
  -> 2, Capture start {0}
< 9
# {0}
# {1}, foo"
            );
        }

        for route in generate_routes(
            r#"(#char_word, 'x', ^1)"#, // anre
            r#"(\w)x\1"#,               // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Charset ['A'..'Z', 'a'..'z', '0'..'9', '_']
- 1
  -> 3, Capture end {1}
- 2
  -> 0, Capture start {1}
- 3
  -> 4, Jump
- 4
  -> 5, Char 'x'
- 5
  -> 6, Jump
- 6
  -> 7, Back reference {1}
- 7
  -> 9, Capture end {0}
> 8
  -> 2, Capture start {0}
< 9
# {0}
# {1}"
            );
        }
    }

    #[test]
    fn test_compile_optional() {
        // greedy
        for route in generate_routes(
            r#"'a'?"#, // anre
            r#"a?"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Jump
- 2
  -> 0, Jump
  -> 3, Jump
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
# {0}"
            );
        }

        // lazy
        for route in generate_routes(
            r#"'a'??"#, // anre
            r#"a??"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Jump
- 2
  -> 3, Jump
  -> 0, Jump
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_repeat() {
        // repeat 2
        for route in generate_routes(
            r#"'a'{2}"#, // anre
            r#"a{2}"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 5, Repetition forward [2]
  -> 3, Repetition back [2]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }

        // repeat 1
        for route in generate_routes(
            r#"'a'{1}"#, // anre
            r#"a{1}"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }

        // repeat 0
        for route in generate_routes(
            r#"'a'{0}"#, // anre
            r#"a{0}"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Jump
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_repeat_from() {
        // {m,}
        for route in generate_routes(
            r#"'a'{3..}"#, // anre
            r#"a{3,}"#,    // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 3, Repetition back [3..]
  -> 5, Repetition forward [3..]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }

        // lazy
        for route in generate_routes(
            r#"'a'{3..}?"#, // anre
            r#"a{3,}?"#,    // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 5, Repetition forward [3..]
  -> 3, Repetition back [3..]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }

        // {1,} == one_or_more
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{1..}"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'+"#).unwrap().get_debug_text()
            );
        }

        // {1,}? == lazy one_or_more
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{1..}?"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'+?"#).unwrap().get_debug_text()
            );
        }

        // {0,} == zero_or_more
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{0..}"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'*"#).unwrap().get_debug_text()
            );
        }

        // {0,}? == lazy zero_or_more
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{0..}?"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'*?"#).unwrap().get_debug_text()
            );
        }
    }

    #[test]
    fn test_compile_repeat_range() {
        // greedy
        for route in generate_routes(
            r#"'a'{3..5}"#, // anre
            r#"a{3,5}"#,    // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 3, Repetition back [3..5]
  -> 5, Repetition forward [3..5]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }

        // lazy
        for route in generate_routes(
            r#"'a'{3..5}?"#, // anre
            r#"a{3,5}?"#,    // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 5, Repetition forward [3..5]
  -> 3, Repetition back [3..5]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }

        // {m, m}
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{3..3}"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'{3}"#).unwrap().get_debug_text()
            )
        }

        // {1, 1}
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{1..1}"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'"#).unwrap().get_debug_text()
            )
        }

        // {0, m}
        for route in generate_routes(
            r#"'a'{0..5}"#, // anre
            r#"a{0,5}"#,    // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 3, Repetition back [1..5]
  -> 5, Repetition forward [1..5]
- 5
  -> 7, Jump
- 6
  -> 2, Jump
  -> 7, Jump
- 7
  -> 9, Capture end {0}
> 8
  -> 6, Capture start {0}
< 9
# {0}"
            );
        }

        // {0, m} lazy
        for route in generate_routes(
            r#"'a'{0..5}?"#, // anre
            r#"a{0,5}?"#,    // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 5, Repetition forward [1..5]
  -> 3, Repetition back [1..5]
- 5
  -> 7, Jump
- 6
  -> 7, Jump
  -> 2, Jump
- 7
  -> 9, Capture end {0}
> 8
  -> 6, Capture start {0}
< 9
# {0}"
            );
        }

        // {0, 1}
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{0..1}"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'?"#).unwrap().get_debug_text()
            )
        }

        // {0, 1} lazy
        {
            assert_str_eq!(
                compile_from_anre(r#"'a'{0..1}?"#).unwrap().get_debug_text(),
                compile_from_anre(r#"'a'??"#).unwrap().get_debug_text()
            )
        }

        // {0, 0}
        {
            let route = compile_from_anre(r#"'a'{0..0}"#).unwrap();
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Jump
- 1
  -> 3, Capture end {0}
> 2
  -> 0, Capture start {0}
< 3
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_notation_optional() {
        // optional
        for route in generate_routes(
            r#"'a'?"#, // anre
            r#"a?"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Jump
- 2
  -> 0, Jump
  -> 3, Jump
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
# {0}"
            );
        }

        // lazy optional
        for route in generate_routes(
            r#"'a'??"#, // anre
            r#"a??"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 3, Jump
- 2
  -> 3, Jump
  -> 0, Jump
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_notation_one_or_more() {
        // one or more
        for route in generate_routes(
            r#"'a'+"#, // anre
            r#"a+"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 3, Repetition back [1..]
  -> 5, Repetition forward [1..]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }

        // lazy one or more
        for route in generate_routes(
            r#"'a'+?"#, // anre
            r#"a+?"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 5, Repetition forward [1..]
  -> 3, Repetition back [1..]
- 5
  -> 7, Capture end {0}
> 6
  -> 2, Capture start {0}
< 7
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_notation_zero_or_more() {
        // zero or more
        for route in generate_routes(
            r#"'a'*"#, // anre
            r#"a*"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 3, Repetition back [1..]
  -> 5, Repetition forward [1..]
- 5
  -> 7, Jump
- 6
  -> 2, Jump
  -> 7, Jump
- 7
  -> 9, Capture end {0}
> 8
  -> 6, Capture start {0}
< 9
# {0}"
            );
        }

        // lazy zero or more
        for route in generate_routes(
            r#"'a'*?"#, // anre
            r#"a*?"#,   // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
- 0
  -> 1, Char 'a'
- 1
  -> 4, Counter inc
- 2
  -> 3, Counter reset
- 3
  -> 0, Counter save
- 4
  -> 5, Repetition forward [1..]
  -> 3, Repetition back [1..]
- 5
  -> 7, Jump
- 6
  -> 7, Jump
  -> 2, Jump
- 7
  -> 9, Capture end {0}
> 8
  -> 6, Capture start {0}
< 9
# {0}"
            );
        }
    }

    #[test]
    fn test_compile_is_before() {
        // positive
        for route in generate_routes(
            r#"'a'.is_before("xyz")"#, // anre
            r#"a(?=xyz)"#,             // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
= $0
- 0
  -> 1, Char 'a'
- 1
  -> 3, Look ahead $1
- 2
  -> 0, Jump
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
= $1
> 0
  -> 1, String \"xyz\"
< 1
# {0}"
            );
        }

        // negative
        for route in generate_routes(
            r#"'a'.is_not_before("xyz")"#, // anre
            r#"a(?!xyz)"#,                 // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
= $0
- 0
  -> 1, Char 'a'
- 1
  -> 3, Look ahead negative $1
- 2
  -> 0, Jump
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
= $1
> 0
  -> 1, String \"xyz\"
< 1
# {0}"
            );
        }

        // err: syntax error
        {
            assert!(matches!(
                compile_from_anre(r#"'a'.is_before()"#),
                Err(AnreError::SyntaxIncorrect(_))
            ));
        }
    }

    #[test]
    fn test_compile_is_after() {
        // positive
        for route in generate_routes(
            r#"'a'.is_after("xyz")"#, // anre
            r#"(?<=xyz)a"#,           // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
= $0
- 0
  -> 1, Char 'a'
- 1
  -> 3, Jump
- 2
  -> 0, Look behind $1, match length 3
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
= $1
> 0
  -> 1, String \"xyz\"
< 1
# {0}"
            );
        }

        // negative
        for route in generate_routes(
            r#"'a'.is_not_after("xyz")"#, // anre
            r#"(?<!xyz)a"#,               // regex
        ) {
            let s = route.get_debug_text();

            assert_str_eq!(
                s,
                "\
= $0
- 0
  -> 1, Char 'a'
- 1
  -> 3, Jump
- 2
  -> 0, Look behind negative $1, match length 3
- 3
  -> 5, Capture end {0}
> 4
  -> 2, Capture start {0}
< 5
= $1
> 0
  -> 1, String \"xyz\"
< 1
# {0}"
            );
        }

        // err: syntax error
        {
            assert!(matches!(
                compile_from_anre(r#"'a'.is_after()"#),
                Err(AnreError::SyntaxIncorrect(_))
            ));
        }

        // err: variable length
        {
            assert!(matches!(
                compile_from_anre(r#"'a'.is_after("x" || "yz")"#),
                Err(AnreError::SyntaxIncorrect(_))
            ));

            assert!(matches!(
                compile_from_anre(r#"'a'.is_after("x"+)"#),
                Err(AnreError::SyntaxIncorrect(_))
            ));
        }
    }
}
