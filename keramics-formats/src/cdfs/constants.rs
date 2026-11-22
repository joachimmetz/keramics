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

/// CDFS volume descriptor signature.
pub(crate) const CDFS_VOLUME_DESCRIPTOR_SIGNATURE: &[u8; 5] = b"CD001";

/// CDFS volume descriptor set terminator.
pub(super) const CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR: &[u8; 6] = b"\xffCD001";

/// CDFS volume descriptor type indicator for a primary volume descriptor.
pub(super) const CDFS_VOLUME_DESCRIPTOR_TYPE_PRIMARY: u8 = 1;

/// CDFS volume descriptor type indicator for a supplementary volume descriptor.
pub(super) const CDFS_VOLUME_DESCRIPTOR_TYPE_SUPPLEMENTARY: u8 = 2;

/// CDFS file flag indicating the directory record represents a directory.
pub(super) const CDFS_FILE_FLAG_DIRECTORY: u8 = 0x02;

/// Joliet UCS-2 Level 1 escape sequence.
pub(super) const CDFS_JOLIET_ESCAPE_SEQUENCE_LEVEL1: &[u8; 3] = b"%/@";

/// Joliet UCS-2 Level 2 escape sequence.
pub(super) const CDFS_JOLIET_ESCAPE_SEQUENCE_LEVEL2: &[u8; 3] = b"%/C";

/// Joliet UCS-2 Level 3 escape sequence.
pub(super) const CDFS_JOLIET_ESCAPE_SEQUENCE_LEVEL3: &[u8; 3] = b"%/E";

/// SUSP Continuation Area entry signature.
pub(super) const CDFS_SUSP_SIGNATURE_CONTINUATION: &[u8; 2] = b"CE";

/// SUSP Padding Field entry signature.
pub(super) const CDFS_SUSP_SIGNATURE_PADDING: &[u8; 2] = b"PD";

/// SUSP System Use Sharing Protocol Indicator entry signature.
pub(super) const CDFS_SUSP_SIGNATURE_PROTOCOL_INDICATOR: &[u8; 2] = b"SP";

/// SUSP System Use Sharing Protocol Terminator entry signature.
pub(super) const CDFS_SUSP_SIGNATURE_TERMINATOR: &[u8; 2] = b"ST";

/// SUSP Extensions Reference entry signature.
pub(super) const CDFS_SUSP_SIGNATURE_EXTENSIONS_REFERENCE: &[u8; 2] = b"ER";

/// SUSP Extension Selector entry signature.
pub(super) const CDFS_SUSP_SIGNATURE_EXTENSION_SELECTOR: &[u8; 2] = b"ES";

/// RRIP POSIX File Attributes entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_POSIX_ATTRIBUTES: &[u8; 2] = b"PX";

/// RRIP POSIX Device Number entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_DEVICE_NUMBER: &[u8; 2] = &[0x50, 0x4e];

/// RRIP Symbolic Link entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_SYMBOLIC_LINK: &[u8; 2] = b"SL";

/// RRIP Alternate Name entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_ALTERNATE_NAME: &[u8; 2] = b"NM";

/// RRIP Child Link entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_CHILD_LINK: &[u8; 2] = b"CL";

/// RRIP Parent Link entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_PARENT_LINK: &[u8; 2] = b"PL";

/// RRIP Relocated Directory entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_RELOCATED_DIRECTORY: &[u8; 2] = b"RE";

/// RRIP Time Stamp(s) entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_TIME_STAMPS: &[u8; 2] = b"TF";

/// RRIP Rock Ridge Indicator entry signature.
pub(super) const CDFS_RRIP_SIGNATURE_ROCK_RIDGE_INDICATOR: &[u8; 2] = b"RR";

/// RRIP timestamp flag for creation time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_CREATION: u8 = 0x01;

/// RRIP timestamp flag for modification time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_MODIFY: u8 = 0x02;

/// RRIP timestamp flag for access time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_ACCESS: u8 = 0x04;

/// RRIP timestamp flag for attribute change time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_ATTRIBUTES: u8 = 0x08;

/// RRIP timestamp flag for backup time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_BACKUP: u8 = 0x10;

