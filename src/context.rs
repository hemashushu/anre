// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::object::Map;

/// Shared state for one search, including nested lookaround evaluations.
pub struct Context<'a> {
    pub bytes: &'a [u8],
    // Lookaround evaluations suspend their parent routine on this stack.
    pub routines: Vec<Routine>,
    // `None` means the group did not participate in the match.
    pub matched_slots: Vec<Option<MatchRange>>,
    pub counter_slots: Vec<usize>,
    // Byte offsets used to detect repetitions that make no progress.
    pub iteration_start_slots: Vec<usize>,
    // Previous slot values allow backtracking to restore earlier states.
    pub undo_log: Vec<UndoEntry>,
}

/// A previous slot value recorded for backtracking.
pub enum UndoEntry {
    Capture(usize, Option<MatchRange>),
    Counter(usize, usize),
    IterationStart(usize, usize),
}

/// Evaluation state for a route within a search range.
pub struct Routine {
    // Inclusive byte offset where this routine starts searching.
    pub range_start: usize,
    // Exclusive byte offset of the search range.
    pub range_end: usize,
    pub route_index: usize,
    // Pending alternatives are saved here until they succeed or are exhausted.
    pub transition_stack: Vec<TransitionStackItem>,
}

impl Routine {
    pub fn new(range_start: usize, range_end: usize, route_index: usize) -> Routine {
        Routine {
            range_start,
            range_end,
            route_index,
            transition_stack: vec![],
        }
    }
}

/// A pending transition with the state needed to resume it during backtracking.
pub struct TransitionStackItem {
    // Byte offset in the source text.
    pub cursor: usize,
    pub repetition_count: usize,
    pub current_node_index: usize,
    pub transition_index: usize,
    // Length of the undo log when this item was pushed.
    pub undo_log_length: usize,
}

impl TransitionStackItem {
    pub fn new(
        cursor: usize,
        repetition_count: usize,
        current_node_index: usize,
        transition_index: usize,
        undo_log_length: usize,
    ) -> Self {
        TransitionStackItem {
            cursor,
            repetition_count,
            current_node_index,
            transition_index,
            undo_log_length,
        }
    }
}

/// Half-open byte range captured by a group.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct MatchRange {
    pub start: usize,
    pub end: usize,
}

impl MatchRange {
    pub fn new(start: usize, end: usize) -> Self {
        MatchRange { start, end }
    }
}

impl<'a> Context<'a> {
    pub fn new(
        s: &'a str,
        number_of_capture_groups: usize, // Number of pre-allocated match slots.
        number_of_counters: usize,       // Number of pre-allocated counter slots.
    ) -> Self {
        let bytes = s.as_bytes();
        Self::from_bytes(bytes, number_of_capture_groups, number_of_counters)
    }

    pub fn from_bytes(
        bytes: &'a [u8],
        number_of_capture_groups: usize, // Number of pre-allocated match slots.
        number_of_counters: usize,       // Number of pre-allocated counter slots.
    ) -> Self {
        Context {
            bytes,
            routines: vec![],

            counter_slots: vec![0; number_of_counters],

            iteration_start_slots: vec![0; number_of_counters],

            matched_slots: vec![None; number_of_capture_groups],

            undo_log: vec![],
        }
    }

    /// Clears the state left by a previous process.
    pub fn reset(&mut self) {
        self.matched_slots.fill(None);
        self.counter_slots.fill(0);
        self.iteration_start_slots.fill(0);
        self.undo_log.clear();
    }

    pub fn set_capture_slot(&mut self, index: usize, value: Option<MatchRange>) {
        let old = std::mem::replace(&mut self.matched_slots[index], value);
        self.undo_log.push(UndoEntry::Capture(index, old));
    }

    pub fn set_counter_slot(&mut self, index: usize, value: usize) {
        let old = std::mem::replace(&mut self.counter_slots[index], value);
        self.undo_log.push(UndoEntry::Counter(index, old));
    }

    pub fn set_iteration_start_slot(&mut self, index: usize, value: usize) {
        let old = std::mem::replace(&mut self.iteration_start_slots[index], value);
        self.undo_log.push(UndoEntry::IterationStart(index, old));
    }

    /// Restores slots to the state when the undo log had `length` entries.
    pub fn rollback(&mut self, length: usize) {
        while self.undo_log.len() > length {
            match self.undo_log.pop().unwrap() {
                UndoEntry::Capture(index, value) => self.matched_slots[index] = value,
                UndoEntry::Counter(index, value) => self.counter_slots[index] = value,
                UndoEntry::IterationStart(index, value) => {
                    self.iteration_start_slots[index] = value
                }
            }
        }
    }

    pub fn push_transitions_of_node(
        &mut self,
        map: &Map,
        node_index: usize,
        cursor: usize,
        repetition_count: usize,
    ) {
        let undo_log_length = self.undo_log.len();
        let routine = self.get_current_routine_ref_mut();
        let route_index = routine.route_index;
        let node = &map.routes[route_index].nodes[node_index];
        let transition_count = node.path.len();

        for transition_index in (0..transition_count).rev() {
            routine.transition_stack.push(TransitionStackItem::new(
                cursor,
                repetition_count,
                node_index,
                transition_index,
                undo_log_length,
            ));
        }
    }

    pub fn pop_transition_stack_item(&mut self) -> Option<TransitionStackItem> {
        self.get_current_routine_ref_mut().transition_stack.pop()
    }

    #[inline]
    pub fn get_current_routine_ref(&self) -> &Routine {
        self.routines.last().unwrap()
    }

    #[inline]
    pub fn get_current_routine_ref_mut(&mut self) -> &mut Routine {
        let count = self.routines.len();
        &mut self.routines[count - 1]
    }
}
