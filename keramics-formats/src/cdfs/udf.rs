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
use std::io::SeekFrom;
use std::sync::{Arc, RwLock};

use keramics_core::{DataStreamReference, ErrorTrace, FakeDataStream};
use keramics_datetime::CdFsDateTime;
use keramics_encodings::CharacterEncoding;
use keramics_types::{ByteString, Ucs2String, bytes_to_u16_le, bytes_to_u32_le, bytes_to_u64_le};

use crate::block_stream::BlockStream;
use crate::traits::BlockReader;

use super::constants::*;

/// UDF descriptor tag (16 bytes).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct UdfDescriptorTag {
    /// Tag identifier.
    pub tag_identifier: u16,

    /// Descriptor version.
    pub descriptor_version: u16,

    /// Tag checksum.
    pub tag_checksum: u8,

    /// Tag serial number.
    pub tag_serial_number: u16,

    /// Descriptor CRC.
    pub descriptor_crc: u16,

    /// Descriptor CRC length.
    pub descriptor_crc_length: u16,

    /// Tag location.
    pub tag_location: u32,
}

impl UdfDescriptorTag {
    /// Calculates the CRC-ITU-T (polynomial 0x1021, initial value 0x0000) of a buffer.
    pub fn calculate_crc(data: &[u8]) -> u16 {
        let mut crc: u16 = 0;

        for byte in data.iter() {
            crc ^= (*byte as u16) << 8;
            for _ in 0..8 {
                if (crc & 0x8000) != 0 {
                    crc = (crc << 1) ^ 0x1021;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    /// Reads and validates a 16-byte UDF descriptor tag from a buffer.
    pub fn read_data(data: &[u8]) -> Result<Self, ErrorTrace> {
        if data.len() < 16 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF descriptor tag data size"
            ));
        }
        let mut checksum: u8 = 0;
        for byte_index in 0..16 {
            if byte_index != 4 {
                checksum = checksum.wrapping_add(data[byte_index]);
            }
        }
        let tag_checksum: u8 = data[4];
        if checksum != tag_checksum {
            return Err(keramics_core::error_trace_new!(format!(
                "Mismatch between stored: 0x{:02x} and calculated: 0x{:02x} UDF tag checksum",
                tag_checksum, checksum
            )));
        }
        let descriptor_crc: u16 = bytes_to_u16_le!(data, 8);
        let descriptor_crc_length: u16 = bytes_to_u16_le!(data, 10);

        if descriptor_crc_length > 0 {
            let crc_end_offset: usize = 16 + (descriptor_crc_length as usize);
            if crc_end_offset > data.len() {
                return Err(keramics_core::error_trace_new!(
                    "Unsupported UDF descriptor CRC length out of bounds"
                ));
            }
            let calculated_crc: u16 = Self::calculate_crc(&data[16..crc_end_offset]);
            if calculated_crc != descriptor_crc {
                return Err(keramics_core::error_trace_new!(format!(
                    "Mismatch between stored: 0x{:04x} and calculated: 0x{:04x} UDF descriptor CRC",
                    descriptor_crc, calculated_crc
                )));
            }
        }
        Ok(Self {
            tag_identifier: bytes_to_u16_le!(data, 0),
            descriptor_version: bytes_to_u16_le!(data, 2),
            tag_checksum,
            tag_serial_number: bytes_to_u16_le!(data, 6),
            descriptor_crc,
            descriptor_crc_length,
            tag_location: bytes_to_u32_le!(data, 12),
        })
    }
}

/// UDF extent descriptor (`extent_ad`, 8 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct UdfExtentAddress {
    /// Length in bytes.
    pub length: u32,

    /// Logical sector number.
    pub location: u32,
}

impl UdfExtentAddress {
    /// Reads an 8-byte extent address from a buffer.
    pub fn read_data(data: &[u8]) -> Result<Self, ErrorTrace> {
        if data.len() < 8 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF extent address data size"
            ));
        }
        Ok(Self {
            length: bytes_to_u32_le!(data, 0),
            location: bytes_to_u32_le!(data, 4),
        })
    }
}

/// UDF long allocation descriptor (`long_ad`, 16 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct UdfLongAllocationDescriptor {
    /// Extent length in bytes (lower 30 bits) and extent type (upper 2 bits).
    pub extent_length: u32,

    /// Logical block number within the partition.
    pub logical_block_number: u32,

    /// Partition reference number.
    pub partition_reference_number: u16,
}

impl UdfLongAllocationDescriptor {
    /// Retrieves the extent type (upper 2 bits: 0 = recorded and allocated).
    pub fn get_extent_type(&self) -> u8 {
        ((self.extent_length >> 30) & 0x03) as u8
    }

    /// Retrieves the extent length in bytes (lower 30 bits).
    pub fn get_length(&self) -> u32 {
        self.extent_length & 0x3fff_ffff
    }

    /// Reads a 16-byte long allocation descriptor from a buffer.
    pub fn read_data(data: &[u8]) -> Result<Self, ErrorTrace> {
        if data.len() < 16 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF long allocation descriptor data size"
            ));
        }
        Ok(Self {
            extent_length: bytes_to_u32_le!(data, 0),
            logical_block_number: bytes_to_u32_le!(data, 4),
            partition_reference_number: bytes_to_u16_le!(data, 8),
        })
    }
}

/// UDF partition entry.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct UdfPartition {
    /// Partition number.
    pub partition_number: u16,

    /// Starting logical sector number.
    pub starting_location: u32,

    /// Length in logical sectors.
    pub length: u32,
}

/// UDF data extent for reading file or directory streams.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct UdfDataExtent {
    /// Offset within the underlying data stream (`None` for sparse/zero-filled extents).
    pub data_offset: Option<u64>,

    /// Size of the extent in bytes.
    pub size: u64,
}

/// UDF file entry (parsed from tag 261 File Entry or tag 266 Extended File Entry).
#[derive(Clone, Debug)]
pub(super) struct UdfFileEntry {
    /// Byte offset of this file entry descriptor in the data stream.
    pub offset: u64,

    /// File type from ICB tag (`UDF_FILE_TYPE_*`).
    pub file_type: u8,

    /// POSIX-compatible file mode derived from file type and UDF permissions.
    pub file_mode: u32,

    /// Raw UDF permissions bitmask.
    pub permissions: u32,

    /// Owner user identifier.
    pub owner_identifier: u32,

    /// Group identifier.
    pub group_identifier: u32,

    /// File link count.
    pub number_of_links: u16,

    /// Logical file size in bytes.
    pub size: u64,

    /// Access time.
    pub access_time: Option<CdFsDateTime>,

    /// Modification time.
    pub modification_time: Option<CdFsDateTime>,

    /// Attribute change time.
    pub change_time: Option<CdFsDateTime>,

    /// Creation time (present on Extended File Entry, tag 266).
    pub creation_time: Option<CdFsDateTime>,

    /// Inline data (when ICB allocation descriptor type is 3).
    pub inline_data: Option<Vec<u8>>,

    /// Data extents (when ICB allocation descriptor type is 0 or 1).
    pub extents: Vec<UdfDataExtent>,
}

/// UDF directory entry parsed from a File Identifier Descriptor (tag 257).
#[derive(Clone, Debug)]
pub(super) struct UdfDirectoryEntry {
    /// File name decoded from OSTA Compressed Unicode (CS0).
    pub name: ByteString,

    /// Target file entry ICB location.
    pub icb: UdfLongAllocationDescriptor,
}

/// UDF block reader for reading multi-extent or sparse UDF file data.
pub(super) struct UdfBlockReader {
    /// Underlying volume data stream.
    data_stream: DataStreamReference,

    /// Data extents.
    extents: Vec<UdfDataExtent>,

    /// Total logical size in bytes.
    size: u64,
}

impl UdfBlockReader {
    /// Creates a new UDF block reader.
    pub fn new(data_stream: &DataStreamReference, extents: Vec<UdfDataExtent>, size: u64) -> Self {
        Self {
            data_stream: data_stream.clone(),
            extents,
            size,
        }
    }
}

impl BlockReader for UdfBlockReader {
    /// Retrieves the size of the data.
    fn get_size(&self) -> u64 {
        self.size
    }

    /// Reads data from the extents at the specified offset.
    fn read_data_from_blocks(&mut self, data: &mut [u8], offset: u64) -> Result<usize, ErrorTrace> {
        if data.is_empty() || offset >= self.size {
            return Ok(0);
        }
        let target_read_size: usize = min(data.len(), (self.size - offset) as usize);
        let mut bytes_read: usize = 0;
        let mut current_position: u64 = offset;
        let mut logical_extent_start: u64 = 0;

        for extent in self.extents.iter() {
            let logical_extent_end: u64 = logical_extent_start + extent.size;

            if current_position < logical_extent_end {
                let offset_in_extent: u64 = current_position - logical_extent_start;
                let available: usize = min(
                    target_read_size - bytes_read,
                    (extent.size - offset_in_extent) as usize,
                );
                match extent.data_offset {
                    Some(physical_offset) => {
                        keramics_core::data_stream_read_exact_at_position!(
                            &self.data_stream,
                            &mut data[bytes_read..bytes_read + available],
                            SeekFrom::Start(physical_offset + offset_in_extent)
                        );
                    }
                    None => {
                        data[bytes_read..bytes_read + available].fill(0);
                    }
                }
                bytes_read += available;
                current_position += available as u64;

                if bytes_read >= target_read_size {
                    break;
                }
            }
            logical_extent_start = logical_extent_end;
        }
        // Zero-fill any trailing bytes beyond the last extent up to the logical file size.
        if bytes_read < target_read_size {
            data[bytes_read..target_read_size].fill(0);
            bytes_read = target_read_size;
        }
        Ok(bytes_read)
    }
}

