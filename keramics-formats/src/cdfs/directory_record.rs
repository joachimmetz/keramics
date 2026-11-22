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

use std::cmp::min;

use keramics_core::ErrorTrace;
use keramics_datetime::CdFsDateTime;
use keramics_encodings::CharacterEncoding;
use keramics_layout_map::LayoutMap;
use keramics_types::{
    ByteString, Ucs2String, bytes_to_u16_be, bytes_to_u16_le, bytes_to_u32_be, bytes_to_u32_le,
};

#[derive(Clone, LayoutMap)]
#[layout_map(
    structure(
        byte_order = "big",
        field(name = "record_size", data_type = "u8"),
        field(name = "extended_attribute_record_size", data_type = "u8"),
        field(
            name = "data_start_sector_le",
            data_type = "u32",
            byte_order = "little"
        ),
        field(name = "data_start_sector_be", data_type = "u32"),
        field(name = "data_size_le", data_type = "u32", byte_order = "little"),
        field(name = "data_size_be", data_type = "u32"),
        field(name = "recording_time", data_type = "CdFsDateTime"),
        field(name = "file_flags", data_type = "u8", format = "hex"),
        field(name = "file_unit_size", data_type = "u8"),
        field(name = "interleave_gap_size", data_type = "u8"),
        field(name = "volume_set_index_le", data_type = "u16", byte_order = "little"),
        field(name = "volume_set_index_be", data_type = "u16"),
        field(name = "name_size", data_type = "u8"),
        field(name = "name", data_type = "u8"),
    ),
    methods("debug_read_data")
)]
/// CD file system (CDFS) directory record.
pub struct CdFsDirectoryRecord {
    /// Record size.
    pub record_size: u8,

    /// Extended attribute record size.
    pub extended_attribute_record_size: u8,

    /// Data start sector.
    pub data_start_sector: u32,

    /// Data size.
    pub data_size: u32,

    /// Recording time.
    pub recording_time: CdFsDateTime,

    /// File flags.
    pub file_flags: u8,

    /// Volume set index.
    pub volume_set_index: u16,

    /// Name.
    pub name: ByteString,

    /// System Use area bytes (used by SUSP and Rock Ridge).
    pub system_use_data: Vec<u8>,
}

impl CdFsDirectoryRecord {
    /// Creates a new directory record.
    pub fn new() -> Self {
        Self {
            record_size: 0,
            extended_attribute_record_size: 0,
            data_start_sector: 0,
            data_size: 0,
            recording_time: CdFsDateTime::default(),
            file_flags: 0,
            volume_set_index: 0,
            name: ByteString::new_with_encoding(&CharacterEncoding::Ascii),
            system_use_data: Vec::new(),
        }
    }

