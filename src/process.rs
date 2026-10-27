// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{
    context::{Context, MatchRange, Routine},
    object::{MAIN_ROUTE_INDEX, Map},
    transition::{CharSetItem, RepetitionType, Transition},
    utf8_codepoint_reader::{next_codepoint, previous_codepoint},
};

/// Searches the main route from a byte offset, discarding state from the previous search.
pub fn start_process(context: &mut Context, object: &Map, start_position: usize) -> bool {
    // The end position is the length of the source text.
    let end = context.bytes.len();

    context.reset();

    // Start the main routine for matching.
    start_routine(context, object, MAIN_ROUTE_INDEX, start_position, end)
}

/// Evaluates a route within a byte range, suspending any parent lookaround routine.
pub fn start_routine(
    context: &mut Context,
    map: &Map,
    route_index: usize,
    start_position: usize, // Inclusive byte offset of the search range.
    end_position: usize,   // Exclusive byte offset of the search range.
) -> bool {
    let mut result = false;
    let mut cursor = start_position;

    // Add a routine for the route to the context.
    let routine = Routine::new(start_position, end_position, route_index);
    context.routines.push(routine);

    // Include the end position because zero-width patterns may match there.
    loop {
        if execute_transitions(context, map, cursor) {
            result = true;
            break;
        }

        // Anchored routes and lookaround subroutes must not search later positions.
        if map.routes[route_index].is_fixed_cursor_begin_position || cursor >= end_position {
            break;
        }

        // Move forward by one character and try again.
        let (_, byte_length) = next_codepoint(context.bytes, cursor);
        cursor += byte_length;
    }

    // Remove the routine for the route from the context.
    context.routines.pop();
    result
}

/// Tries paths from `cursor` until one reaches the exit node or all alternatives fail.
fn execute_transitions(context: &mut Context, map: &Map, cursor: usize) -> bool {
    let (route_index, entry_node_index, exit_node_index) = {
        let routine = context.get_current_routine_ref();
        let route_index = routine.route_index;
        let route = &map.routes[route_index];
        (route_index, route.entry_node_index, route.exit_node_index)
    };

    // Add transitions of the entry node.
    let undo_log_length = context.undo_log.len();
    context.push_transitions_of_node(map, entry_node_index, cursor, 0);

    // Pending alternatives preserve both the cursor and the state needed for backtracking.
    while let Some(frame) = context.pop_transition_stack_item() {
        // Discard slot changes made after this frame was pushed.
        context.rollback(frame.undo_log_length);

        let route = &map.routes[route_index];
        let node = &route.nodes[frame.current_node_index];
        let transition_item = &node.path[frame.transition_index];

        let last_cursor = frame.cursor;
        let last_repetition_count = frame.repetition_count;
        let transition = &transition_item.transition;
        let target_node_index = transition_item.target_node_index;

        let execute_result = transition.execute(context, map, last_cursor, last_repetition_count);

        match execute_result {
            ExecuteResult::Success(move_forward_in_bytes, current_repetition_count) => {
                if target_node_index == exit_node_index {
                    // Reached the exit node of the route, meaning the route matching succeeded.
                    return true;
                }

                // Add transitions of the target node.
                context.push_transitions_of_node(
                    map,
                    target_node_index,
                    last_cursor + move_forward_in_bytes,
                    current_repetition_count,
                );
            }
            ExecuteResult::Failure => {
                // Check the next transition.
            }
        }
    }

    // All transitions failed, meaning the route matching failed.
    context.rollback(undo_log_length);
    false
}

