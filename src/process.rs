// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{
    context::{Context, MatchRange, Routine, TransitionStackItem},
    object::{MAIN_ROUTE_INDEX, Map},
    transition::{CharRunTransition, RepetitionType, Transition},
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
    let transition_stack = context.spare_stacks.pop().unwrap_or_default();
    let routine = Routine::new(start_position, end_position, route_index, transition_stack);
    context.routines.push(routine);

    let is_fixed = map.routes[route_index].is_fixed_cursor_begin_position;
    let is_main_search = route_index == MAIN_ROUTE_INDEX && !is_fixed;
    let leading_run = map.leading_run.as_ref().filter(|_| is_main_search);

    // Include the end position because zero-width patterns may match there.
    loop {
        // Skip positions where no match can begin.
        if is_main_search {
            match next_candidate(context.bytes, map, cursor, end_position) {
                Some(candidate) => cursor = candidate,
                None => break,
            }
        }

        if execute_transitions(context, map, cursor) {
            result = true;
            break;
        }

        // Anchored routes and lookaround subroutes must not search later positions.
        if is_fixed || cursor >= end_position {
            break;
        }

        // Starts inside the leading run would only retry continuations that already failed.
        if let Some(run) = leading_run {
            let run_start = cursor;
            while let Some(length) = run.match_char(context.bytes, cursor, end_position) {
                cursor += length;
            }
            if cursor > run_start {
                continue;
            }
        }

        // Move forward by one character and try again.
        let (_, byte_length) = next_codepoint(context.bytes, cursor);
        cursor += byte_length;
    }

    // Remove the routine for the route from the context.
    let mut transition_stack = context.routines.pop().unwrap().transition_stack;
    transition_stack.clear();
    context.spare_stacks.push(transition_stack);
    result
}

/// Returns the first position at or after `cursor` where a match may begin,
/// or `None` if no match is possible in the rest of the text.
fn next_candidate(bytes: &[u8], map: &Map, mut cursor: usize, end: usize) -> Option<usize> {
    if let Some(prefix) = &map.literal_prefix {
        // Find the prefix's first byte quickly, then compare the whole prefix.
        loop {
            cursor += find_byte(&bytes[cursor..end], prefix[0])?;
            if bytes[cursor..end].starts_with(prefix) {
                return Some(cursor);
            }
            // `prefix[0]` never equals a UTF-8 continuation byte, so the next hit is a character boundary.
            cursor += 1;
        }
    }

    match &map.first_bytes {
        Some(table) => bytes[cursor..end]
            .iter()
            .position(|byte| table[*byte as usize])
            .map(|offset| cursor + offset),
        None => Some(cursor),
    }
}

/// Finds `target` by checking eight bytes at a time.
fn find_byte(haystack: &[u8], target: u8) -> Option<usize> {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;
    let pattern = ONES * target as u64;

    let (chunks, remainder) = haystack.as_chunks::<8>();
    for (index, chunk) in chunks.iter().enumerate() {
        // Bytes equal to `target` become zero; the classic zero-byte test then sets their high bit.
        let word = u64::from_le_bytes(*chunk) ^ pattern;
        let zero_bytes = word.wrapping_sub(ONES) & !word & HIGHS;
        if zero_bytes != 0 {
            return Some(index * 8 + zero_bytes.trailing_zeros() as usize / 8);
        }
    }

    let offset = chunks.len() * 8;
    remainder
        .iter()
        .position(|byte| *byte == target)
        .map(|position| offset + position)
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
    let mut next_frame = context.pop_transition_stack_item();
    while let Some(frame) = next_frame {
        // Discard slot changes made after this frame was pushed.
        context.rollback(frame.undo_log_length);

        let route = &map.routes[route_index];
        let node = &route.nodes[frame.current_node_index];
        let transition_item = &node.path[frame.transition_index];

        let last_cursor = frame.cursor;
        let last_repetition_count = frame.repetition_count;
        let transition = &transition_item.transition;
        let target_node_index = transition_item.target_node_index;

        let execute_result = match transition {
            Transition::CharRun(run) => {
                execute_char_run(context, map, run, last_cursor, target_node_index)
            }
            _ => transition.execute(context, map, last_cursor, last_repetition_count),
        };

        next_frame = match execute_result {
            ExecuteResult::Success(move_forward_in_bytes, current_repetition_count) => {
                if target_node_index == exit_node_index {
                    // Reached the exit node of the route, meaning the route matching succeeded.
                    return true;
                }

                let next_cursor = last_cursor + move_forward_in_bytes;
                if route.nodes[target_node_index].path.len() == 1 {
                    // A node with a single path is not a backtracking point,
                    // so its transition runs directly instead of going through the stack.
                    Some(TransitionStackItem::new(
                        next_cursor,
                        current_repetition_count,
                        target_node_index,
                        0,
                        context.undo_log.len(),
                    ))
                } else {
                    context.push_transitions_of_node(
                        map,
                        target_node_index,
                        next_cursor,
                        current_repetition_count,
                    );
                    context.pop_transition_stack_item()
                }
            }
            ExecuteResult::Failure => context.pop_transition_stack_item(),
        };
    }

    // All transitions failed, meaning the route matching failed.
    context.rollback(undo_log_length);
    false
}