    /// Reads the directory record from a buffer using ASCII name encoding.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        self.read_data_with_encoding(data, &CharacterEncoding::Ascii)
    }

    /// Reads the directory record from a buffer using the specified character encoding.
    pub fn read_data_with_encoding(
        &mut self,
        data: &[u8],
        encoding: &CharacterEncoding,
    ) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();

        if data_size < 33 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        self.record_size = data[0];
        self.extended_attribute_record_size = data[1];
        self.data_start_sector = bytes_to_u32_be!(data, 6);

        let data_start_sector: u32 = bytes_to_u32_le!(data, 2);
        if self.data_start_sector != data_start_sector {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian data start sector"
            ));
        }
        self.data_size = bytes_to_u32_be!(data, 14);

        let data_size_value: u32 = bytes_to_u32_le!(data, 10);
        if self.data_size != data_size_value {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian data size"
            ));
        }
        self.recording_time = CdFsDateTime::from_bytes(&data[18..25]);
        self.file_flags = data[25];

        self.volume_set_index = bytes_to_u16_be!(data, 30);

        let volume_set_index: u16 = bytes_to_u16_le!(data, 28);
        if self.volume_set_index != volume_set_index {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian volume set index"
            ));
        }
        let name_size: usize = data[32] as usize;
        let data_end_offset: usize = 33 + name_size;

        if data_end_offset > data_size {
            return Err(keramics_core::error_trace_new!(
                "Unsupported file identifier size value out of bounds"
            ));
        }
        let name_bytes: &[u8] = &data[33..data_end_offset];
        if *encoding == CharacterEncoding::Utf16BigEndian
            && !(name_size == 1 && (name_bytes[0] == 0x00 || name_bytes[0] == 0x01))
        {
            let even_length: usize = name_bytes.len() & !1;
            let ucs2_string: Ucs2String = Ucs2String::from_be_bytes(&name_bytes[..even_length]);
            let string: String = String::from_utf16_lossy(&ucs2_string.elements);
            self.name = ByteString::from(string.as_str());
        } else {
            self.name = ByteString::new_with_encoding(&CharacterEncoding::Ascii);
            self.name.read_data(name_bytes);
        }
        // System Use area begins after the file identifier and optional 1-byte even alignment pad.
        let system_use_offset: usize = data_end_offset + (data_end_offset & 1);
        let record_end_offset: usize = min(self.record_size as usize, data_size);

        if system_use_offset < record_end_offset {
            self.system_use_data = data[system_use_offset..record_end_offset].to_vec();
        } else {
            self.system_use_data.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_data() -> Vec<u8> {
        vec![
            0x29, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x08, 0x00, 0x00,
            0x00, 0x00, 0x08, 0x00, 0x1d, 0x09, 0x35, 0x1d, 0x02, 0x02, 0x00, 0x02, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x01, 0x08, 0x54, 0x45, 0x53, 0x54, 0x44, 0x49, 0x52, 0x31,
        ]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

        let test_data: Vec<u8> = get_test_data();
        test_struct.read_data(&test_data)?;

        assert_eq!(test_struct.record_size, 41);
        assert_eq!(test_struct.extended_attribute_record_size, 0);
        assert_eq!(test_struct.data_start_sector, 24);
        assert_eq!(test_struct.data_size, 2048);
        assert_eq!(test_struct.file_flags, 0x02);
        assert_eq!(test_struct.volume_set_index, 1);
        assert_eq!(
            test_struct.name,
            ByteString {
                encoding: CharacterEncoding::Ascii,
                elements: b"TESTDIR1".to_vec(),
            }
        );
        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

        let test_data: Vec<u8> = get_test_data();
        let result: Result<(), ErrorTrace> = test_struct.read_data(&test_data[0..32]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_data_start_sector_mismatch() {
        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

        let mut test_data: Vec<u8> = get_test_data();
        test_data[2] = 0xff;

        let result: Result<(), ErrorTrace> = test_struct.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_data_size_mismatch() {
        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

        let mut test_data: Vec<u8> = get_test_data();
        test_data[10] = 0xff;

        let result: Result<(), ErrorTrace> = test_struct.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_volume_set_index_mismatch() {
        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

        let mut test_data: Vec<u8> = get_test_data();
        test_data[28] = 0xff;

        let result: Result<(), ErrorTrace> = test_struct.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_name_size_out_of_bounds() {
        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

        let mut test_data: Vec<u8> = get_test_data();
        test_data[32] = 0xff;

        let result: Result<(), ErrorTrace> = test_struct.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_joliet_and_system_use() -> Result<(), ErrorTrace> {
        let mut test_data: Vec<u8> = vec![0; 48];
        test_data[0] = 48;
        test_data[2..6].copy_from_slice(&30u32.to_le_bytes());
        test_data[6..10].copy_from_slice(&30u32.to_be_bytes());
        test_data[10..14].copy_from_slice(&128u32.to_le_bytes());
        test_data[14..18].copy_from_slice(&128u32.to_be_bytes());
        test_data[18..25].copy_from_slice(&[126, 9, 29, 10, 20, 30, 0]);
        test_data[25] = 0x00;
        test_data[28..30].copy_from_slice(&1u16.to_le_bytes());
        test_data[30..32].copy_from_slice(&1u16.to_be_bytes());
        // UTF-16BE "Hi" (4 bytes, even -> 1 pad byte at offset 37, System Use starts at 38).
        test_data[32] = 4;
        test_data[33..37].copy_from_slice(&[0x00, b'H', 0x00, b'i']);
        test_data[38..48].copy_from_slice(&[b'S', b'P', 7, 1, 0xbe, 0xef, 0, 0, 0, 0]);

        let mut test_struct: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        test_struct.read_data_with_encoding(&test_data, &CharacterEncoding::Utf16BigEndian)?;

        assert_eq!(test_struct.recording_time.year, 2026);
        assert_eq!(test_struct.name.encoding, CharacterEncoding::Utf8);
        assert_eq!(test_struct.name.to_string(), "Hi");
        assert_eq!(test_struct.system_use_data.len(), 10);
        assert_eq!(&test_struct.system_use_data[0..2], b"SP");

        Ok(())
    }
}