impl Transition {
    /// Evaluates a transition using the cursor and count restored from its stack frame.
    pub fn execute(
        &self,
        context: &mut Context,
        map: &Map,

        // Byte offset in the source text.
        cursor: usize,

        // Count used by repetition entry and exit checks.
        repetition_count: usize,
    ) -> ExecuteResult {
        match self {
            Transition::Jump(_) => {
                // Jumps consume no input and cannot fail.
                ExecuteResult::Success(0, 0)
            }
            Transition::Char(transition) => {
                let routine = context.get_current_routine_ref();

                if cursor >= routine.range_end {
                    ExecuteResult::Failure
                } else {
                    let (cp, _) = next_codepoint(context.bytes, cursor);
                    if cp == transition.codepoint {
                        ExecuteResult::Success(transition.byte_length, 0)
                    } else {
                        ExecuteResult::Failure
                    }
                }
            }
            Transition::AnyChar(_) => {
                let routine = context.get_current_routine_ref();

                if cursor >= routine.range_end {
                    ExecuteResult::Failure
                } else {
                    let (current_char, byte_length) = get_char(context.bytes, cursor);

                    // `char_any` excludes LF and CR, but not other Unicode line separators.
                    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_expressions/Character_classes
                    if current_char != '\n' as u32 && current_char != '\r' as u32 {
                        ExecuteResult::Success(byte_length, 0)
                    } else {
                        ExecuteResult::Failure
                    }
                }
            }
            Transition::String(transition) => {
                let routine = context.get_current_routine_ref();

                if cursor + transition.byte_length > routine.range_end {
                    ExecuteResult::Failure
                } else {
                    let mut all_code_point_matched = true;
                    let mut current_position: usize = cursor;

                    for expected_codepoint in &transition.codepoints {
                        let (actual_code_point, length) =
                            next_codepoint(context.bytes, current_position);
                        if *expected_codepoint != actual_code_point {
                            all_code_point_matched = false;
                            break;
                        }
                        current_position += length;
                    }

                    if all_code_point_matched {
                        ExecuteResult::Success(transition.byte_length, 0)
                    } else {
                        ExecuteResult::Failure
                    }
                }
            }
            Transition::CharSet(transition) => {
                let routine = context.get_current_routine_ref();

                if cursor >= routine.range_end {
                    return ExecuteResult::Failure;
                }

                let (current_char, byte_length) = get_char(context.bytes, cursor);
                let mut found_any: bool = false;

                for item in &transition.items {
                    found_any = match item {
                        CharSetItem::Char(c) => current_char == *c,
                        CharSetItem::Range(r) => {
                            current_char >= r.start && current_char <= r.end_inclusive
                        }
                    };

                    if found_any {
                        break;
                    }
                }

                if found_any ^ transition.negative {
                    ExecuteResult::Success(byte_length, 0)
                } else {
                    ExecuteResult::Failure
                }
            }
            Transition::BackReference(transition) => {
                // A group that did not participate in the match is treated as empty.
                let (start, end) = match &context.matched_slots[transition.capture_group_index] {
                    Some(MatchRange { start, end }) => (*start, *end),
                    None => (0, 0),
                };

                let bytes = &context.bytes[start..end];
                let length_in_byte = end - start;

                let routine = context.get_current_routine_ref();

                if cursor + length_in_byte > routine.range_end {
                    ExecuteResult::Failure
                } else {
                    let mut all_byte_matched = true;

                    for (idx, actual_byte) in bytes.iter().enumerate() {
                        if actual_byte != &context.bytes[idx + cursor] {
                            all_byte_matched = false;
                            break;
                        }
                    }

                    if all_byte_matched {
                        ExecuteResult::Success(length_in_byte, 0)
                    } else {
                        ExecuteResult::Failure
                    }
                }
            }
            Transition::LineBoundaryAssertion(transition) => {
                let bytes = context.bytes;
                let success = if transition.is_end {
                    is_end(bytes, cursor)
                } else {
                    is_first_char(cursor)
                };

                if success {
                    ExecuteResult::Success(0, 0)
                } else {
                    ExecuteResult::Failure
                }
            }
            Transition::WordBoundaryAssertion(transition) => {
                let bytes = context.bytes;
                let success = if transition.is_negative {
                    !is_word_bound(bytes, cursor)
                } else {
                    is_word_bound(bytes, cursor)
                };

                if success {
                    ExecuteResult::Success(0, 0)
                } else {
                    ExecuteResult::Failure
                }
            }
            Transition::CaptureStart(transition) => {
                context.set_capture_slot(
                    transition.capture_group_index,
                    Some(MatchRange::new(cursor, cursor)),
                );
                ExecuteResult::Success(0, 0)
            }
            Transition::CaptureEnd(transition) => {
                let index = transition.capture_group_index;
                let start = context.matched_slots[index]
                    .as_ref()
                    .map_or(cursor, |range| range.start);
                context.set_capture_slot(index, Some(MatchRange::new(start, cursor)));
                ExecuteResult::Success(0, 0)
            }
            Transition::CounterReset(transition) => {
                let counter_index = transition.counter_index;
                context.set_counter_slot(counter_index, 0);
                context.set_iteration_start_slot(counter_index, cursor);
                ExecuteResult::Success(0, 0)
            }
            Transition::CounterIncrement(transition) => {
                let counter_index = transition.counter_index;
                let new_count = context.counter_slots[counter_index] + 1;
                context.set_counter_slot(counter_index, new_count);
                ExecuteResult::Success(0, new_count)
            }
            Transition::RepetitionForward(transition) => {
                let can_forward = match transition.repetition_type {
                    RepetitionType::Repeat(m) => repetition_count == m,
                    RepetitionType::RepeatFrom(from) => repetition_count >= from,
                    RepetitionType::RepeatRange(from, to) => {
                        repetition_count >= from && repetition_count <= to
                    }
                };
                if can_forward {
                    ExecuteResult::Success(0, repetition_count)
                } else {
                    ExecuteResult::Failure
                }
            }
            Transition::RepetitionBack(transition) => {
                let (can_backward, min) = match transition.repetition_type {
                    RepetitionType::Repeat(n) => (repetition_count < n, n),
                    RepetitionType::RepeatFrom(from) => (true, from),
                    RepetitionType::RepeatRange(from, to) => (repetition_count < to, from),
                };

                // Stop empty iterations once the minimum is met to prevent an infinite loop.
                let counter_index = transition.counter_index;
                let is_empty_iteration = context.iteration_start_slots[counter_index] == cursor;

                if can_backward && !(is_empty_iteration && repetition_count >= min) {
                    context.set_iteration_start_slot(counter_index, cursor);
                    ExecuteResult::Success(0, repetition_count)
                } else {
                    ExecuteResult::Failure
                }
            }
            Transition::LookAheadAssertion(transition) => {
                let route_index = transition.route_index;
                let undo_log_length = context.undo_log.len();
                let routine_result =
                    start_routine(context, map, route_index, cursor, context.bytes.len());

                let result = routine_result ^ transition.is_negative;
                if result {
                    // The assertion leaves the parent routine's cursor unchanged.
                    const NO_FORWARD: usize = 0;
                    ExecuteResult::Success(NO_FORWARD, 0)
                } else {
                    context.rollback(undo_log_length);
                    ExecuteResult::Failure
                }
            }
            Transition::LookBehindAssertion(transition) => {
                let route_index = transition.route_index;
                let undo_log_length = context.undo_log.len();
                let routine_result = if let Ok(start) = get_position_by_chars_backward(
                    context.bytes,
                    cursor,
                    transition.match_length_in_char,
                ) {
                    // Start the subroutine at the fixed-length lookbehind's first character.
                    start_routine(context, map, route_index, start, context.bytes.len())
                } else {
                    false
                };

                let result = routine_result ^ transition.is_negative;
                if result {
                    // The assertion leaves the parent routine's cursor unchanged.
                    const NO_FORWARD: usize = 0;
                    ExecuteResult::Success(NO_FORWARD, 0)
                } else {
                    context.rollback(undo_log_length);
                    ExecuteResult::Failure
                }
            }
        }
    }
}