/// UDF volume metadata and reader.
#[derive(Clone)]
pub(super) struct UdfVolume {
    /// Underlying data stream.
    data_stream: DataStreamReference,

    /// ECMA-167 NSR revision (`2` for NSR02, `3` for NSR03).
    pub nsr_version: u8,

    /// Logical block size in bytes.
    pub block_size: u32,

    /// Volume identifier from the Primary Volume Descriptor.
    pub volume_identifier: Option<ByteString>,

    /// Volume set identifier from the Primary Volume Descriptor.
    pub volume_set_identifier: Option<ByteString>,

    /// Logical volume identifier from the Logical Volume Descriptor.
    pub logical_volume_identifier: Option<ByteString>,

    /// Partitions indexed by partition map index.
    partitions: Vec<UdfPartition>,

    /// Root directory ICB location from the File Set Descriptor.
    pub root_directory_icb: UdfLongAllocationDescriptor,
}

impl UdfVolume {
    /// Converts UDF permissions and file type into a POSIX file mode.
    fn convert_permissions_to_posix_mode(file_type: u8, permissions: u32) -> u32 {
        let mode_type: u32 = match file_type {
            UDF_FILE_TYPE_DIRECTORY => 0o040000,
            UDF_FILE_TYPE_REGULAR_FILE => 0o100000,
            UDF_FILE_TYPE_BLOCK_DEVICE => 0o060000,
            UDF_FILE_TYPE_CHARACTER_DEVICE => 0o020000,
            UDF_FILE_TYPE_FIFO => 0o010000,
            UDF_FILE_TYPE_SOCKET => 0o140000,
            UDF_FILE_TYPE_SYMBOLIC_LINK => 0o120000,
            _ => 0,
        };
        let mut mode_bits: u32 = 0;
        // Other permissions (bits 0..4: execute, write, read, change attributes, delete).
        if (permissions & 0x0001) != 0 {
            mode_bits |= 0o0001;
        }
        if (permissions & 0x0002) != 0 {
            mode_bits |= 0o0002;
        }
        if (permissions & 0x0004) != 0 {
            mode_bits |= 0o0004;
        }
        // Group permissions (bits 5..9: execute, write, read, change attributes, delete).
        if (permissions & 0x0020) != 0 {
            mode_bits |= 0o0010;
        }
        if (permissions & 0x0040) != 0 {
            mode_bits |= 0o0020;
        }
        if (permissions & 0x0080) != 0 {
            mode_bits |= 0o0040;
        }
        // Owner permissions (bits 10..14: execute, write, read, change attributes, delete).
        if (permissions & 0x0400) != 0 {
            mode_bits |= 0o0100;
        }
        if (permissions & 0x0800) != 0 {
            mode_bits |= 0o0200;
        }
        if (permissions & 0x1000) != 0 {
            mode_bits |= 0o0400;
        }
        mode_type | mode_bits
    }

    /// Decodes an OSTA Compressed Unicode (CS0) byte slice.
    pub fn decode_cs0(data: &[u8]) -> Result<ByteString, ErrorTrace> {
        if data.is_empty() {
            return Ok(ByteString::new_with_encoding(&CharacterEncoding::Utf8));
        }
        match data[0] {
            0x08 => {
                let mut byte_string: ByteString =
                    ByteString::new_with_encoding(&CharacterEncoding::Iso8859_1);
                byte_string.elements.extend_from_slice(&data[1..]);
                Ok(byte_string)
            }
            0x10 => {
                let payload_length: usize = (data.len() - 1) & !1;
                let ucs2_string: Ucs2String =
                    Ucs2String::from_be_bytes(&data[1..1 + payload_length]);
                let string: String = String::from_utf16_lossy(&ucs2_string.elements);
                Ok(ByteString::from(string.as_str()))
            }
            compression_identifier => Err(keramics_core::error_trace_new!(format!(
                "Unsupported UDF CS0 compression identifier: 0x{:02x}",
                compression_identifier
            ))),
        }
    }

    /// Decodes a fixed-size UDF `dstring` (last byte holds the byte length including compression ID).
    pub fn decode_dstring(data: &[u8]) -> Option<ByteString> {
        if data.is_empty() {
            return None;
        }
        let length: usize = data[data.len() - 1] as usize;
        if length == 0 || length >= data.len() {
            return None;
        }
        match Self::decode_cs0(&data[..length]) {
            Ok(byte_string) if !byte_string.is_empty() => Some(byte_string),
            _ => None,
        }
    }

    /// Detects an ECMA-167 Volume Recognition Sequence (`BEA01`, `NSR02`/`NSR03`, `TEA01`) starting
    /// at the specified byte offset. Returns `Some(2)` for `NSR02` or `Some(3)` for `NSR03`.
    pub fn detect_volume_recognition_sequence(
        data_stream: &DataStreamReference,
        start_offset: u64,
        sector_size: u64,
    ) -> Result<Option<u8>, ErrorTrace> {
        let stream_size: u64 = keramics_core::data_stream_get_size!(data_stream);
        let mut offset: u64 = start_offset;
        let mut data: [u8; 7] = [0; 7];
        let mut found_beginning_extended_area: bool = false;
        let mut nsr_version: Option<u8> = None;

        // Scan up to 32 descriptors in the Volume Recognition Sequence.
        for _ in 0..32 {
            if offset + sector_size > stream_size {
                break;
            }
            keramics_core::data_stream_read_exact_at_position!(
                data_stream,
                &mut data,
                SeekFrom::Start(offset)
            );
            let signature: &[u8] = &data[1..6];

            if signature == UDF_VRS_SIGNATURE_BEA01 {
                found_beginning_extended_area = true;
            } else if found_beginning_extended_area && signature == UDF_VRS_SIGNATURE_NSR02 {
                nsr_version = Some(2);
            } else if found_beginning_extended_area && signature == UDF_VRS_SIGNATURE_NSR03 {
                nsr_version = Some(3);
            } else if found_beginning_extended_area && signature == UDF_VRS_SIGNATURE_TEA01 {
                break;
            } else if signature == CDFS_VOLUME_DESCRIPTOR_SIGNATURE {
                // Skip ISO 9660 volume descriptors if scanning from sector 16.
            } else {
                break;
            }
            offset += sector_size;
        }
        if found_beginning_extended_area {
            Ok(nsr_version)
        } else {
            Ok(None)
        }
    }

    /// Retrieves a [`DataStreamReference`] for a UDF file entry.
    pub fn get_file_entry_data_stream(&self, file_entry: &UdfFileEntry) -> DataStreamReference {
        if let Some(inline_data) = &file_entry.inline_data {
            return Arc::new(RwLock::new(FakeDataStream::new(
                inline_data.as_slice(),
                file_entry.size,
            )));
        }
        let block_reader: UdfBlockReader = UdfBlockReader::new(
            &self.data_stream,
            file_entry.extents.clone(),
            file_entry.size,
        );
        Arc::new(RwLock::new(BlockStream::new(block_reader)))
    }

