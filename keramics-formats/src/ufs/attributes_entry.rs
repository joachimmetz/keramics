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
use keramics_types::ByteString;

use crate::ffs::FfsByteOrder;
use crate::indexed_hash_map::IndexedHashMap;

use super::constants::*;

/// Unix File System (UFS) version 2 extended attribute record.
#[derive(Clone, Debug)]
pub struct UfsAttributesEntry {
    /// Attribute value bytes.
    pub value_data: Vec<u8>,
}

impl UfsAttributesEntry {
    /// Creates a new attributes entry.
    pub fn new(value_data: Vec<u8>) -> Self {
        Self { value_data }
    }

    /// Parses packed UFS2 extended attribute records from a byte slice.
    pub fn read_entries(
        data: &[u8],
        byte_order: FfsByteOrder,
        attributes: &mut IndexedHashMap<ByteString, UfsAttributesEntry>,
    ) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();
        let mut data_offset: usize = 0;

        while data_offset + 7 <= data_size {
            let record_length: usize = byte_order.read_u32(data, data_offset) as usize;

            if record_length == 0 {
                break;
            }
            if record_length < 8 || data_offset + record_length > data_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Invalid extended attribute record length: {} value out of bounds",
                    record_length
                )));
            }
            let namespace_offset: usize = data_offset + 4;
            let content_padding_offset: usize = data_offset + 5;
            let name_length_offset: usize = data_offset + 6;

            let namespace: u8 = data[namespace_offset];
            let content_padding_length: usize = data[content_padding_offset] as usize;
            let name_length: usize = data[name_length_offset] as usize;

            let header_and_name_size: usize = (7 + name_length + 7) & !7;
            if header_and_name_size + content_padding_length > record_length {
                return Err(keramics_core::error_trace_new!(
                    "Invalid extended attribute name or padding length value out of bounds"
                ));
            }
            if namespace != UFS_EXTENDED_ATTRIBUTE_NAMESPACE_EMPTY {
                let name_start_offset: usize = data_offset + 7;
                let name_end_offset: usize = name_start_offset + name_length;
                let content_start_offset: usize = data_offset + header_and_name_size;
                let content_end_offset: usize =
                    (data_offset + record_length) - content_padding_length;

                let prefix: &[u8] = match namespace {
                    UFS_EXTENDED_ATTRIBUTE_NAMESPACE_USER => b"user.",
                    UFS_EXTENDED_ATTRIBUTE_NAMESPACE_SYSTEM => b"system.",
                    _ => b"",
                };
                let mut full_name_bytes: Vec<u8> = Vec::with_capacity(prefix.len() + name_length);
                full_name_bytes.extend_from_slice(prefix);
                full_name_bytes.extend_from_slice(&data[name_start_offset..name_end_offset]);

                let mut full_name: ByteString = ByteString::new();
                full_name.read_data(full_name_bytes.as_slice());

                let value_data: Vec<u8> = data[content_start_offset..content_end_offset].to_vec();
                let entry: UfsAttributesEntry = UfsAttributesEntry::new(value_data);

                attributes.insert(full_name, entry);
            }
            data_offset += record_length;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_entries() -> Result<(), ErrorTrace> {
        // Construct a single user.test = "hello" attribute record:
        // header (7) + "test" (4) = 11 -> aligned to 16 bytes;
        // content "hello" (5) + 3 padding bytes = 8 -> total record length = 24 bytes.
        let mut data: Vec<u8> = vec![0; 24];
        let record_length_bytes: [u8; 4] = 24_u32.to_le_bytes();
        data[0..4].copy_from_slice(&record_length_bytes);
        data[4] = UFS_EXTENDED_ATTRIBUTE_NAMESPACE_USER;
        data[5] = 3;
        data[6] = 4;
        data[7..11].copy_from_slice(b"test");
        data[16..21].copy_from_slice(b"hello");

        let mut attributes: IndexedHashMap<ByteString, UfsAttributesEntry> = IndexedHashMap::new();
        UfsAttributesEntry::read_entries(&data, FfsByteOrder::LittleEndian, &mut attributes)?;

        let count: usize = attributes.len();
        assert_eq!(count, 1);

        let key: ByteString = ByteString::from("user.test");
        let entry: &UfsAttributesEntry = attributes.get_value_by_key(&key).unwrap();
        let value_slice: &[u8] = entry.value_data.as_slice();
        assert_eq!(value_slice, b"hello");

        Ok(())
    }
}
