// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

/// Returns `(codepoint, byte_length)`; callers must provide a valid UTF-8 character offset within `data`.
#[inline]
pub fn next_codepoint(data: &[u8], position: usize) -> (u32, usize) {
    // UTF-8 byte layouts (bits shown as groups; high-order bits on the left):
    //
    // 1 byte:  0_bbb_aaaa
    // 2 bytes: 110_ccc_bb, 10_bb_aaaa
    // 3 bytes: 1110_dddd, 10_cccc_bb, 10_bb_aaaa
    // 4 bytes: 11110_f_ee, 10_ee_dddd, 10_cccc_bb, 10_bb_aaaa
    //
    // Payload bits are combined after removing each byte's UTF-8 prefix.
    // Reference: https://en.wikipedia.org/wiki/UTF-8

    let mut code: u32 = 0;

    let first_byte = data[position];
    let byte_length = match first_byte.leading_ones() {
        0 => {
            // 0_bbb_aaaa
            code |= first_byte as u32;
            1
        }
        2 => {
            // 110_ccc_bb, 10_bb_aaaa
            code |= ((first_byte & 0b1_1111) as u32) << 6;
            code |= (data[position + 1] & 0b11_1111) as u32;
            2
        }
        3 => {
            // 1110_dddd, 10_cccc_bb, 10_bb_aaaa
            code |= ((first_byte & 0b1111) as u32) << 12;
            code |= ((data[position + 1] & 0b11_1111) as u32) << 6;
            code |= (data[position + 2] & 0b11_1111) as u32;
            3
        }
        4 => {
            // 11110_f_ee, 10_ee_dddd, 10_cccc_bb, 10_bb_aaaa
            code |= ((first_byte & 0b111) as u32) << 18;
            code |= ((data[position + 1] & 0b11_1111) as u32) << 12;
            code |= ((data[position + 2] & 0b11_1111) as u32) << 6;
            code |= (data[position + 3] & 0b11_1111) as u32;
            4
        }
        _ => {
            // Invalid leading bytes are returned unchanged; this is not UTF-8 validation.
            code = first_byte as u32;
            1
        }
    };

    (code, byte_length)
}

#[allow(dead_code)]
pub fn previous_codepoint(data: &[u8], mut position: usize) -> (u32, usize) {
    position -= 1;
    while data[position].leading_ones() == 1 {
        position -= 1;
    }
    next_codepoint(data, position)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::utf8_codepoint_reader::{next_codepoint, previous_codepoint};

    #[test]
    fn test_next_codepoint() {
        let data = "a文b😋c".bytes().collect::<Vec<u8>>();
        let data_ref = &data[..];

        assert_eq!(next_codepoint(data_ref, 0), ('a' as u32, 1));
        assert_eq!(next_codepoint(data_ref, 1), ('文' as u32, 3));
        assert_eq!(next_codepoint(data_ref, 4), ('b' as u32, 1));
        assert_eq!(next_codepoint(data_ref, 5), ('😋' as u32, 4));
        assert_eq!(next_codepoint(data_ref, 9), ('c' as u32, 1));
    }

    #[test]
    fn test_previous_codepoint() {
        let data = "a文b😋c".bytes().collect::<Vec<u8>>();
        let data_ref = &data[..];

        assert_eq!(previous_codepoint(data_ref, 1), ('a' as u32, 1));
        assert_eq!(previous_codepoint(data_ref, 4), ('文' as u32, 3));
        assert_eq!(previous_codepoint(data_ref, 5), ('b' as u32, 1));
        assert_eq!(previous_codepoint(data_ref, 9), ('😋' as u32, 4));
        assert_eq!(previous_codepoint(data_ref, 10), ('c' as u32, 1));
    }
}