// Returns `Err` if moving backward would pass the start of the input.
#[inline]
fn get_position_by_chars_backward(
    bytes: &[u8],
    mut current_position: usize,
    backward_chars: usize,
) -> Result<usize, ()> {
    for _ in 0..backward_chars {
        if current_position == 0 {
            return Err(());
        }

        let (_, char_length_in_byte) = previous_codepoint(bytes, current_position);
        current_position -= char_length_in_byte;
    }

    Ok(current_position)
}

#[inline]
fn get_char(bytes: &[u8], position: usize) -> (u32, usize) {
    next_codepoint(bytes, position)
}

#[inline]
fn is_first_char(position: usize) -> bool {
    position == 0
}

#[inline]
fn is_end(bytes: &[u8], position: usize) -> bool {
    let total_byte_length = bytes.len();
    position >= total_byte_length
}

#[inline]
fn is_word_bound(bytes: &[u8], position: usize) -> bool {
    if bytes.is_empty() {
        false
    } else if position == 0 {
        let (current_char, _) = get_char(bytes, position);
        is_word_char(current_char)
    } else if position >= bytes.len() {
        let (previous_char, _) = previous_codepoint(bytes, position);
        is_word_char(previous_char)
    } else {
        let (current_char, _) = get_char(bytes, position);
        let (previous_char, _) = previous_codepoint(bytes, position);

        if is_word_char(current_char) {
            !is_word_char(previous_char)
        } else {
            is_word_char(previous_char)
        }
    }
}

#[inline]
fn is_word_char(c: u32) -> bool {
    (c >= 'a' as u32 && c <= 'z' as u32)
        || (c >= 'A' as u32 && c <= 'Z' as u32)
        || (c >= '0' as u32 && c <= '9' as u32)
        || (c == '_' as u32)
}

/// Represents the result of executing a transition.
pub enum ExecuteResult {
    Success(
        usize, // Number of bytes to move forward
        usize, // The latest repetition count
    ),
    Failure, // Indicates that the transition failed
}
