/* Copyright 2024-2026 Joachim Metz <joachim.metz@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License. You may
 * obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations
 * under the License.
 */

use std::fmt;

use keramics_types::{
    bytes_to_i16_be, bytes_to_i16_le, bytes_to_i32_be, bytes_to_i32_le, bytes_to_i64_be,
    bytes_to_i64_le, bytes_to_u16_be, bytes_to_u16_le, bytes_to_u32_be, bytes_to_u32_le,
    bytes_to_u64_be, bytes_to_u64_le,
};

/// Fast File System (FFS) byte order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FfsByteOrder {
    /// Big-endian byte order.
    BigEndian,

    /// Little-endian byte order.
    LittleEndian,
}

impl FfsByteOrder {
    /// Reads a signed 16-bit integer at the given offset.
    pub fn read_i16(&self, data: &[u8], offset: usize) -> i16 {
        match self {
            FfsByteOrder::BigEndian => bytes_to_i16_be!(data, offset),
            FfsByteOrder::LittleEndian => bytes_to_i16_le!(data, offset),
        }
    }

    /// Reads a signed 32-bit integer at the given offset.
    pub fn read_i32(&self, data: &[u8], offset: usize) -> i32 {
        match self {
            FfsByteOrder::BigEndian => bytes_to_i32_be!(data, offset),
            FfsByteOrder::LittleEndian => bytes_to_i32_le!(data, offset),
        }
    }

    /// Reads a signed 64-bit integer at the given offset.
    pub fn read_i64(&self, data: &[u8], offset: usize) -> i64 {
        match self {
            FfsByteOrder::BigEndian => bytes_to_i64_be!(data, offset),
            FfsByteOrder::LittleEndian => bytes_to_i64_le!(data, offset),
        }
    }

    /// Reads an unsigned 16-bit integer at the given offset.
    pub fn read_u16(&self, data: &[u8], offset: usize) -> u16 {
        match self {
            FfsByteOrder::BigEndian => bytes_to_u16_be!(data, offset),
            FfsByteOrder::LittleEndian => bytes_to_u16_le!(data, offset),
        }
    }

    /// Reads an unsigned 32-bit integer at the given offset.
    pub fn read_u32(&self, data: &[u8], offset: usize) -> u32 {
        match self {
            FfsByteOrder::BigEndian => bytes_to_u32_be!(data, offset),
            FfsByteOrder::LittleEndian => bytes_to_u32_le!(data, offset),
        }
    }

    /// Reads an unsigned 64-bit integer at the given offset.
    pub fn read_u64(&self, data: &[u8], offset: usize) -> u64 {
        match self {
            FfsByteOrder::BigEndian => bytes_to_u64_be!(data, offset),
            FfsByteOrder::LittleEndian => bytes_to_u64_le!(data, offset),
        }
    }
}

impl fmt::Display for FfsByteOrder {
    /// Formats the byte order for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FfsByteOrder::BigEndian => write!(formatter, "big-endian"),
            FfsByteOrder::LittleEndian => write!(formatter, "little-endian"),
        }
    }
}

/// Fast File System (FFS) format version.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FfsFormatVersion {
    /// FFS1 (UFS1).
    Ffs1,

    /// FFS2 (UFS2).
    Ffs2,

    /// NetBSD FFS2EA (UFS2 with extended attributes).
    Ffs2Ea,
}

impl FfsFormatVersion {
    /// Retrieves the numeric format version (1 for FFS1, 2 for FFS2/FFS2EA).
    pub fn as_u8(&self) -> u8 {
        match self {
            FfsFormatVersion::Ffs1 => 1,
            FfsFormatVersion::Ffs2 | FfsFormatVersion::Ffs2Ea => 2,
        }
    }

    /// Determines if the format version is FFS2 or FFS2EA.
    pub fn is_ffs2(&self) -> bool {
        matches!(self, FfsFormatVersion::Ffs2 | FfsFormatVersion::Ffs2Ea)
    }
}

impl fmt::Display for FfsFormatVersion {
    /// Formats the format version for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FfsFormatVersion::Ffs1 => write!(formatter, "FFS1 (UFS1)"),
            FfsFormatVersion::Ffs2 => write!(formatter, "FFS2 (UFS2)"),
            FfsFormatVersion::Ffs2Ea => write!(formatter, "FFS2EA (UFS2EA)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_order_reads() {
        let data: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

        let big_endian: FfsByteOrder = FfsByteOrder::BigEndian;
        let little_endian: FfsByteOrder = FfsByteOrder::LittleEndian;

        let value_u16_be: u16 = big_endian.read_u16(&data, 0);
        assert_eq!(value_u16_be, 0x0102);
        let value_u16_le: u16 = little_endian.read_u16(&data, 0);
        assert_eq!(value_u16_le, 0x0201);

        let value_i16_be: i16 = big_endian.read_i16(&data, 0);
        assert_eq!(value_i16_be, 0x0102);
        let value_i16_le: i16 = little_endian.read_i16(&data, 0);
        assert_eq!(value_i16_le, 0x0201);

        let value_u32_be: u32 = big_endian.read_u32(&data, 0);
        assert_eq!(value_u32_be, 0x01020304);
        let value_u32_le: u32 = little_endian.read_u32(&data, 0);
        assert_eq!(value_u32_le, 0x04030201);

        let value_i32_be: i32 = big_endian.read_i32(&data, 0);
        assert_eq!(value_i32_be, 0x01020304);
        let value_i32_le: i32 = little_endian.read_i32(&data, 0);
        assert_eq!(value_i32_le, 0x04030201);

        let value_u64_be: u64 = big_endian.read_u64(&data, 0);
        assert_eq!(value_u64_be, 0x0102030405060708);
        let value_u64_le: u64 = little_endian.read_u64(&data, 0);
        assert_eq!(value_u64_le, 0x0807060504030201);

        let value_i64_be: i64 = big_endian.read_i64(&data, 0);
        assert_eq!(value_i64_be, 0x0102030405060708);
        let value_i64_le: i64 = little_endian.read_i64(&data, 0);
        assert_eq!(value_i64_le, 0x0807060504030201);

        let be_string: String = big_endian.to_string();
        assert_eq!(be_string, "big-endian");
        let le_string: String = little_endian.to_string();
        assert_eq!(le_string, "little-endian");
    }

    #[test]
    fn test_format_version() {
        let ffs1: FfsFormatVersion = FfsFormatVersion::Ffs1;
        let ffs2: FfsFormatVersion = FfsFormatVersion::Ffs2;
        let ffs2ea: FfsFormatVersion = FfsFormatVersion::Ffs2Ea;

        let version1: u8 = ffs1.as_u8();
        assert_eq!(version1, 1);
        let is_ffs2_1: bool = ffs1.is_ffs2();
        assert!(!is_ffs2_1);
        let string1: String = ffs1.to_string();
        assert_eq!(string1, "FFS1 (UFS1)");

        let version2: u8 = ffs2.as_u8();
        assert_eq!(version2, 2);
        let is_ffs2_2: bool = ffs2.is_ffs2();
        assert!(is_ffs2_2);
        let string2: String = ffs2.to_string();
        assert_eq!(string2, "FFS2 (UFS2)");

        let version2ea: u8 = ffs2ea.as_u8();
        assert_eq!(version2ea, 2);
        let is_ffs2_ea: bool = ffs2ea.is_ffs2();
        assert!(is_ffs2_ea);
        let string2ea: String = ffs2ea.to_string();
        assert_eq!(string2ea, "FFS2EA (UFS2EA)");
    }
}