/// RRIP timestamp flag for expiration time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_EXPIRATION: u8 = 0x20;

/// RRIP timestamp flag for effective time.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_EFFECTIVE: u8 = 0x40;

/// RRIP timestamp flag indicating 17-byte long-form date and time strings.
pub(super) const CDFS_RRIP_TIME_STAMP_FLAG_LONG_FORM: u8 = 0x80;

/// ECMA-167 Beginning Extended Area Descriptor signature.
pub(crate) const UDF_VRS_SIGNATURE_BEA01: &[u8; 5] = b"BEA01";

/// ECMA-167 2nd edition Volume Structure Descriptor signature (UDF 1.02 and 1.50).
pub(super) const UDF_VRS_SIGNATURE_NSR02: &[u8; 5] = b"NSR02";

/// ECMA-167 3rd edition Volume Structure Descriptor signature (UDF 2.00 and later).
pub(super) const UDF_VRS_SIGNATURE_NSR03: &[u8; 5] = b"NSR03";

/// ECMA-167 Terminating Extended Area Descriptor signature.
pub(super) const UDF_VRS_SIGNATURE_TEA01: &[u8; 5] = b"TEA01";

/// UDF descriptor tag identifier for Primary Volume Descriptor.
pub(super) const UDF_TAG_PRIMARY_VOLUME_DESCRIPTOR: u16 = 1;

/// UDF descriptor tag identifier for Anchor Volume Descriptor Pointer.
pub(super) const UDF_TAG_ANCHOR_VOLUME_DESCRIPTOR_POINTER: u16 = 2;

/// UDF descriptor tag identifier for Partition Descriptor.
pub(super) const UDF_TAG_PARTITION_DESCRIPTOR: u16 = 5;

/// UDF descriptor tag identifier for Logical Volume Descriptor.
pub(super) const UDF_TAG_LOGICAL_VOLUME_DESCRIPTOR: u16 = 6;

/// UDF descriptor tag identifier for Terminating Descriptor.
pub(super) const UDF_TAG_TERMINATING_DESCRIPTOR: u16 = 8;

/// UDF descriptor tag identifier for File Set Descriptor.
pub(super) const UDF_TAG_FILE_SET_DESCRIPTOR: u16 = 256;

/// UDF descriptor tag identifier for File Identifier Descriptor.
pub(super) const UDF_TAG_FILE_IDENTIFIER_DESCRIPTOR: u16 = 257;

/// UDF descriptor tag identifier for File Entry.
pub(super) const UDF_TAG_FILE_ENTRY: u16 = 261;

/// UDF descriptor tag identifier for Extended File Entry.
pub(super) const UDF_TAG_EXTENDED_FILE_ENTRY: u16 = 266;

/// UDF file characteristic flag indicating a directory.
#[cfg(test)]
pub(super) const UDF_FILE_CHARACTERISTIC_DIRECTORY: u8 = 0x02;

/// UDF file characteristic flag indicating a deleted entry.
pub(super) const UDF_FILE_CHARACTERISTIC_DELETED: u8 = 0x04;

/// UDF file characteristic flag indicating the parent directory entry.
pub(super) const UDF_FILE_CHARACTERISTIC_PARENT: u8 = 0x08;

/// UDF ICB file type for a directory.
pub(super) const UDF_FILE_TYPE_DIRECTORY: u8 = 4;

/// UDF ICB file type for a regular file.
pub(super) const UDF_FILE_TYPE_REGULAR_FILE: u8 = 5;

/// UDF ICB file type for a block special device.
pub(super) const UDF_FILE_TYPE_BLOCK_DEVICE: u8 = 6;

/// UDF ICB file type for a character special device.
pub(super) const UDF_FILE_TYPE_CHARACTER_DEVICE: u8 = 7;

/// UDF ICB file type for a FIFO.
pub(super) const UDF_FILE_TYPE_FIFO: u8 = 9;

/// UDF ICB file type for a socket.
pub(super) const UDF_FILE_TYPE_SOCKET: u8 = 10;

/// UDF ICB file type for a symbolic link.
pub(super) const UDF_FILE_TYPE_SYMBOLIC_LINK: u8 = 12;
