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

/// AFF file header signature.
pub(super) const AFF_FILE_SIGNATURE: [u8; 8] = *b"AFF10\r\n\0";

/// AFF file header signature size.
pub(super) const AFF_FILE_SIGNATURE_SIZE: u64 = 8;

/// AFF segment header signature.
pub(super) const AFF_SEGMENT_SIGNATURE: [u8; 4] = *b"AFF\0";

/// AFF segment header size.
pub(super) const AFF_SEGMENT_HEADER_SIZE: u64 = 16;

/// AFF segment tail signature.
pub(super) const AFF_SEGMENT_TAIL_SIGNATURE: [u8; 4] = *b"ATT\0";

/// AFF segment tail size.
pub(super) const AFF_SEGMENT_TAIL_SIZE: u64 = 8;

/// Maximum size of an AFF segment name in bytes.
pub(super) const AFF_MAX_SEGMENT_NAME_SIZE: u32 = 64;

/// AFF flag indicating that the segment data is stored compressed.
pub(super) const AFF_PAGE_FLAG_COMPRESSED: u32 = 0x0001;

/// AFF flag indicating that a segment contains a zero page or a 64-bit value.
pub(super) const AFF_PAGE_FLAG_ZERO: u32 = 0x0033;

/// Mask of the AFF flag bits that identify the compression algorithm.
pub(super) const AFF_PAGE_COMPRESSION_ALGORITHM_MASK: u32 = 0x00f0;

/// AFF compression algorithm for zlib compressed data.
pub(super) const AFF_PAGE_COMPRESSION_ALGORITHM_ZLIB: u32 = 0x0000;

/// AFF compression algorithm for bzip2 compressed data.
pub(super) const AFF_PAGE_COMPRESSION_ALGORITHM_BZIP2: u32 = 0x0010;

/// AFF compression algorithm for LZMA compressed data.
pub(super) const AFF_PAGE_COMPRESSION_ALGORITHM_LZMA: u32 = 0x0020;

/// AFF compression algorithm for a zero page.
pub(super) const AFF_PAGE_COMPRESSION_ALGORITHM_ZERO: u32 = 0x0030;

/// Default AFF page size in bytes (16 MiB).
pub(super) const AFF_DEFAULT_PAGE_SIZE: u64 = 0x1000000;

/// Default AFF bytes per sector.
pub(super) const AFF_DEFAULT_BYTES_PER_SECTOR: u32 = 0x200;