/// Consumes as many characters as allowed and continues with the longest run.
/// Each shorter valid length is pushed as an alternative for the target node,
/// so backtracking gives back one character at a time, like the general repetition.
fn execute_char_run(
    context: &mut Context,
    map: &Map,
    run: &CharRunTransition,
    cursor: usize,
    target_node_index: usize,
) -> ExecuteResult {
    let range_end = context.get_current_routine_ref().range_end;
    let min = run.repetition_type.min();
    let max = run.repetition_type.max();

    let mut count = 0;
    let mut position = cursor;
    while max.is_none_or(|max| count < max) {
        let Some(length) = run
            .char_transition
            .match_char(context.bytes, position, range_end)
        else {
            break;
        };
        if count >= min {
            context.push_transitions_of_node(map, target_node_index, position, 0);
        }
        position += length;
        count += 1;
    }

    if count >= min {
        ExecuteResult::Success(position - cursor, 0)
    } else {
        ExecuteResult::Failure
    }
}

impl Transition {
    /// Matches one character for `Char`, `AnyChar`, and `CharSet` transitions,
    /// returning its byte length.
    pub fn match_char(&self, bytes: &[u8], cursor: usize, range_end: usize) -> Option<usize> {
        if cursor >= range_end {
            return None;
        }

        let (current_char, byte_length) = get_char(bytes, cursor);
        let is_matched = match self {
            Transition::Char(transition) => current_char == transition.codepoint,
            // `char_any` excludes LF and CR, but not other Unicode line separators.
            Transition::AnyChar(_) => current_char != '\n' as u32 && current_char != '\r' as u32,
            Transition::CharSet(transition) => {
                transition.contains(current_char) ^ transition.negative
            }
            _ => unreachable!(),
        };

        is_matched.then_some(byte_length)
    }

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
            Transition::CharRun(_) => {
                unreachable!("`execute_transitions` runs char runs because they need the target node")
            }
            Transition::Char(_) | Transition::AnyChar(_) | Transition::CharSet(_) => {
                let range_end = context.get_current_routine_ref().range_end;
                match self.match_char(context.bytes, cursor, range_end) {
                    Some(byte_length) => ExecuteResult::Success(byte_length, 0),
                    None => ExecuteResult::Failure,
                }
            }
            Transition::String(transition) => {
                let routine = context.get_current_routine_ref();

                if cursor + transition.byte_length > routine.range_end {
                    ExecuteResult::Failure
                } else if context.bytes[cursor..cursor + transition.byte_length]
                    == *transition.text.as_bytes()
                {
                    ExecuteResult::Success(transition.byte_length, 0)
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

#[cfg(test)]
mod tests {
    use super::find_byte;

    #[test]
    fn test_find_byte() {
        let text: Vec<u8> = (0..40).collect();
        for target in 0..40 {
            assert_eq!(find_byte(&text, target), Some(target as usize));
            assert_eq!(find_byte(&text[3..], target), (target as usize).checked_sub(3));
        }
        assert_eq!(find_byte(&text, 200), None);
        assert_eq!(find_byte(b"", b'a'), None);

        // Only the first occurrence counts, even when several share a chunk.
        assert_eq!(find_byte(b"xxaxaxxxaa", b'a'), Some(2));
        assert_eq!(find_byte(&[0x80, 0x01, 0x81, 0x01], 0x01), Some(1));
    }
}
