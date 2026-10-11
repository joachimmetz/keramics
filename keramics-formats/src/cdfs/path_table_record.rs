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

use keramics_encodings::CharacterEncoding;
use keramics_types::ByteString;

/// CD file system (CDFS) path table record.
pub struct CdFsPathTableRecord {
    /// Extended attribute record size.
    pub extended_attribute_record_size: u8,

    /// Directory start sector.
    pub directory_start_sector: u32,

    /// Parent directory number.
    pub parent_directory_number: u16,

    /// Name.
    pub name: ByteString,

    /// Size of the record, including padding.
    pub size: usize,
}

impl CdFsPathTableRecord {
    /// Creates a new path table record.
    pub fn new() -> Self {
        Self {
            extended_attribute_record_size: 0,
            directory_start_sector: 0,
            parent_directory_number: 0,
            name: ByteString::new_with_encoding(&CharacterEncoding::Ascii),
            size: 0,
        }
    }
}
