// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

pub const ROUND_QUEUE_LENGTH: usize = 8;

/// Extends `std::iter::Peekable` with fixed-capacity lookahead.
pub struct PeekableIter<'a, T>
where
    T: PartialEq,
{
    upstream: &'a mut dyn Iterator<Item = T>,
    buffer: RoundQueue<T>,
}

impl<'a, T> PeekableIter<'a, T>
where
    T: PartialEq,
{
    /// Buffers up to `ROUND_QUEUE_LENGTH` items from the upstream iterator.
    pub fn new(upstream: &'a mut dyn Iterator<Item = T>) -> Self {
        let mut buffer = RoundQueue::new();

        // Pre-fill the buffer with the first `ROUND_QUEUE_LENGTH` elements from the upstream iterator.
        for _ in 0..ROUND_QUEUE_LENGTH {
            let value = upstream.next();
            buffer.enqueue(value);
        }

        Self { upstream, buffer }
    }

    /// Looks ahead by `offset` items; panics if `offset >= ROUND_QUEUE_LENGTH`.
    pub fn peek(&self, offset: usize) -> Option<&T> {
        assert!(offset < ROUND_QUEUE_LENGTH);
        self.buffer.peek(offset)
    }
}

impl<T> Iterator for PeekableIter<'_, T>
where
    T: PartialEq,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // Refill only after removing the oldest buffered item.
        let value = self.buffer.dequeue();

        let next_value = self.upstream.next();
        self.buffer.enqueue(next_value);

        value
    }
}

/// Fixed-size circular buffer for `PeekableIter` lookahead.
struct RoundQueue<T>
where
    T: PartialEq,
{
    position_read: usize,
    position_write: usize,
    data: [Option<T>; ROUND_QUEUE_LENGTH],
}

impl<T> RoundQueue<T>
where
    T: PartialEq,
{
    /// Creates a new RoundQueue.
    pub fn new() -> Self {
        Self {
            position_read: 0,
            position_write: 0,
            data: [const { None }; ROUND_QUEUE_LENGTH],
        }
    }

    /// Overwrites the next slot, wrapping at the buffer's end.
    pub fn enqueue(&mut self, value: Option<T>) {
        self.data[self.position_write] = value;

        self.position_write += 1;
        if self.position_write == ROUND_QUEUE_LENGTH {
            self.position_write = 0;
        }
    }

    /// Takes the next slot, wrapping at the buffer's end.
    pub fn dequeue(&mut self) -> Option<T> {
        let value = self.data[self.position_read].take();

        self.position_read += 1;
        if self.position_read == ROUND_QUEUE_LENGTH {
            self.position_read = 0;
        }

        value
    }

    /// Reads relative to the next slot; panics if `offset >= ROUND_QUEUE_LENGTH`.
    pub fn peek(&self, offset: usize) -> Option<&T> {
        assert!(offset < ROUND_QUEUE_LENGTH);

        let mut position = self.position_read + offset;
        if position >= ROUND_QUEUE_LENGTH {
            position -= ROUND_QUEUE_LENGTH;
        }

        self.data[position].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use crate::peekable_iter::PeekableIter;

    #[test]
    fn test_peekable_iter() {
        let s = "0123";
        let mut chars = s.chars();
        let mut iter = PeekableIter::new(&mut chars);

        // Initial state: buffer contains '0', '1', '2'
        assert_eq!(Some(&'0'), iter.peek(0));
        assert_eq!(Some(&'1'), iter.peek(1));
        assert_eq!(Some(&'2'), iter.peek(2));

        // Consume '0'
        assert_eq!(Some('0'), iter.next());
        assert_eq!(Some(&'1'), iter.peek(0));
        assert_eq!(Some(&'2'), iter.peek(1));
        assert_eq!(Some(&'3'), iter.peek(2));

        // Consume '1'
        assert_eq!(Some('1'), iter.next());
        assert_eq!(Some(&'2'), iter.peek(0));
        assert_eq!(Some(&'3'), iter.peek(1));
        assert_eq!(None, iter.peek(2));

        // Consume '2'
        assert_eq!(Some('2'), iter.next());
        assert_eq!(Some(&'3'), iter.peek(0));
        assert_eq!(None, iter.peek(1));
        assert_eq!(None, iter.peek(2));

        // Consume '3'
        assert_eq!(Some('3'), iter.next());
        assert_eq!(None, iter.peek(0));
        assert_eq!(None, iter.peek(1));
        assert_eq!(None, iter.peek(2));

        // Iterator is now empty
        assert_eq!(None, iter.next());
        assert_eq!(None, iter.peek(0));
        assert_eq!(None, iter.peek(1));
        assert_eq!(None, iter.peek(2));
    }

    #[test]
    fn test_nested_peekable_iter() {
        let s = "0123";
        let mut chars = s.chars();
        let mut iter1 = PeekableIter::new(&mut chars);
        let mut iter2 = PeekableIter::new(&mut iter1);

        // Initial state: buffer contains '0', '1', '2'
        assert_eq!(Some(&'0'), iter2.peek(0));
        assert_eq!(Some(&'1'), iter2.peek(1));
        assert_eq!(Some(&'2'), iter2.peek(2));

        // Consume '0'
        assert_eq!(Some('0'), iter2.next());
        assert_eq!(Some(&'1'), iter2.peek(0));

        // Consume '1'
        assert_eq!(Some('1'), iter2.next());
        assert_eq!(Some(&'2'), iter2.peek(0));

        // Consume '2'
        assert_eq!(Some('2'), iter2.next());
        assert_eq!(Some(&'3'), iter2.peek(0));

        // Consume '3'
        assert_eq!(Some('3'), iter2.next());
        assert_eq!(None, iter2.peek(0));

        // Iterator is now empty
        assert_eq!(None, iter2.next());
        assert_eq!(None, iter2.peek(0));
    }
}
