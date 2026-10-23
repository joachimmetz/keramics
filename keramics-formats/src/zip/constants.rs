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

/// Zip compression method value for stored (no compression)
pub const ZIP_COMPRESSION_METHOD_STORED: u16 = 0;

/// Zip compression method value for deflate
pub const ZIP_COMPRESSION_METHOD_DEFLATE: u16 = 8;

/// Zip central directory header signature
pub const ZIP_CENTRAL_DIRECTORY_HEADER_SIGNATURE: &[u8] = b"PK\x01\x02";

/// Zip local file header signature
pub const ZIP_LOCAL_FILE_HEADER_SIGNATURE: &[u8] = b"PK\x03\x04";

/// Zip end of central directory record signature
pub const ZIP_END_OF_CENTRAL_DIRECTORY_RECORD_SIGNATURE: &[u8] = b"PK\x05\x06";

/// Zip64 end of central directory record signature
pub const ZIP64_END_OF_CENTRAL_DIRECTORY_RECORD_SIGNATURE: &[u8] = b"PK\x06\x06";

/// Zip64 end of central directory locator signature
pub const ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE: &[u8] = b"PK\x06\x07";
