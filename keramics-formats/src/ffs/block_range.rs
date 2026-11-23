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

/// Fast File System (FFS) block range type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FfsBlockRangeType {
    /// Range backed by data in the volume stream.
    InFile,

    /// Unallocated sparse range reading as zero bytes.
    Sparse,
}

/// Fast File System (FFS) byte-addressed block or fragment range.
#[derive(Clone, Debug, PartialEq)]
pub struct FfsBlockRange {
    /// Logical byte offset within the stream.
    pub logical_offset: u64,

    /// Physical byte offset within the volume stream.
    pub physical_offset: u64,

    /// Size of the range in bytes.
    pub size: u64,

    /// Range type.
    pub range_type: FfsBlockRangeType,
}

impl FfsBlockRange {
    /// Creates a new block range.
    pub fn new(
        logical_offset: u64,
        physical_offset: u64,
        size: u64,
        range_type: FfsBlockRangeType,
    ) -> Self {
        Self {
            logical_offset,
            physical_offset,
            size,
            range_type,
        }
    }
}