    /// Reads an Anchor Volume Descriptor Pointer (AVDP) from logical sector 256 (or backup sectors).
    fn read_anchor_volume_descriptor_pointer(
        data_stream: &DataStreamReference,
        sector_size: u64,
    ) -> Result<(UdfExtentAddress, UdfExtentAddress), ErrorTrace> {
        let stream_size: u64 = keramics_core::data_stream_get_size!(data_stream);
        let number_of_sectors: u64 = stream_size / sector_size;

        let mut candidate_sectors: Vec<u64> = vec![256];
        if number_of_sectors > 0 {
            let last_sector: u64 = number_of_sectors - 1;
            if !candidate_sectors.contains(&last_sector) {
                candidate_sectors.push(last_sector);
            }
            if last_sector >= 256 {
                let backup_sector: u64 = last_sector - 256;
                if !candidate_sectors.contains(&backup_sector) {
                    candidate_sectors.push(backup_sector);
                }
            }
        }
        let mut buffer: Vec<u8> = vec![0; sector_size as usize];

        for sector in candidate_sectors {
            let offset: u64 = sector * sector_size;
            if offset + sector_size > stream_size {
                continue;
            }
            keramics_core::data_stream_read_exact_at_position!(
                data_stream,
                &mut buffer,
                SeekFrom::Start(offset)
            );
            if let Ok(tag) = UdfDescriptorTag::read_data(&buffer) {
                if tag.tag_identifier == UDF_TAG_ANCHOR_VOLUME_DESCRIPTOR_POINTER {
                    let main_volume_descriptor_sequence: UdfExtentAddress =
                        match UdfExtentAddress::read_data(&buffer[16..24]) {
                            Ok(extent) => extent,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read UDF Main VDS extent address"
                                );
                                return Err(error);
                            }
                        };
                    let reserve_volume_descriptor_sequence: UdfExtentAddress =
                        match UdfExtentAddress::read_data(&buffer[24..32]) {
                            Ok(extent) => extent,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read UDF Reserve VDS extent address"
                                );
                                return Err(error);
                            }
                        };
                    return Ok((
                        main_volume_descriptor_sequence,
                        reserve_volume_descriptor_sequence,
                    ));
                }
            }
        }
        Err(keramics_core::error_trace_new!(
            "Missing UDF Anchor Volume Descriptor Pointer"
        ))
    }

    /// Reads a UDF volume from a data stream.
    pub fn read_data_stream(
        data_stream: &DataStreamReference,
        nsr_version: u8,
        sector_size: u64,
    ) -> Result<Self, ErrorTrace> {
        let (main_volume_descriptor_sequence, reserve_volume_descriptor_sequence): (
            UdfExtentAddress,
            UdfExtentAddress,
        ) = match Self::read_anchor_volume_descriptor_pointer(data_stream, sector_size) {
            Ok(extents) => extents,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read UDF Anchor Volume Descriptor Pointer"
                );
                return Err(error);
            }
        };

        let (
            block_size,
            volume_identifier,
            volume_set_identifier,
            logical_volume_identifier,
            partitions,
            file_set_descriptor_location,
        ): (
            u32,
            Option<ByteString>,
            Option<ByteString>,
            Option<ByteString>,
            Vec<UdfPartition>,
            UdfLongAllocationDescriptor,
        ) = match Self::read_volume_descriptor_sequence(
            data_stream,
            sector_size,
            main_volume_descriptor_sequence,
        ) {
            Ok(result) => result,
            Err(main_error) => {
                if reserve_volume_descriptor_sequence.length > 0 {
                    match Self::read_volume_descriptor_sequence(
                        data_stream,
                        sector_size,
                        reserve_volume_descriptor_sequence,
                    ) {
                        Ok(result) => result,
                        Err(_) => return Err(main_error),
                    }
                } else {
                    return Err(main_error);
                }
            }
        };

        let mut udf_volume: Self = Self {
            data_stream: data_stream.clone(),
            nsr_version,
            block_size,
            volume_identifier,
            volume_set_identifier,
            logical_volume_identifier,
            partitions,
            root_directory_icb: UdfLongAllocationDescriptor::default(),
        };

        // Read the File Set Descriptor (tag 256) to locate the root directory ICB.
        let file_set_descriptor_offset: u64 = match udf_volume.resolve_partition_block_offset(
            file_set_descriptor_location.partition_reference_number,
            file_set_descriptor_location.logical_block_number,
        ) {
            Ok(offset) => offset,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to resolve UDF File Set Descriptor location"
                );
                return Err(error);
            }
        };
        let mut file_set_descriptor_data: Vec<u8> = vec![0; block_size as usize];
        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut file_set_descriptor_data,
            SeekFrom::Start(file_set_descriptor_offset)
        );
        let file_set_descriptor_tag: UdfDescriptorTag =
            match UdfDescriptorTag::read_data(&file_set_descriptor_data) {
                Ok(tag) => tag,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read UDF File Set Descriptor tag"
                    );
                    return Err(error);
                }
            };
        if file_set_descriptor_tag.tag_identifier != UDF_TAG_FILE_SET_DESCRIPTOR {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported UDF File Set Descriptor tag identifier: {}",
                file_set_descriptor_tag.tag_identifier
            )));
        }
        if file_set_descriptor_data.len() < 416 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF File Set Descriptor size"
            ));
        }
        udf_volume.root_directory_icb =
            match UdfLongAllocationDescriptor::read_data(&file_set_descriptor_data[400..416]) {
                Ok(icb) => icb,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read UDF root directory ICB"
                    );
                    return Err(error);
                }
            };

        Ok(udf_volume)
    }

    /// Reads the directory entries (File Identifier Descriptors) of a UDF directory file entry.
    pub fn read_directory_entries(
        &self,
        directory_entry: &UdfFileEntry,
    ) -> Result<Vec<UdfDirectoryEntry>, ErrorTrace> {
        let data: Vec<u8> = match self.read_file_entry_bytes(directory_entry) {
            Ok(data) => data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read UDF directory data");
                return Err(error);
            }
        };
        let mut entries: Vec<UdfDirectoryEntry> = Vec::new();
        let mut offset: usize = 0;
        let data_size: usize = data.len();

        while offset + 38 <= data_size {
            let tag: UdfDescriptorTag = match UdfDescriptorTag::read_data(&data[offset..]) {
                Ok(tag) => tag,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read UDF File Identifier Descriptor tag at offset: {}",
                            offset
                        )
                    );
                    return Err(error);
                }
            };
            if tag.tag_identifier != UDF_TAG_FILE_IDENTIFIER_DESCRIPTOR {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported UDF File Identifier Descriptor tag identifier: {}",
                    tag.tag_identifier
                )));
            }
            let file_characteristics: u8 = data[offset + 18];
            let file_identifier_length: usize = data[offset + 19] as usize;
            let icb: UdfLongAllocationDescriptor =
                match UdfLongAllocationDescriptor::read_data(&data[offset + 20..offset + 36]) {
                    Ok(icb) => icb,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to read UDF File Identifier Descriptor ICB"
                        );
                        return Err(error);
                    }
                };
            let implementation_use_length: usize = bytes_to_u16_le!(&data, offset + 36) as usize;

            let name_start: usize = offset + 38 + implementation_use_length;
            let name_end: usize = name_start + file_identifier_length;

            if name_end > data_size {
                return Err(keramics_core::error_trace_new!(
                    "Unsupported UDF File Identifier Descriptor length out of bounds"
                ));
            }
            let unpadded_size: usize = 38 + implementation_use_length + file_identifier_length;
            let padded_size: usize = (unpadded_size + 3) & !3;

            let is_deleted: bool = (file_characteristics & UDF_FILE_CHARACTERISTIC_DELETED) != 0;
            let is_parent: bool = (file_characteristics & UDF_FILE_CHARACTERISTIC_PARENT) != 0;

            if !is_deleted && !is_parent && file_identifier_length > 0 {
                let name: ByteString = match Self::decode_cs0(&data[name_start..name_end]) {
                    Ok(name) => name,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to decode UDF file identifier"
                        );
                        return Err(error);
                    }
                };
                entries.push(UdfDirectoryEntry { name, icb });
            }
            offset += padded_size;
        }
        Ok(entries)
    }

    /// Reads a UDF File Entry (tag 261) or Extended File Entry (tag 266) at the specified ICB.
    pub fn read_file_entry(
        &self,
        icb: &UdfLongAllocationDescriptor,
    ) -> Result<UdfFileEntry, ErrorTrace> {
        let entry_offset: u64 = match self.resolve_partition_block_offset(
            icb.partition_reference_number,
            icb.logical_block_number,
        ) {
            Ok(offset) => offset,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to resolve UDF ICB location");
                return Err(error);
            }
        };
        let mut data: Vec<u8> = vec![0; self.block_size as usize];
        keramics_core::data_stream_read_exact_at_position!(
            &self.data_stream,
            &mut data,
            SeekFrom::Start(entry_offset)
        );
        let tag: UdfDescriptorTag = match UdfDescriptorTag::read_data(&data) {
            Ok(tag) => tag,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read UDF file entry tag");
                return Err(error);
            }
        };
        let is_extended: bool = match tag.tag_identifier {
            UDF_TAG_FILE_ENTRY => false,
            UDF_TAG_EXTENDED_FILE_ENTRY => true,
            tag_identifier => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported UDF file entry tag identifier: {}",
                    tag_identifier
                )));
            }
        };
        let minimum_size: usize = if is_extended { 216 } else { 176 };
        if data.len() < minimum_size {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF file entry size"
            ));
        }
        // ICB tag is 20 bytes at offset 16..36.
        let file_type: u8 = data[27];
        let icb_flags: u16 = bytes_to_u16_le!(&data, 34);
        let allocation_descriptor_type: u16 = icb_flags & 0x0007;

        let owner_identifier: u32 = bytes_to_u32_le!(&data, 36);
        let group_identifier: u32 = bytes_to_u32_le!(&data, 40);
        let permissions: u32 = bytes_to_u32_le!(&data, 44);
        let number_of_links: u16 = bytes_to_u16_le!(&data, 48);
        let size: u64 = bytes_to_u64_le!(&data, 56);

        let (
            access_time,
            modification_time,
            creation_time,
            change_time,
            extended_attributes_size,
            allocation_descriptors_size,
            extended_attributes_offset,
        ): (
            Option<CdFsDateTime>,
            Option<CdFsDateTime>,
            Option<CdFsDateTime>,
            Option<CdFsDateTime>,
            usize,
            usize,
            usize,
        ) = if is_extended {
            (
                Self::read_time_stamp(&data[80..92]),
                Self::read_time_stamp(&data[92..104]),
                Self::read_time_stamp(&data[104..116]),
                Self::read_time_stamp(&data[116..128]),
                bytes_to_u32_le!(&data, 208) as usize,
                bytes_to_u32_le!(&data, 212) as usize,
                216,
            )
        } else {
            (
                Self::read_time_stamp(&data[72..84]),
                Self::read_time_stamp(&data[84..96]),
                None,
                Self::read_time_stamp(&data[96..108]),
                bytes_to_u32_le!(&data, 168) as usize,
                bytes_to_u32_le!(&data, 172) as usize,
                176,
            )
        };
        let allocation_descriptors_offset: usize =
            extended_attributes_offset + extended_attributes_size;
        let allocation_descriptors_end_offset: usize =
            allocation_descriptors_offset + allocation_descriptors_size;

        if allocation_descriptors_end_offset > data.len() {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF allocation descriptors out of bounds"
            ));
        }
        let allocation_descriptors_data: &[u8] =
            &data[allocation_descriptors_offset..allocation_descriptors_end_offset];
        let mut inline_data: Option<Vec<u8>> = None;
        let mut extents: Vec<UdfDataExtent> = Vec::new();

        match allocation_descriptor_type {
            0 => {
                // Short allocation descriptors (8 bytes each, same partition as the ICB).
                let mut offset: usize = 0;
                while offset + 8 <= allocation_descriptors_data.len() {
                    let raw_length: u32 = bytes_to_u32_le!(allocation_descriptors_data, offset);
                    let logical_block: u32 =
                        bytes_to_u32_le!(allocation_descriptors_data, offset + 4);
                    let extent_size: u64 = (raw_length & 0x3fff_ffff) as u64;
                    let extent_type: u8 = ((raw_length >> 30) & 0x03) as u8;

                    if extent_size > 0 {
                        let data_offset: Option<u64> = if extent_type == 0 {
                            match self.resolve_partition_block_offset(
                                icb.partition_reference_number,
                                logical_block,
                            ) {
                                Ok(resolved_offset) => Some(resolved_offset),
                                Err(mut error) => {
                                    keramics_core::error_trace_add_frame!(
                                        error,
                                        "Unable to resolve UDF short allocation descriptor offset"
                                    );
                                    return Err(error);
                                }
                            }
                        } else {
                            None
                        };
                        extents.push(UdfDataExtent {
                            data_offset,
                            size: extent_size,
                        });
                    }
                    offset += 8;
                }
            }
            1 => {
                // Long allocation descriptors (16 bytes each).
                let mut offset: usize = 0;
                while offset + 16 <= allocation_descriptors_data.len() {
                    let long_allocation_descriptor: UdfLongAllocationDescriptor =
                        match UdfLongAllocationDescriptor::read_data(
                            &allocation_descriptors_data[offset..offset + 16],
                        ) {
                            Ok(descriptor) => descriptor,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read UDF long allocation descriptor"
                                );
                                return Err(error);
                            }
                        };
                    let extent_size: u64 = long_allocation_descriptor.get_length() as u64;
                    let extent_type: u8 = long_allocation_descriptor.get_extent_type();

                    if extent_size > 0 {
                        let data_offset: Option<u64> = if extent_type == 0 {
                            match self.resolve_partition_block_offset(
                                long_allocation_descriptor.partition_reference_number,
                                long_allocation_descriptor.logical_block_number,
                            ) {
                                Ok(resolved_offset) => Some(resolved_offset),
                                Err(mut error) => {
                                    keramics_core::error_trace_add_frame!(
                                        error,
                                        "Unable to resolve UDF long allocation descriptor offset"
                                    );
                                    return Err(error);
                                }
                            }
                        } else {
                            None
                        };
                        extents.push(UdfDataExtent {
                            data_offset,
                            size: extent_size,
                        });
                    }
                    offset += 16;
                }
            }
            3 => {
                // Inline data embedded directly in the allocation descriptors region.
                let inline_size: usize = min(size as usize, allocation_descriptors_data.len());
                inline_data = Some(allocation_descriptors_data[..inline_size].to_vec());
            }
            _ => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported UDF allocation descriptor type: {}",
                    allocation_descriptor_type
                )));
            }
        }
        let file_mode: u32 = Self::convert_permissions_to_posix_mode(file_type, permissions);

        Ok(UdfFileEntry {
            offset: entry_offset,
            file_type,
            file_mode,
            permissions,
            owner_identifier,
            group_identifier,
            number_of_links,
            size,
            access_time,
            modification_time,
            change_time,
            creation_time,
            inline_data,
            extents,
        })
    }

    /// Reads the complete byte contents of a UDF file entry (used for directories and symbolic links).
    pub fn read_file_entry_bytes(&self, file_entry: &UdfFileEntry) -> Result<Vec<u8>, ErrorTrace> {
        if let Some(inline_data) = &file_entry.inline_data {
            return Ok(inline_data.clone());
        }
        let mut reader: UdfBlockReader = UdfBlockReader::new(
            &self.data_stream,
            file_entry.extents.clone(),
            file_entry.size,
        );
        let mut data: Vec<u8> = vec![0; file_entry.size as usize];
        match reader.read_data_from_blocks(&mut data, 0) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read UDF file entry bytes");
                return Err(error);
            }
        }
        Ok(data)
    }

    /// Reads a UDF symbolic link target from a symbolic link file entry's path component records.
    pub fn read_symbolic_link_target(
        &self,
        file_entry: &UdfFileEntry,
    ) -> Result<Option<ByteString>, ErrorTrace> {
        if file_entry.file_type != UDF_FILE_TYPE_SYMBOLIC_LINK {
            return Ok(None);
        }
        let data: Vec<u8> = match self.read_file_entry_bytes(file_entry) {
            Ok(data) => data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read UDF symbolic link target data"
                );
                return Err(error);
            }
        };
        let mut parts: Vec<String> = Vec::new();
        let mut offset: usize = 0;

        while offset + 4 <= data.len() {
            let component_type: u8 = data[offset];
            let component_length: usize = data[offset + 1] as usize;
            let component_end: usize = offset + 4 + component_length;

            if component_end > data.len() {
                break;
            }
            match component_type {
                1 | 2 => parts.push(String::new()),
                3 => parts.push(String::from("..")),
                4 => parts.push(String::from(".")),
                5 => {
                    let component_name: ByteString =
                        match Self::decode_cs0(&data[offset + 4..component_end]) {
                            Ok(name) => name,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to decode UDF symbolic link path component"
                                );
                                return Err(error);
                            }
                        };
                    parts.push(component_name.to_string());
                }
                _ => {}
            }
            offset = component_end;
        }
        let target: String = if parts.len() == 1 && parts[0].is_empty() {
            String::from("/")
        } else {
            parts.join("/")
        };
        let mut byte_string: ByteString = ByteString::new_with_encoding(&CharacterEncoding::Utf8);
        byte_string.elements = target.into_bytes();

        Ok(Some(byte_string))
    }

    /// Reads an ECMA-167 12-byte timestamp into a [`CdFsDateTime`].
    pub fn read_time_stamp(data: &[u8]) -> Option<CdFsDateTime> {
        if data.len() < 12 {
            return None;
        }
        let year: u16 = bytes_to_u16_le!(data, 2);
        let month: u8 = data[4];
        let day_of_month: u8 = data[5];
        let hours: u8 = data[6];
        let minutes: u8 = data[7];
        let seconds: u8 = data[8];
        let centiseconds: u8 = data[9];

        if year == 0 && month == 0 && day_of_month == 0 {
            return None;
        }
        let type_and_time_zone: u16 = bytes_to_u16_le!(data, 0);
        let time_zone_type: u16 = (type_and_time_zone >> 12) & 0x0f;
        let raw_time_zone: i16 = (type_and_time_zone & 0x0fff) as i16;
        let signed_time_zone_minutes: i16 = if (raw_time_zone & 0x0800) != 0 {
            raw_time_zone | !0x0fff
        } else {
            raw_time_zone
        };
        let utc_offset: i8 = if time_zone_type == 1 && signed_time_zone_minutes != -2047 {
            (signed_time_zone_minutes / 15) as i8
        } else {
            0
        };
        Some(CdFsDateTime {
            year,
            month,
            day_of_month,
            hours,
            minutes,
            seconds,
            fraction: Some(centiseconds),
            utc_offset,
        })
    }

    /// Reads a Volume Descriptor Sequence (VDS).
    fn read_volume_descriptor_sequence(
        data_stream: &DataStreamReference,
        sector_size: u64,
        volume_descriptor_sequence_extent: UdfExtentAddress,
    ) -> Result<
        (
            u32,
            Option<ByteString>,
            Option<ByteString>,
            Option<ByteString>,
            Vec<UdfPartition>,
            UdfLongAllocationDescriptor,
        ),
        ErrorTrace,
    > {
        if volume_descriptor_sequence_extent.length == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported UDF Volume Descriptor Sequence length"
            ));
        }
        let mut volume_identifier: Option<ByteString> = None;
        let mut volume_set_identifier: Option<ByteString> = None;
        let mut logical_volume_identifier: Option<ByteString> = None;
        let mut block_size: u32 = sector_size as u32;
        let mut file_set_descriptor_location: Option<UdfLongAllocationDescriptor> = None;
        let mut raw_partitions: Vec<UdfPartition> = Vec::new();
        let mut partition_map_numbers: Vec<u16> = Vec::new();

        let number_of_sectors: u64 =
            (volume_descriptor_sequence_extent.length as u64) / sector_size;
        let mut buffer: Vec<u8> = vec![0; sector_size as usize];

        for sector_index in 0..number_of_sectors {
            let offset: u64 =
                ((volume_descriptor_sequence_extent.location as u64) + sector_index) * sector_size;
            keramics_core::data_stream_read_exact_at_position!(
                data_stream,
                &mut buffer,
                SeekFrom::Start(offset)
            );
            let tag: UdfDescriptorTag = match UdfDescriptorTag::read_data(&buffer) {
                Ok(tag) => tag,
                Err(_) => break,
            };
            match tag.tag_identifier {
                UDF_TAG_PRIMARY_VOLUME_DESCRIPTOR => {
                    if buffer.len() >= 340 {
                        volume_identifier = Self::decode_dstring(&buffer[24..56]);
                        volume_set_identifier = Self::decode_dstring(&buffer[212..340]);
                    }
                }
                UDF_TAG_PARTITION_DESCRIPTOR => {
                    if buffer.len() >= 196 {
                        raw_partitions.push(UdfPartition {
                            partition_number: bytes_to_u16_le!(&buffer, 22),
                            starting_location: bytes_to_u32_le!(&buffer, 188),
                            length: bytes_to_u32_le!(&buffer, 192),
                        });
                    }
                }
                UDF_TAG_LOGICAL_VOLUME_DESCRIPTOR => {
                    if buffer.len() >= 440 {
                        logical_volume_identifier = Self::decode_dstring(&buffer[84..212]);
                        let parsed_block_size: u32 = bytes_to_u32_le!(&buffer, 212);
                        if parsed_block_size > 0 {
                            block_size = parsed_block_size;
                        }
                        file_set_descriptor_location = Some(
                            match UdfLongAllocationDescriptor::read_data(&buffer[248..264]) {
                                Ok(location) => location,
                                Err(mut error) => {
                                    keramics_core::error_trace_add_frame!(
                                        error,
                                        "Unable to read UDF File Set Descriptor location"
                                    );
                                    return Err(error);
                                }
                            },
                        );

                        let map_table_length: usize = bytes_to_u32_le!(&buffer, 264) as usize;
                        let number_of_maps: u32 = bytes_to_u32_le!(&buffer, 268);
                        let mut map_offset: usize = 440;
                        let map_end: usize = min(440 + map_table_length, buffer.len());

                        for _ in 0..number_of_maps {
                            if map_offset + 2 > map_end {
                                break;
                            }
                            let map_type: u8 = buffer[map_offset];
                            let map_length: usize = buffer[map_offset + 1] as usize;
                            if map_length == 0 || map_offset + map_length > map_end {
                                break;
                            }
                            if map_type == 1 && map_length >= 6 {
                                partition_map_numbers
                                    .push(bytes_to_u16_le!(&buffer, map_offset + 4));
                            } else if map_type == 2 && map_length >= 40 {
                                partition_map_numbers
                                    .push(bytes_to_u16_le!(&buffer, map_offset + 38));
                            }
                            map_offset += map_length;
                        }
                    }
                }
                UDF_TAG_TERMINATING_DESCRIPTOR => break,
                _ => {}
            }
        }
        let file_set_descriptor_location: UdfLongAllocationDescriptor =
            match file_set_descriptor_location {
                Some(location) => location,
                None => {
                    return Err(keramics_core::error_trace_new!(
                        "Missing UDF Logical Volume Descriptor"
                    ));
                }
            };
        if raw_partitions.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing UDF Partition Descriptor"
            ));
        }
        let partitions: Vec<UdfPartition> = if partition_map_numbers.is_empty() {
            raw_partitions
        } else {
            let mut mapped: Vec<UdfPartition> = Vec::new();
            for partition_number in partition_map_numbers {
                if let Some(partition) = raw_partitions
                    .iter()
                    .find(|partition| partition.partition_number == partition_number)
                {
                    mapped.push(partition.clone());
                }
            }
            if mapped.is_empty() {
                raw_partitions
            } else {
                mapped
            }
        };
        Ok((
            block_size,
            volume_identifier,
            volume_set_identifier,
            logical_volume_identifier,
            partitions,
            file_set_descriptor_location,
        ))
    }

    /// Resolves a `(partition_reference_number, logical_block_number)` pair to a byte offset.
    pub fn resolve_partition_block_offset(
        &self,
        partition_reference_number: u16,
        logical_block_number: u32,
    ) -> Result<u64, ErrorTrace> {
        let partition: &UdfPartition =
            match self.partitions.get(partition_reference_number as usize) {
                Some(partition) => partition,
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing UDF partition with reference number: {}",
                        partition_reference_number
                    )));
                }
            };
        Ok(
            ((partition.starting_location as u64) + (logical_block_number as u64))
                * (self.block_size as u64),
        )
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    use keramics_core::open_fake_data_stream;

    fn write_udf_tag(
        sector: &mut [u8],
        tag_identifier: u16,
        version: u16,
        crc_length: u16,
        location: u32,
    ) {
        let crc: u16 = if crc_length > 0 {
            UdfDescriptorTag::calculate_crc(&sector[16..16 + crc_length as usize])
        } else {
            0
        };
        sector[0..2].copy_from_slice(&tag_identifier.to_le_bytes());
        sector[2..4].copy_from_slice(&version.to_le_bytes());
        sector[5] = 0;
        sector[6..8].copy_from_slice(&1u16.to_le_bytes());
        sector[8..10].copy_from_slice(&crc.to_le_bytes());
        sector[10..12].copy_from_slice(&crc_length.to_le_bytes());
        sector[12..16].copy_from_slice(&location.to_le_bytes());

        let mut checksum: u8 = 0;
        for byte_index in 0..16 {
            if byte_index != 4 {
                checksum = checksum.wrapping_add(sector[byte_index]);
            }
        }
        sector[4] = checksum;
    }

    fn write_dstring(target: &mut [u8], text: &str) {
        target.fill(0);
        target[0] = 0x08;
        let bytes: &[u8] = text.as_bytes();
        target[1..1 + bytes.len()].copy_from_slice(bytes);
        target[target.len() - 1] = (1 + bytes.len()) as u8;
    }

    fn write_udf_time_stamp(
        target: &mut [u8],
        year: u16,
        month: u8,
        day: u8,
        time_zone_minutes: i16,
    ) {
        let time_zone_bits: u16 = 0x1000 | ((time_zone_minutes as u16) & 0x0fff);
        target[0..2].copy_from_slice(&time_zone_bits.to_le_bytes());
        target[2..4].copy_from_slice(&year.to_le_bytes());
        target[4] = month;
        target[5] = day;
        target[6] = 12;
        target[7] = 30;
        target[8] = 45;
        target[9] = 50;
        target[10] = 0;
        target[11] = 0;
    }

    pub(in crate::cdfs) fn build_test_udf_image() -> Vec<u8> {
        let sector_size: usize = 2048;
        // Sector layout:
        //  16: BEA01, 17: NSR03, 18: TEA01
        //  32..36: Main VDS (PVD=32, PD=33, LVD=34, TD=35)
        //  256: AVDP
        //  260..276: Partition 0 (starting_location=260, length=16):
        //    block 0 (sector 260): FSD -> root ICB at block 1
        //    block 1 (sector 261): Root directory Extended File Entry (inline FIDs)
        //    block 2 (sector 262): Regular file Extended File Entry (short_ad -> block 3)
        //    block 3 (sector 263): Regular file data ("Hello UDF!")
        //    block 4 (sector 264): Symbolic link File Entry (inline symbolic link -> "/target")
        let mut image: Vec<u8> = vec![0; 280 * sector_size];

        // Volume Recognition Sequence at sectors 16..18.
        image[16 * sector_size + 1..16 * sector_size + 6].copy_from_slice(UDF_VRS_SIGNATURE_BEA01);
        image[16 * sector_size + 6] = 1;
        image[17 * sector_size + 1..17 * sector_size + 6].copy_from_slice(UDF_VRS_SIGNATURE_NSR03);
        image[17 * sector_size + 6] = 1;
        image[18 * sector_size + 1..18 * sector_size + 6].copy_from_slice(UDF_VRS_SIGNATURE_TEA01);
        image[18 * sector_size + 6] = 1;

        // Primary Volume Descriptor at sector 32.
        {
            let sector: &mut [u8] = &mut image[32 * sector_size..33 * sector_size];
            write_dstring(&mut sector[24..56], "UDF_VOL");
            write_dstring(&mut sector[212..340], "UDF_SET");
            write_udf_tag(sector, UDF_TAG_PRIMARY_VOLUME_DESCRIPTOR, 3, 496, 32);
        }
        // Partition Descriptor at sector 33 (partition 0, start = 260, length = 16).
        {
            let sector: &mut [u8] = &mut image[33 * sector_size..34 * sector_size];
            sector[22..24].copy_from_slice(&0u16.to_le_bytes());
            sector[188..192].copy_from_slice(&260u32.to_le_bytes());
            sector[192..196].copy_from_slice(&16u32.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_PARTITION_DESCRIPTOR, 3, 496, 33);
        }
        // Logical Volume Descriptor at sector 34 (block_size = 2048, FSD at partition 0 block 0).
        {
            let sector: &mut [u8] = &mut image[34 * sector_size..35 * sector_size];
            write_dstring(&mut sector[84..212], "UDF_LOGVOL");
            sector[212..216].copy_from_slice(&2048u32.to_le_bytes());
            sector[248..252].copy_from_slice(&2048u32.to_le_bytes());
            sector[252..256].copy_from_slice(&0u32.to_le_bytes());
            sector[256..258].copy_from_slice(&0u16.to_le_bytes());
            sector[264..268].copy_from_slice(&6u32.to_le_bytes());
            sector[268..272].copy_from_slice(&1u32.to_le_bytes());
            // Type 1 partition map at offset 440.
            sector[440] = 1;
            sector[441] = 6;
            sector[442..444].copy_from_slice(&1u16.to_le_bytes());
            sector[444..446].copy_from_slice(&0u16.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_LOGICAL_VOLUME_DESCRIPTOR, 3, 430, 34);
        }
        // Terminating Descriptor at sector 35.
        {
            let sector: &mut [u8] = &mut image[35 * sector_size..36 * sector_size];
            write_udf_tag(sector, UDF_TAG_TERMINATING_DESCRIPTOR, 3, 496, 35);
        }
        // Anchor Volume Descriptor Pointer at sector 256.
        {
            let sector: &mut [u8] = &mut image[256 * sector_size..257 * sector_size];
            sector[16..20].copy_from_slice(&(4 * 2048u32).to_le_bytes());
            sector[20..24].copy_from_slice(&32u32.to_le_bytes());
            write_udf_tag(
                sector,
                UDF_TAG_ANCHOR_VOLUME_DESCRIPTOR_POINTER,
                3,
                496,
                256,
            );
        }
        // File Set Descriptor at partition block 0 (sector 260) pointing to root ICB at block 1.
        {
            let sector: &mut [u8] = &mut image[260 * sector_size..261 * sector_size];
            sector[400..404].copy_from_slice(&2048u32.to_le_bytes());
            sector[404..408].copy_from_slice(&1u32.to_le_bytes());
            sector[408..410].copy_from_slice(&0u16.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_FILE_SET_DESCRIPTOR, 3, 496, 0);
        }
        // Root directory Extended File Entry at block 1 (sector 261) with inline descriptors.
        {
            let mut file_identifier_descriptors: Vec<u8> = Vec::new();
            // Parent entry (skipped during directory enumeration).
            let mut parent_descriptor: Vec<u8> = vec![0; 40];
            parent_descriptor[18] =
                UDF_FILE_CHARACTERISTIC_DIRECTORY | UDF_FILE_CHARACTERISTIC_PARENT;
            parent_descriptor[19] = 0;
            parent_descriptor[20..24].copy_from_slice(&2048u32.to_le_bytes());
            parent_descriptor[24..28].copy_from_slice(&1u32.to_le_bytes());
            write_udf_tag(
                &mut parent_descriptor,
                UDF_TAG_FILE_IDENTIFIER_DESCRIPTOR,
                3,
                24,
                1,
            );
            file_identifier_descriptors.extend_from_slice(&parent_descriptor);

            // Regular file "hello.txt" (CS0 8-bit) pointing to block 2.
            let name_bytes: &[u8] = b"\x08hello.txt";
            let descriptor_size: usize = (38 + name_bytes.len() + 3) & !3;
            let mut file_descriptor: Vec<u8> = vec![0; descriptor_size];
            file_descriptor[18] = 0;
            file_descriptor[19] = name_bytes.len() as u8;
            file_descriptor[20..24].copy_from_slice(&2048u32.to_le_bytes());
            file_descriptor[24..28].copy_from_slice(&2u32.to_le_bytes());
            file_descriptor[38..38 + name_bytes.len()].copy_from_slice(name_bytes);
            write_udf_tag(
                &mut file_descriptor,
                UDF_TAG_FILE_IDENTIFIER_DESCRIPTOR,
                3,
                (descriptor_size - 16) as u16,
                1,
            );
            file_identifier_descriptors.extend_from_slice(&file_descriptor);

            // Deleted entry (skipped during directory enumeration).
            let deleted_name: &[u8] = b"\x08gone.txt";
            let deleted_size: usize = (38 + deleted_name.len() + 3) & !3;
            let mut deleted_descriptor: Vec<u8> = vec![0; deleted_size];
            deleted_descriptor[18] = UDF_FILE_CHARACTERISTIC_DELETED;
            deleted_descriptor[19] = deleted_name.len() as u8;
            deleted_descriptor[38..38 + deleted_name.len()].copy_from_slice(deleted_name);
            write_udf_tag(
                &mut deleted_descriptor,
                UDF_TAG_FILE_IDENTIFIER_DESCRIPTOR,
                3,
                (deleted_size - 16) as u16,
                1,
            );
            file_identifier_descriptors.extend_from_slice(&deleted_descriptor);

            // Symbolic link "link" (CS0 16-bit BE) pointing to block 4.
            let symbolic_link_name: &[u8] = &[0x10, 0x00, b'l', 0x00, b'i', 0x00, b'n', 0x00, b'k'];
            let symbolic_link_descriptor_size: usize = (38 + symbolic_link_name.len() + 3) & !3;
            let mut symbolic_link_descriptor: Vec<u8> = vec![0; symbolic_link_descriptor_size];
            symbolic_link_descriptor[18] = 0;
            symbolic_link_descriptor[19] = symbolic_link_name.len() as u8;
            symbolic_link_descriptor[20..24].copy_from_slice(&2048u32.to_le_bytes());
            symbolic_link_descriptor[24..28].copy_from_slice(&4u32.to_le_bytes());
            symbolic_link_descriptor[38..38 + symbolic_link_name.len()]
                .copy_from_slice(symbolic_link_name);
            write_udf_tag(
                &mut symbolic_link_descriptor,
                UDF_TAG_FILE_IDENTIFIER_DESCRIPTOR,
                3,
                (symbolic_link_descriptor_size - 16) as u16,
                1,
            );
            file_identifier_descriptors.extend_from_slice(&symbolic_link_descriptor);

            let sector: &mut [u8] = &mut image[261 * sector_size..262 * sector_size];
            sector[27] = UDF_FILE_TYPE_DIRECTORY;
            // Inline allocation descriptor type.
            sector[34..36].copy_from_slice(&3u16.to_le_bytes());
            sector[36..40].copy_from_slice(&1000u32.to_le_bytes());
            sector[40..44].copy_from_slice(&1000u32.to_le_bytes());
            sector[44..48].copy_from_slice(&0x14a5u32.to_le_bytes());
            sector[48..50].copy_from_slice(&2u16.to_le_bytes());
            sector[56..64]
                .copy_from_slice(&(file_identifier_descriptors.len() as u64).to_le_bytes());
            write_udf_time_stamp(&mut sector[80..92], 2026, 9, 29, 120);
            write_udf_time_stamp(&mut sector[92..104], 2026, 9, 29, 120);
            write_udf_time_stamp(&mut sector[104..116], 2026, 9, 29, -60);
            write_udf_time_stamp(&mut sector[116..128], 2026, 9, 29, 0);
            sector[212..216]
                .copy_from_slice(&(file_identifier_descriptors.len() as u32).to_le_bytes());
            sector[216..216 + file_identifier_descriptors.len()]
                .copy_from_slice(&file_identifier_descriptors);
            write_udf_tag(
                sector,
                UDF_TAG_EXTENDED_FILE_ENTRY,
                3,
                (200 + file_identifier_descriptors.len()) as u16,
                1,
            );
        }
        // Regular file Extended File Entry at block 2 (sector 262) using short allocation descriptors.
        {
            let payload: &[u8] = b"Hello UDF!";
            image[263 * sector_size..263 * sector_size + payload.len()].copy_from_slice(payload);

            let sector: &mut [u8] = &mut image[262 * sector_size..263 * sector_size];
            sector[27] = UDF_FILE_TYPE_REGULAR_FILE;
            // Short allocation descriptor type.
            sector[34..36].copy_from_slice(&0u16.to_le_bytes());
            sector[36..40].copy_from_slice(&500u32.to_le_bytes());
            sector[40..44].copy_from_slice(&501u32.to_le_bytes());
            sector[44..48].copy_from_slice(&0x14a4u32.to_le_bytes());
            sector[48..50].copy_from_slice(&1u16.to_le_bytes());
            sector[56..64].copy_from_slice(&(payload.len() as u64).to_le_bytes());
            write_udf_time_stamp(&mut sector[92..104], 2026, 9, 29, 0);
            sector[212..216].copy_from_slice(&8u32.to_le_bytes());
            sector[216..220].copy_from_slice(&(payload.len() as u32).to_le_bytes());
            sector[220..224].copy_from_slice(&3u32.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_EXTENDED_FILE_ENTRY, 3, 208, 2);
        }
        // Symbolic link File Entry (tag 261) at block 4 (sector 264) with inline path "/target".
        {
            let mut path_components: Vec<u8> = Vec::new();
            // Root path component.
            path_components.extend_from_slice(&[2, 0, 0, 0]);
            path_components
                .extend_from_slice(&[5, 7, 0, 0, 0x08, b't', b'a', b'r', b'g', b'e', b't']);

            let sector: &mut [u8] = &mut image[264 * sector_size..265 * sector_size];
            sector[27] = UDF_FILE_TYPE_SYMBOLIC_LINK;
            // Inline allocation descriptor type.
            sector[34..36].copy_from_slice(&3u16.to_le_bytes());
            sector[44..48].copy_from_slice(&0x16b7u32.to_le_bytes());
            sector[48..50].copy_from_slice(&1u16.to_le_bytes());
            sector[56..64].copy_from_slice(&(path_components.len() as u64).to_le_bytes());
            sector[172..176].copy_from_slice(&(path_components.len() as u32).to_le_bytes());
            sector[176..176 + path_components.len()].copy_from_slice(&path_components);
            write_udf_tag(
                sector,
                UDF_TAG_FILE_ENTRY,
                3,
                (160 + path_components.len()) as u16,
                4,
            );
        }

        image
    }

    #[test]
    fn test_udf_volume_and_entries() -> Result<(), ErrorTrace> {
        let image: Vec<u8> = build_test_udf_image();
        let data_stream: DataStreamReference = open_fake_data_stream(&image);

        let nsr_version: Option<u8> =
            UdfVolume::detect_volume_recognition_sequence(&data_stream, 16 * 2048, 2048)?;
        assert_eq!(nsr_version, Some(3));

        let volume: UdfVolume = UdfVolume::read_data_stream(&data_stream, 3, 2048)?;
        assert_eq!(volume.block_size, 2048);
        assert_eq!(
            volume.volume_identifier.as_ref().unwrap().to_string(),
            "UDF_VOL"
        );
        assert_eq!(
            volume.volume_set_identifier.as_ref().unwrap().to_string(),
            "UDF_SET"
        );
        assert_eq!(
            volume
                .logical_volume_identifier
                .as_ref()
                .unwrap()
                .to_string(),
            "UDF_LOGVOL"
        );

        let root_file_entry: UdfFileEntry = volume.read_file_entry(&volume.root_directory_icb)?;
        assert_eq!(root_file_entry.file_type, UDF_FILE_TYPE_DIRECTORY);
        assert_eq!(root_file_entry.owner_identifier, 1000);
        assert_eq!(
            root_file_entry.creation_time.as_ref().unwrap().utc_offset,
            -4
        );
        // Inline data stream for directory entry works via FakeDataStream.
        let inline_stream: DataStreamReference =
            volume.get_file_entry_data_stream(&root_file_entry);
        let inline_size: u64 = keramics_core::data_stream_get_size!(&inline_stream);
        assert_eq!(inline_size, root_file_entry.size);

        // Non-symbolic-link returns None from read_symbolic_link_target.
        assert_eq!(volume.read_symbolic_link_target(&root_file_entry)?, None);

        let entries: Vec<UdfDirectoryEntry> = volume.read_directory_entries(&root_file_entry)?;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name.to_string(), "hello.txt");
        assert_eq!(entries[1].name.to_string(), "link");

        // Read regular file via data stream.
        let file_entry: UdfFileEntry = volume.read_file_entry(&entries[0].icb)?;
        assert_eq!(file_entry.size, 10);
        assert_eq!(file_entry.owner_identifier, 500);
        assert_eq!(file_entry.group_identifier, 501);
        let file_stream: DataStreamReference = volume.get_file_entry_data_stream(&file_entry);
        let mut content: Vec<u8> = vec![0; 10];
        keramics_core::data_stream_read_exact_at_position!(
            &file_stream,
            &mut content,
            SeekFrom::Start(0)
        );
        assert_eq!(content, b"Hello UDF!");

        // Read symbolic link target.
        let symbolic_link_file_entry: UdfFileEntry = volume.read_file_entry(&entries[1].icb)?;
        let target: Option<ByteString> =
            volume.read_symbolic_link_target(&symbolic_link_file_entry)?;
        assert_eq!(target.unwrap().to_string(), "/target");

        Ok(())
    }

    #[test]
    fn test_udf_block_reader_and_allocation_descriptors() -> Result<(), ErrorTrace> {
        // Test UdfBlockReader with recorded, sparse, and trailing zero-filled extents.
        let backing: Vec<u8> = b"ABCDEF".to_vec();
        let data_stream: DataStreamReference = open_fake_data_stream(&backing);
        let extents: Vec<UdfDataExtent> = vec![
            UdfDataExtent {
                data_offset: Some(0),
                size: 3,
            },
            UdfDataExtent {
                data_offset: None,
                size: 2,
            },
            UdfDataExtent {
                data_offset: Some(3),
                size: 3,
            },
        ];
        let mut reader: UdfBlockReader = UdfBlockReader::new(&data_stream, extents, 10);
        assert_eq!(reader.get_size(), 10);

        let mut empty: [u8; 0] = [];
        assert_eq!(reader.read_data_from_blocks(&mut empty, 0)?, 0);
        let mut past_end: [u8; 4] = [0; 4];
        assert_eq!(reader.read_data_from_blocks(&mut past_end, 10)?, 0);

        let mut output: [u8; 10] = [0xff; 10];
        let read_count: usize = reader.read_data_from_blocks(&mut output, 0)?;
        assert_eq!(read_count, 10);
        assert_eq!(&output, b"ABC\x00\x00DEF\x00\x00");

        // Test long allocation descriptors (recorded + sparse), short sparse extent, and relative symbolic link.
        let mut image: Vec<u8> = build_test_udf_image();
        let sector_size: usize = 2048;

        // Block 5 (sector 265): File Entry using long allocation descriptors (type 1):
        //  extent 0 = recorded block 3 (5 bytes "Hello"), extent 1 = sparse (3 bytes zeros).
        {
            let sector: &mut [u8] = &mut image[265 * sector_size..266 * sector_size];
            sector[27] = UDF_FILE_TYPE_REGULAR_FILE;
            sector[34..36].copy_from_slice(&1u16.to_le_bytes());
            sector[56..64].copy_from_slice(&8u64.to_le_bytes());
            sector[172..176].copy_from_slice(&32u32.to_le_bytes());
            // Long allocation descriptor 0: recorded, length 5, block 3, partition 0.
            sector[176..180].copy_from_slice(&5u32.to_le_bytes());
            sector[180..184].copy_from_slice(&3u32.to_le_bytes());
            sector[184..186].copy_from_slice(&0u16.to_le_bytes());
            // Long allocation descriptor 1: unrecorded/allocated (extent_type = 1), length 3.
            sector[192..196].copy_from_slice(&(0x4000_0003u32).to_le_bytes());
            write_udf_tag(sector, UDF_TAG_FILE_ENTRY, 3, 192, 5);
        }
        // Block 6 (sector 266): File Entry using short allocation descriptor with sparse extent.
        {
            let sector: &mut [u8] = &mut image[266 * sector_size..267 * sector_size];
            sector[27] = UDF_FILE_TYPE_REGULAR_FILE;
            sector[34..36].copy_from_slice(&0u16.to_le_bytes());
            sector[56..64].copy_from_slice(&4u64.to_le_bytes());
            sector[172..176].copy_from_slice(&8u32.to_le_bytes());
            sector[176..180].copy_from_slice(&(0x4000_0004u32).to_le_bytes());
            write_udf_tag(sector, UDF_TAG_FILE_ENTRY, 3, 168, 6);
        }
        // Block 7 (sector 267): Symbolic link with parent "..", current ".", unknown type 9, and root-only variant.
        {
            let mut components: Vec<u8> = Vec::new();
            components.extend_from_slice(&[3, 0, 0, 0]);
            components.extend_from_slice(&[4, 0, 0, 0]);
            components.extend_from_slice(&[9, 0, 0, 0]);
            components.extend_from_slice(&[5, 2, 0, 0, 0x08, b'x']);
            let sector: &mut [u8] = &mut image[267 * sector_size..268 * sector_size];
            sector[27] = UDF_FILE_TYPE_SYMBOLIC_LINK;
            sector[34..36].copy_from_slice(&3u16.to_le_bytes());
            sector[56..64].copy_from_slice(&(components.len() as u64).to_le_bytes());
            sector[172..176].copy_from_slice(&(components.len() as u32).to_le_bytes());
            sector[176..176 + components.len()].copy_from_slice(&components);
            write_udf_tag(
                sector,
                UDF_TAG_FILE_ENTRY,
                3,
                (160 + components.len()) as u16,
                7,
            );
        }
        // Block 8 (sector 268): Root-only symbolic link "/".
        {
            let components: [u8; 4] = [1, 0, 0, 0];
            let sector: &mut [u8] = &mut image[268 * sector_size..269 * sector_size];
            sector[27] = UDF_FILE_TYPE_SYMBOLIC_LINK;
            sector[34..36].copy_from_slice(&3u16.to_le_bytes());
            sector[56..64].copy_from_slice(&4u64.to_le_bytes());
            sector[172..176].copy_from_slice(&4u32.to_le_bytes());
            sector[176..180].copy_from_slice(&components);
            write_udf_tag(sector, UDF_TAG_FILE_ENTRY, 3, 164, 8);
        }

        let stream: DataStreamReference = open_fake_data_stream(&image);
        let volume: UdfVolume = UdfVolume::read_data_stream(&stream, 3, 2048)?;

        let long_ad_entry: UdfFileEntry = volume.read_file_entry(&UdfLongAllocationDescriptor {
            extent_length: 2048,
            logical_block_number: 5,
            partition_reference_number: 0,
        })?;
        assert_eq!(
            volume.read_file_entry_bytes(&long_ad_entry)?,
            b"Hello\x00\x00\x00"
        );

        let sparse_short_entry: UdfFileEntry =
            volume.read_file_entry(&UdfLongAllocationDescriptor {
                extent_length: 2048,
                logical_block_number: 6,
                partition_reference_number: 0,
            })?;
        assert_eq!(volume.read_file_entry_bytes(&sparse_short_entry)?, [0; 4]);

        let relative_link_entry: UdfFileEntry =
            volume.read_file_entry(&UdfLongAllocationDescriptor {
                extent_length: 2048,
                logical_block_number: 7,
                partition_reference_number: 0,
            })?;
        assert_eq!(
            volume
                .read_symbolic_link_target(&relative_link_entry)?
                .unwrap()
                .to_string(),
            ".././x"
        );

        let root_link_entry: UdfFileEntry =
            volume.read_file_entry(&UdfLongAllocationDescriptor {
                extent_length: 2048,
                logical_block_number: 8,
                partition_reference_number: 0,
            })?;
        assert_eq!(
            volume
                .read_symbolic_link_target(&root_link_entry)?
                .unwrap()
                .to_string(),
            "/"
        );

        Ok(())
    }

    #[test]
    fn test_udf_fallback_avdp_reserve_vds_and_type2_partition_map() -> Result<(), ErrorTrace> {
        let sector_size: usize = 2048;
        let mut image: Vec<u8> = build_test_udf_image();

        // Copy Main VDS (sectors 32..36) to Reserve VDS (sectors 40..44) with a Type 2 partition map.
        for index in 0..4 {
            let source_start: usize = (32 + index) * sector_size;
            let target_start: usize = (40 + index) * sector_size;
            let sector_copy: Vec<u8> = image[source_start..source_start + sector_size].to_vec();
            image[target_start..target_start + sector_size].copy_from_slice(&sector_copy);
        }
        // Rewrite LVD at sector 42 to use a Type 2 partition map (length 64, partition number at +38).
        {
            let sector: &mut [u8] = &mut image[42 * sector_size..43 * sector_size];
            sector[264..268].copy_from_slice(&64u32.to_le_bytes());
            sector[268..272].copy_from_slice(&1u32.to_le_bytes());
            sector[440..504].fill(0);
            sector[440] = 2;
            sector[441] = 64;
            sector[440 + 38..440 + 40].copy_from_slice(&0u16.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_LOGICAL_VOLUME_DESCRIPTOR, 3, 488, 42);
        }
        // Corrupt Main VDS (sector 32) and primary AVDP (sector 256), and place backup AVDP at last sector (279).
        image[32 * sector_size..36 * sector_size].fill(0);
        image[256 * sector_size..257 * sector_size].fill(0);
        {
            let sector: &mut [u8] = &mut image[279 * sector_size..280 * sector_size];
            // Main VDS points to corrupted sectors 32..36, Reserve VDS points to valid sectors 40..44.
            sector[16..20].copy_from_slice(&(4 * 2048u32).to_le_bytes());
            sector[20..24].copy_from_slice(&32u32.to_le_bytes());
            sector[24..28].copy_from_slice(&(4 * 2048u32).to_le_bytes());
            sector[28..32].copy_from_slice(&40u32.to_le_bytes());
            write_udf_tag(
                sector,
                UDF_TAG_ANCHOR_VOLUME_DESCRIPTOR_POINTER,
                3,
                496,
                279,
            );
        }

        // Also test NSR02 VRS detection and skipping ISO 9660 CD001 descriptors.
        image[16 * sector_size + 1..16 * sector_size + 6]
            .copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SIGNATURE);
        image[17 * sector_size + 1..17 * sector_size + 6].copy_from_slice(UDF_VRS_SIGNATURE_BEA01);
        image[18 * sector_size + 1..18 * sector_size + 6].copy_from_slice(UDF_VRS_SIGNATURE_NSR02);
        image[19 * sector_size + 1..19 * sector_size + 6].copy_from_slice(UDF_VRS_SIGNATURE_TEA01);

        let data_stream: DataStreamReference = open_fake_data_stream(&image);
        let nsr_version: Option<u8> =
            UdfVolume::detect_volume_recognition_sequence(&data_stream, 16 * 2048, 2048)?;
        assert_eq!(nsr_version, Some(2));

        let volume: UdfVolume = UdfVolume::read_data_stream(&data_stream, 2, 2048)?;
        assert_eq!(volume.nsr_version, 2);
        assert_eq!(volume.block_size, 2048);

        Ok(())
    }

    #[test]
    fn test_udf_permissions_and_time_stamps() {
        assert_eq!(
            UdfVolume::convert_permissions_to_posix_mode(UDF_FILE_TYPE_BLOCK_DEVICE, 0x1fff),
            0o060777
        );
        assert_eq!(
            UdfVolume::convert_permissions_to_posix_mode(UDF_FILE_TYPE_CHARACTER_DEVICE, 0x0421),
            0o020111
        );
        assert_eq!(
            UdfVolume::convert_permissions_to_posix_mode(UDF_FILE_TYPE_FIFO, 0x0842),
            0o010222
        );
        assert_eq!(
            UdfVolume::convert_permissions_to_posix_mode(UDF_FILE_TYPE_SOCKET, 0x1084),
            0o140444
        );
        assert_eq!(UdfVolume::convert_permissions_to_posix_mode(99, 0), 0);

        // Test unspecified local timezone (-2047) and UTC (type 0).
        let mut local_unspecified: [u8; 12] = [0; 12];
        write_udf_time_stamp(&mut local_unspecified, 2026, 9, 29, -2047);
        assert_eq!(
            UdfVolume::read_time_stamp(&local_unspecified)
                .unwrap()
                .utc_offset,
            0
        );

        let mut utc_time_stamp: [u8; 12] = [0; 12];
        write_udf_time_stamp(&mut utc_time_stamp, 2026, 9, 29, 60);
        utc_time_stamp[1] &= 0x0f;
        assert_eq!(
            UdfVolume::read_time_stamp(&utc_time_stamp)
                .unwrap()
                .utc_offset,
            0
        );
    }

    #[test]
    fn test_udf_helpers_and_errors() {
        // Test descriptor tag errors: short slice, bad checksum, CRC out of bounds, CRC mismatch.
        let result: Result<UdfDescriptorTag, ErrorTrace> = UdfDescriptorTag::read_data(&[0; 8]);
        assert!(result.is_err());

        let invalid_tag: [u8; 16] = [1, 0, 3, 0, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let result: Result<UdfDescriptorTag, ErrorTrace> =
            UdfDescriptorTag::read_data(&invalid_tag);
        assert!(result.is_err());

        let mut out_of_bounds_crc: [u8; 20] = [0; 20];
        out_of_bounds_crc[10..12].copy_from_slice(&100u16.to_le_bytes());
        let mut checksum: u8 = 0;
        for byte_index in 0..16 {
            if byte_index != 4 {
                checksum = checksum.wrapping_add(out_of_bounds_crc[byte_index]);
            }
        }
        out_of_bounds_crc[4] = checksum;
        let result: Result<UdfDescriptorTag, ErrorTrace> =
            UdfDescriptorTag::read_data(&out_of_bounds_crc);
        assert!(result.is_err());

        let mut invalid_crc: [u8; 24] = [0; 24];
        write_udf_tag(&mut invalid_crc, 1, 3, 8, 0);
        invalid_crc[16] ^= 0xff;
        let result: Result<UdfDescriptorTag, ErrorTrace> =
            UdfDescriptorTag::read_data(&invalid_crc);
        assert!(result.is_err());

        // Test short extent address and long allocation descriptor buffers.
        let result: Result<UdfExtentAddress, ErrorTrace> = UdfExtentAddress::read_data(&[0; 4]);
        assert!(result.is_err());

        let result: Result<UdfLongAllocationDescriptor, ErrorTrace> =
            UdfLongAllocationDescriptor::read_data(&[0; 8]);
        assert!(result.is_err());

        // Test unsupported CS0 compression identifier.
        let result: Result<ByteString, ErrorTrace> = UdfVolume::decode_cs0(&[0x99, 0x41]);
        assert!(result.is_err());

        // Test empty CS0 and dstring edge cases.
        assert!(UdfVolume::decode_cs0(&[]).unwrap().is_empty());
        assert_eq!(UdfVolume::decode_dstring(&[]), None);
        assert_eq!(UdfVolume::decode_dstring(&[0, 0]), None);
        assert_eq!(UdfVolume::decode_dstring(&[0x08, 0x41, 10]), None);

        // Test zero and short timestamps returning None.
        assert_eq!(UdfVolume::read_time_stamp(&[0; 12]), None);
        assert_eq!(UdfVolume::read_time_stamp(&[0; 4]), None);

        // Test VRS detection without BEA01.
        let empty_stream: DataStreamReference = open_fake_data_stream(&vec![0; 20 * 2048]);
        let nsr_result: Option<u8> =
            UdfVolume::detect_volume_recognition_sequence(&empty_stream, 16 * 2048, 2048).unwrap();
        assert_eq!(nsr_result, None);

        // Test missing AVDP.
        let result: Result<UdfVolume, ErrorTrace> =
            UdfVolume::read_data_stream(&empty_stream, 3, 2048);
        assert!(result.is_err());

        // Test read_volume_descriptor_sequence with zero length.
        let result = UdfVolume::read_volume_descriptor_sequence(
            &empty_stream,
            2048,
            UdfExtentAddress::default(),
        );
        assert!(result.is_err());

        // Test invalid FSD tag, invalid file entry tag, unsupported allocation descriptor type,
        // allocation descriptors out of bounds, missing partition reference, and invalid FID tag.
        let sector_size: usize = 2048;
        let mut bad_fsd_image: Vec<u8> = build_test_udf_image();
        write_udf_tag(
            &mut bad_fsd_image[260 * sector_size..261 * sector_size],
            UDF_TAG_TERMINATING_DESCRIPTOR,
            3,
            0,
            0,
        );
        let bad_fsd_stream: DataStreamReference = open_fake_data_stream(&bad_fsd_image);
        let result: Result<UdfVolume, ErrorTrace> =
            UdfVolume::read_data_stream(&bad_fsd_stream, 3, 2048);
        assert!(result.is_err());

        let mut bad_entries_image: Vec<u8> = build_test_udf_image();
        // Block 5: wrong file entry tag identifier (tag 8).
        write_udf_tag(
            &mut bad_entries_image[265 * sector_size..266 * sector_size],
            UDF_TAG_TERMINATING_DESCRIPTOR,
            3,
            0,
            5,
        );
        // Block 6: unsupported allocation descriptor type (2).
        {
            let sector: &mut [u8] = &mut bad_entries_image[266 * sector_size..267 * sector_size];
            sector[27] = UDF_FILE_TYPE_REGULAR_FILE;
            sector[34..36].copy_from_slice(&2u16.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_FILE_ENTRY, 3, 160, 6);
        }
        // Block 7: allocation descriptors length out of bounds.
        {
            let sector: &mut [u8] = &mut bad_entries_image[267 * sector_size..268 * sector_size];
            sector[27] = UDF_FILE_TYPE_REGULAR_FILE;
            sector[172..176].copy_from_slice(&4096u32.to_le_bytes());
            write_udf_tag(sector, UDF_TAG_FILE_ENTRY, 3, 160, 7);
        }
        // Block 8: directory containing an invalid FID tag identifier.
        {
            let mut bad_descriptor: Vec<u8> = vec![0; 40];
            write_udf_tag(
                &mut bad_descriptor,
                UDF_TAG_TERMINATING_DESCRIPTOR,
                3,
                24,
                8,
            );
            let sector: &mut [u8] = &mut bad_entries_image[268 * sector_size..269 * sector_size];
            sector[27] = UDF_FILE_TYPE_DIRECTORY;
            sector[34..36].copy_from_slice(&3u16.to_le_bytes());
            sector[56..64].copy_from_slice(&40u64.to_le_bytes());
            sector[172..176].copy_from_slice(&40u32.to_le_bytes());
            sector[176..216].copy_from_slice(&bad_descriptor);
            write_udf_tag(sector, UDF_TAG_FILE_ENTRY, 3, 200, 8);
        }
        let bad_entries_stream: DataStreamReference = open_fake_data_stream(&bad_entries_image);
        let volume: UdfVolume = UdfVolume::read_data_stream(&bad_entries_stream, 3, 2048).unwrap();

        let result: Result<u64, ErrorTrace> = volume.resolve_partition_block_offset(99, 0);
        assert!(result.is_err());

        for block_number in [5, 6, 7] {
            let result: Result<UdfFileEntry, ErrorTrace> =
                volume.read_file_entry(&UdfLongAllocationDescriptor {
                    extent_length: 2048,
                    logical_block_number: block_number,
                    partition_reference_number: 0,
                });
            assert!(result.is_err());
        }
        let bad_dir_entry: UdfFileEntry = volume
            .read_file_entry(&UdfLongAllocationDescriptor {
                extent_length: 2048,
                logical_block_number: 8,
                partition_reference_number: 0,
            })
            .unwrap();
        let result: Result<Vec<UdfDirectoryEntry>, ErrorTrace> =
            volume.read_directory_entries(&bad_dir_entry);
        assert!(result.is_err());
    }
}
