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

use keramics_core::ErrorTrace;
use keramics_layout_map::LayoutMap;
use keramics_types::bytes_to_u32_be;

use super::constants::*;

#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "big",
        field(name = "signature", data_type = "[u8; 4]", format = "hex"),
        field(name = "name_length", data_type = "u32"),
        field(name = "data_length", data_type = "u32"),
        field(name = "flag", data_type = "u32", format = "hex"),
    ),
    methods("debug_read_data", "read_at_position")
)]
/// Advanced Forensic Format (AFF) segment header.
pub(super) struct AffSegmentHeader {
    /// Segment name length in bytes.
    pub name_length: u32,

    /// Segment data length in bytes.
    pub data_length: u32,

    /// Segment flag.
    pub flag: u32,
}

impl AffSegmentHeader {
    /// Creates a new segment header.
    pub fn new() -> Self {
        Self {
            name_length: 0,
            data_length: 0,
            flag: 0,
        }
    }

    /// Reads the segment header from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 16 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        if &data[0..4] != AFF_SEGMENT_SIGNATURE {
            return Err(keramics_core::error_trace_new!("Unsupported signature"));
        }
        self.name_length = bytes_to_u32_be!(data, 4);
        self.data_length = bytes_to_u32_be!(data, 8);
        self.flag = bytes_to_u32_be!(data, 12);

        Ok(())
    }
}

#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "big",
        field(name = "signature", data_type = "[u8; 4]", format = "hex"),
        field(name = "segment_size", data_type = "u32"),
    ),
    methods("debug_read_data", "read_at_position")
)]
/// Advanced Forensic Format (AFF) segment tail.
pub(super) struct AffSegmentTail {
    /// Total segment size in bytes.
    pub segment_size: u32,
}

impl AffSegmentTail {
    /// Creates a new segment tail.
    pub fn new() -> Self {
        Self { segment_size: 0 }
    }

    /// Reads the segment tail from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 8 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        if &data[0..4] != AFF_SEGMENT_TAIL_SIGNATURE {
            return Err(keramics_core::error_trace_new!("Unsupported signature"));
        }
        self.segment_size = bytes_to_u32_be!(data, 4);

        Ok(())
    }
}

/// AFF segment.
#[derive(Clone, Debug)]
pub(super) struct AffSegment {
    /// Segment name.
    name: String,

    /// Segment flag.
    flag: u32,

    /// Offset in bytes of the segment data in the data stream.
    data_offset: u64,

    /// Size in bytes of the segment data.
    data_length: u32,
}

impl AffSegment {
    /// Creates a new AFF segment.
    pub fn new(name: String, flag: u32, data_offset: u64, data_length: u32) -> Self {
        Self {
            name,
            flag,
            data_offset,
            data_length,
        }
    }

    /// Retrieves the data length.
    pub fn get_data_length(&self) -> u32 {
        self.data_length
    }

    /// Retrieves the data offset.
    pub fn get_data_offset(&self) -> u64 {
        self.data_offset
    }

    /// Retrieves the flag.
    pub fn get_flag(&self) -> u32 {
        self.flag
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let segment: AffSegment = AffSegment::new(String::from("page0"), 1, 878, 10109);

        assert_eq!(segment.get_name(), "page0");
        assert_eq!(segment.get_flag(), 1);
        assert_eq!(segment.get_data_offset(), 878);
        assert_eq!(segment.get_data_length(), 10109);
    }

    #[test]
    fn test_segment_header_read_data() -> Result<(), ErrorTrace> {
        let test_data: [u8; 16] = [
            b'A', b'F', b'F', 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00,
            0x00, 0x01,
        ];
        let mut header: AffSegmentHeader = AffSegmentHeader::new();
        header.read_data(&test_data)?;

        assert_eq!(header.name_length, 5);
        assert_eq!(header.data_length, 16);
        assert_eq!(header.flag, 1);

        // Test with unsupported data size.
        let result: Result<(), ErrorTrace> = header.read_data(&test_data[0..8]);
        assert!(result.is_err());

        // Test with unsupported signature.
        let mut bad_sig: [u8; 16] = test_data;
        bad_sig[0] = 0xff;
        let result: Result<(), ErrorTrace> = header.read_data(&bad_sig);
        assert!(result.is_err());

        #[cfg(feature = "debug-trace")]
        {
            let debug_string: String = AffSegmentHeader::debug_read_data(&test_data);
            assert!(!debug_string.is_empty());
        }
        Ok(())
    }

    #[test]
    fn test_segment_tail_read_data() -> Result<(), ErrorTrace> {
        let test_data: [u8; 8] = [b'A', b'T', b'T', 0x00, 0x00, 0x00, 0x00, 0x2d];
        let mut tail: AffSegmentTail = AffSegmentTail::new();
        tail.read_data(&test_data)?;

        assert_eq!(tail.segment_size, 45);

        // Test with unsupported data size.
        let result: Result<(), ErrorTrace> = tail.read_data(&test_data[0..4]);
        assert!(result.is_err());

        // Test with unsupported signature.
        let mut bad_sig: [u8; 8] = test_data;
        bad_sig[0] = 0xff;
        let result: Result<(), ErrorTrace> = tail.read_data(&bad_sig);
        assert!(result.is_err());

        #[cfg(feature = "debug-trace")]
        {
            let debug_string: String = AffSegmentTail::debug_read_data(&test_data);
            assert!(!debug_string.is_empty());
        }
        Ok(())
    }
}
