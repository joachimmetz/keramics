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

use std::io::SeekFrom;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_datetime::CdFsDateTime;
use keramics_encodings::CharacterEncoding;
use keramics_types::{ByteString, bytes_to_u32_be, bytes_to_u32_le};

use super::constants::*;

/// Maximum number of SUSP continuation area hops followed for a single record.
const CDFS_SUSP_MAXIMUM_CONTINUATION_DEPTH: usize = 16;

/// Rock Ridge (SUSP / RRIP) attributes parsed from a directory record System Use area.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct CdFsRockRidgeAttributes {
    /// Value indicating that a SUSP `"SP"` protocol indicator was encountered.
    pub susp_detected: bool,

    /// Number of bytes to skip before SUSP entries in directory records (`LEN_SKP`).
    pub susp_skip_length: u8,

    /// Extension identifiers declared via `"ER"` entries.
    pub extensions: Vec<String>,

    /// Alternate POSIX file name (`"NM"`).
    pub alternate_name: Option<ByteString>,

    /// Symbolic link target (`"SL"`).
    pub symbolic_link_target: Option<ByteString>,

    /// POSIX file mode (`"PX"` `st_mode`).
    pub file_mode: Option<u32>,

    /// POSIX link count (`"PX"` `st_nlink`).
    pub number_of_links: Option<u32>,

    /// POSIX user identifier (`"PX"` `st_uid`).
    pub owner_identifier: Option<u32>,

    /// POSIX group identifier (`"PX"` `st_gid`).
    pub group_identifier: Option<u32>,

    /// POSIX file serial number (`"PX"` `st_ino`, RRIP 1.12).
    pub inode_number: Option<u32>,

    /// POSIX device major and minor numbers.
    pub device_number: Option<(u32, u32)>,

    /// Child link target logical block (`"CL"`).
    pub child_link_sector: Option<u32>,

    /// Parent link target logical block (`"PL"`).
    pub parent_link_sector: Option<u32>,

    /// Value indicating this entry is a relocated directory placeholder (`"RE"`).
    pub is_relocated_directory: bool,

    /// Creation time (`"TF"`).
    pub creation_time: Option<CdFsDateTime>,

    /// Modification time (`"TF"`).
    pub modification_time: Option<CdFsDateTime>,

    /// Access time (`"TF"`).
    pub access_time: Option<CdFsDateTime>,

    /// Attribute change time (`"TF"`).
    pub change_time: Option<CdFsDateTime>,

    /// Backup time (`"TF"`).
    pub backup_time: Option<CdFsDateTime>,

    /// Expiration time (`"TF"`).
    pub expiration_time: Option<CdFsDateTime>,

    /// Effective time (`"TF"`).
    pub effective_time: Option<CdFsDateTime>,
}

impl CdFsRockRidgeAttributes {
    /// Creates new empty Rock Ridge attributes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determines if any Rock Ridge extension or RRIP entry was detected.
    pub fn has_rock_ridge(&self) -> bool {
        self.extensions.iter().any(|extension| {
            extension.starts_with("RRIP_")
                || extension.starts_with("IEEE_P1282")
                || extension.starts_with("IEEE_1282")
        }) || self.file_mode.is_some()
            || self.alternate_name.is_some()
            || self.symbolic_link_target.is_some()
            || self.child_link_sector.is_some()
            || self.parent_link_sector.is_some()
            || self.is_relocated_directory
    }

    /// Reads a both-byte (little-endian followed by big-endian) 32-bit integer.
    fn read_both_u32(data: &[u8], offset: usize) -> Result<u32, ErrorTrace> {
        if offset + 8 > data.len() {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        let little_endian: u32 = bytes_to_u32_le!(data, offset);
        let big_endian: u32 = bytes_to_u32_be!(data, offset + 4);

        if little_endian != big_endian {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian 32-bit value"
            ));
        }
        Ok(little_endian)
    }

    /// Reads SUSP and RRIP entries from a directory record System Use area, following `"CE"`
    /// continuation areas as needed.
    pub fn read_data(
        &mut self,
        data_stream: &DataStreamReference,
        bytes_per_sector: u16,
        system_use_data: &[u8],
        skip_length: usize,
    ) -> Result<(), ErrorTrace> {
        if skip_length >= system_use_data.len() {
            return Ok(());
        }
        let mut alternate_name_bytes: Vec<u8> = Vec::new();
        let mut has_alternate_name: bool = false;
        let mut symbolic_link_parts: Vec<Vec<u8>> = Vec::new();
        let mut symbolic_link_continue_component: bool = false;
        let mut has_symbolic_link: bool = false;

        let mut current_buffer: Vec<u8> = system_use_data[skip_length..].to_vec();
        let mut continuation_depth: usize = 0;

        loop {
            let mut next_continuation: Option<(u32, u32, u32)> = None;
            let mut data_offset: usize = 0;
            let data_size: usize = current_buffer.len();

            while data_offset + 4 <= data_size {
                let signature: &[u8] = &current_buffer[data_offset..data_offset + 2];
                let entry_size: usize = current_buffer[data_offset + 2] as usize;

                if entry_size < 4 || data_offset + entry_size > data_size {
                    break;
                }
                let entry_data: &[u8] = &current_buffer[data_offset..data_offset + entry_size];

                if signature == CDFS_SUSP_SIGNATURE_TERMINATOR {
                    break;
                } else if signature == CDFS_SUSP_SIGNATURE_PADDING
                    || signature == CDFS_SUSP_SIGNATURE_EXTENSION_SELECTOR
                    || signature == CDFS_RRIP_SIGNATURE_ROCK_RIDGE_INDICATOR
                {
                    // Padding, extension selector, and deprecated RR indicator require no state.
                } else if signature == CDFS_SUSP_SIGNATURE_PROTOCOL_INDICATOR {
                    if entry_size >= 7 && entry_data[4] == 0xbe && entry_data[5] == 0xef {
                        self.susp_detected = true;
                        self.susp_skip_length = entry_data[6];
                    }
                } else if signature == CDFS_SUSP_SIGNATURE_EXTENSIONS_REFERENCE {
                    if entry_size >= 8 {
                        let identifier_length: usize = entry_data[4] as usize;
                        if 8 + identifier_length <= entry_size {
                            if let Ok(identifier) =
                                std::str::from_utf8(&entry_data[8..8 + identifier_length])
                            {
                                self.extensions.push(identifier.to_string());
                            }
                        }
                    }
                } else if signature == CDFS_SUSP_SIGNATURE_CONTINUATION {
                    if entry_size >= 28 {
                        let block_location: u32 = match Self::read_both_u32(entry_data, 4) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read SUSP continuation block location"
                                );
                                return Err(error);
                            }
                        };
                        let block_offset: u32 = match Self::read_both_u32(entry_data, 12) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read SUSP continuation block offset"
                                );
                                return Err(error);
                            }
                        };
                        let continuation_length: u32 = match Self::read_both_u32(entry_data, 20) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read SUSP continuation length"
                                );
                                return Err(error);
                            }
                        };
                        if continuation_length > 0 {
                            next_continuation =
                                Some((block_location, block_offset, continuation_length));
                        }
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_POSIX_ATTRIBUTES {
                    if entry_size >= 36 {
                        self.file_mode = Some(match Self::read_both_u32(entry_data, 4) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge POSIX file mode"
                                );
                                return Err(error);
                            }
                        });
                        self.number_of_links = Some(match Self::read_both_u32(entry_data, 12) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge POSIX link count"
                                );
                                return Err(error);
                            }
                        });
                        self.owner_identifier = Some(match Self::read_both_u32(entry_data, 20) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge POSIX owner identifier"
                                );
                                return Err(error);
                            }
                        });
                        self.group_identifier = Some(match Self::read_both_u32(entry_data, 28) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge POSIX group identifier"
                                );
                                return Err(error);
                            }
                        });
                        if entry_size >= 44 {
                            self.inode_number = Some(match Self::read_both_u32(entry_data, 36) {
                                Ok(value) => value,
                                Err(mut error) => {
                                    keramics_core::error_trace_add_frame!(
                                        error,
                                        "Unable to read Rock Ridge POSIX inode number"
                                    );
                                    return Err(error);
                                }
                            });
                        }
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_DEVICE_NUMBER {
                    if entry_size >= 20 {
                        let major: u32 = match Self::read_both_u32(entry_data, 4) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge device major number"
                                );
                                return Err(error);
                            }
                        };
                        let minor: u32 = match Self::read_both_u32(entry_data, 12) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge device minor number"
                                );
                                return Err(error);
                            }
                        };
                        self.device_number = Some((major, minor));
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_ALTERNATE_NAME {
                    if entry_size >= 5 {
                        let flags: u8 = entry_data[4];
                        if flags & 0x02 != 0 {
                            alternate_name_bytes.extend_from_slice(b".");
                            has_alternate_name = true;
                        } else if flags & 0x04 != 0 {
                            alternate_name_bytes.extend_from_slice(b"..");
                            has_alternate_name = true;
                        } else {
                            alternate_name_bytes.extend_from_slice(&entry_data[5..entry_size]);
                            has_alternate_name = true;
                        }
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_SYMBOLIC_LINK {
                    if entry_size >= 5 {
                        let symbolic_link_flags: u8 = entry_data[4];
                        let mut component_offset: usize = 5;

                        while component_offset + 2 <= entry_size {
                            let component_flags: u8 = entry_data[component_offset];
                            let component_length: usize = entry_data[component_offset + 1] as usize;
                            let component_end: usize = component_offset + 2 + component_length;

                            if component_end > entry_size {
                                break;
                            }
                            let component_bytes: &[u8] = if component_flags & 0x02 != 0 {
                                b"."
                            } else if component_flags & 0x04 != 0 {
                                b".."
                            } else if component_flags & 0x08 != 0 {
                                b""
                            } else {
                                &entry_data[component_offset + 2..component_end]
                            };

                            if component_flags & 0x08 != 0 {
                                symbolic_link_parts.push(Vec::new());
                            } else if symbolic_link_continue_component {
                                if let Some(last_part) = symbolic_link_parts.last_mut() {
                                    last_part.extend_from_slice(component_bytes);
                                } else {
                                    symbolic_link_parts.push(component_bytes.to_vec());
                                }
                            } else {
                                symbolic_link_parts.push(component_bytes.to_vec());
                            }
                            symbolic_link_continue_component = (component_flags & 0x01) != 0;
                            has_symbolic_link = true;
                            component_offset = component_end;
                        }
                        if (symbolic_link_flags & 0x01) == 0 {
                            symbolic_link_continue_component = false;
                        }
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_CHILD_LINK {
                    if entry_size >= 12 {
                        self.child_link_sector = Some(match Self::read_both_u32(entry_data, 4) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge child link sector"
                                );
                                return Err(error);
                            }
                        });
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_PARENT_LINK {
                    if entry_size >= 12 {
                        self.parent_link_sector = Some(match Self::read_both_u32(entry_data, 4) {
                            Ok(value) => value,
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge parent link sector"
                                );
                                return Err(error);
                            }
                        });
                    }
                } else if signature == CDFS_RRIP_SIGNATURE_RELOCATED_DIRECTORY {
                    self.is_relocated_directory = true;
                } else if signature == CDFS_RRIP_SIGNATURE_TIME_STAMPS {
                    if entry_size >= 5 {
                        match self.read_time_stamps(entry_data) {
                            Ok(_) => {}
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Rock Ridge timestamps"
                                );
                                return Err(error);
                            }
                        }
                    }
                }
                data_offset += entry_size;
            }

            match next_continuation {
                Some((block_location, block_offset, continuation_length)) => {
                    continuation_depth += 1;
                    if continuation_depth > CDFS_SUSP_MAXIMUM_CONTINUATION_DEPTH {
                        return Err(keramics_core::error_trace_new!(
                            "Maximum SUSP continuation depth exceeded"
                        ));
                    }
                    let continuation_offset: u64 =
                        (block_location as u64) * (bytes_per_sector as u64) + (block_offset as u64);
                    let mut continuation_data: Vec<u8> = vec![0; continuation_length as usize];

                    keramics_core::data_stream_read_exact_at_position!(
                        data_stream,
                        &mut continuation_data,
                        SeekFrom::Start(continuation_offset)
                    );
                    current_buffer = continuation_data;
                }
                None => break,
            }
        }

        if has_alternate_name {
            let mut byte_string: ByteString =
                ByteString::new_with_encoding(&CharacterEncoding::Utf8);
            byte_string.elements = alternate_name_bytes;
            self.alternate_name = Some(byte_string);
        }
        if has_symbolic_link {
            let mut target_bytes: Vec<u8> = Vec::new();
            for (part_index, part) in symbolic_link_parts.iter().enumerate() {
                if part_index > 0 {
                    target_bytes.push(b'/');
                }
                target_bytes.extend_from_slice(part);
            }
            if symbolic_link_parts.len() == 1 && symbolic_link_parts[0].is_empty() {
                target_bytes.push(b'/');
            }
            let mut byte_string: ByteString =
                ByteString::new_with_encoding(&CharacterEncoding::Utf8);
            byte_string.elements = target_bytes;
            self.symbolic_link_target = Some(byte_string);
        }
        Ok(())
    }

    /// Reads timestamps from a Rock Ridge `"TF"` entry.
    fn read_time_stamps(&mut self, entry_data: &[u8]) -> Result<(), ErrorTrace> {
        let flags: u8 = entry_data[4];
        let is_long_form: bool = (flags & CDFS_RRIP_TIME_STAMP_FLAG_LONG_FORM) != 0;
        let timestamp_size: usize = if is_long_form { 17 } else { 7 };
        let mut offset: usize = 5;

        let flag_bits: [u8; 7] = [
            CDFS_RRIP_TIME_STAMP_FLAG_CREATION,
            CDFS_RRIP_TIME_STAMP_FLAG_MODIFY,
            CDFS_RRIP_TIME_STAMP_FLAG_ACCESS,
            CDFS_RRIP_TIME_STAMP_FLAG_ATTRIBUTES,
            CDFS_RRIP_TIME_STAMP_FLAG_BACKUP,
            CDFS_RRIP_TIME_STAMP_FLAG_EXPIRATION,
            CDFS_RRIP_TIME_STAMP_FLAG_EFFECTIVE,
        ];

        for flag_bit in flag_bits {
            if (flags & flag_bit) == 0 {
                continue;
            }
            if offset + timestamp_size > entry_data.len() {
                return Err(keramics_core::error_trace_new!(
                    "Unsupported Rock Ridge timestamp entry size"
                ));
            }
            let date_time: CdFsDateTime = if is_long_form {
                match CdFsDateTime::from_string(&entry_data[offset..offset + 17]) {
                    Ok(value) => value,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to read Rock Ridge long-form timestamp"
                        );
                        return Err(error);
                    }
                }
            } else {
                CdFsDateTime::from_bytes(&entry_data[offset..offset + 7])
            };
            offset += timestamp_size;

            match flag_bit {
                CDFS_RRIP_TIME_STAMP_FLAG_CREATION => self.creation_time = Some(date_time),
                CDFS_RRIP_TIME_STAMP_FLAG_MODIFY => self.modification_time = Some(date_time),
                CDFS_RRIP_TIME_STAMP_FLAG_ACCESS => self.access_time = Some(date_time),
                CDFS_RRIP_TIME_STAMP_FLAG_ATTRIBUTES => self.change_time = Some(date_time),
                CDFS_RRIP_TIME_STAMP_FLAG_BACKUP => self.backup_time = Some(date_time),
                CDFS_RRIP_TIME_STAMP_FLAG_EXPIRATION => self.expiration_time = Some(date_time),
                CDFS_RRIP_TIME_STAMP_FLAG_EFFECTIVE => self.effective_time = Some(date_time),
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use keramics_core::open_fake_data_stream;

    fn extend_both_u32(data: &mut Vec<u8>, value: u32) {
        data.extend_from_slice(&value.to_le_bytes());
        data.extend_from_slice(&value.to_be_bytes());
    }

    #[test]
    fn test_has_rock_ridge() {
        let attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        assert!(!attributes.has_rock_ridge());

        for extension in ["RRIP_1991A", "IEEE_P1282", "IEEE_1282"] {
            let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
            attributes.extensions.push(String::from(extension));
            assert!(attributes.has_rock_ridge());
        }
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.file_mode = Some(0o100644);
        assert!(attributes.has_rock_ridge());

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.alternate_name = Some(ByteString::from("name"));
        assert!(attributes.has_rock_ridge());

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.symbolic_link_target = Some(ByteString::from("target"));
        assert!(attributes.has_rock_ridge());

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.child_link_sector = Some(10);
        assert!(attributes.has_rock_ridge());

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.parent_link_sector = Some(10);
        assert!(attributes.has_rock_ridge());

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.is_relocated_directory = true;
        assert!(attributes.has_rock_ridge());
    }

    #[test]
    fn test_read_data_with_susp_and_rrip_entries() -> Result<(), ErrorTrace> {
        let mut system_use: Vec<u8> = Vec::new();

        // SP entry (7 bytes).
        system_use.extend_from_slice(&[b'S', b'P', 7, 1, 0xbe, 0xef, 0]);

        // ER entry declaring IEEE_P1282.
        let extension_identifier: &[u8] = b"IEEE_P1282";
        let extension_reference_size: u8 = (8 + extension_identifier.len()) as u8;
        system_use.extend_from_slice(&[
            b'E',
            b'R',
            extension_reference_size,
            1,
            extension_identifier.len() as u8,
            0,
            0,
            1,
        ]);
        system_use.extend_from_slice(extension_identifier);

        // PX entry (44 bytes, RRIP 1.12 with inode number).
        system_use.extend_from_slice(&[b'P', b'X', 44, 1]);
        extend_both_u32(&mut system_use, 0o100644);
        extend_both_u32(&mut system_use, 2);
        extend_both_u32(&mut system_use, 1000);
        extend_both_u32(&mut system_use, 1001);
        extend_both_u32(&mut system_use, 42);

        // POSIX device number entry (20 bytes).
        system_use.extend_from_slice(&[0x50, 0x4e, 20, 1]);
        extend_both_u32(&mut system_use, 8);
        extend_both_u32(&mut system_use, 1);

        // NM split across two entries using CONTINUE flag (0x01).
        system_use.extend_from_slice(&[b'N', b'M', 9, 1, 0x01]);
        system_use.extend_from_slice(b"Long");
        system_use.extend_from_slice(&[b'N', b'M', 14, 1, 0x00]);
        system_use.extend_from_slice(b"File.txt!");

        // CL, PL, RE, PD, ES, RR entries.
        system_use.extend_from_slice(&[b'C', b'L', 12, 1]);
        extend_both_u32(&mut system_use, 50);
        system_use.extend_from_slice(&[b'P', b'L', 12, 1]);
        extend_both_u32(&mut system_use, 25);
        system_use.extend_from_slice(&[b'R', b'E', 4, 1]);
        system_use.extend_from_slice(&[b'P', b'D', 4, 1]);
        system_use.extend_from_slice(&[b'E', b'S', 5, 1, 0]);
        system_use.extend_from_slice(&[b'R', b'R', 5, 1, 0xff]);

        // TF entry (short form: creation, modify, access, attributes, backup, expiration, effective).
        let time_stamp_flags: u8 = CDFS_RRIP_TIME_STAMP_FLAG_CREATION
            | CDFS_RRIP_TIME_STAMP_FLAG_MODIFY
            | CDFS_RRIP_TIME_STAMP_FLAG_ACCESS
            | CDFS_RRIP_TIME_STAMP_FLAG_ATTRIBUTES
            | CDFS_RRIP_TIME_STAMP_FLAG_BACKUP
            | CDFS_RRIP_TIME_STAMP_FLAG_EXPIRATION
            | CDFS_RRIP_TIME_STAMP_FLAG_EFFECTIVE;
        system_use.extend_from_slice(&[b'T', b'F', 5 + 7 * 7, 1, time_stamp_flags]);
        for _ in 0..7 {
            system_use.extend_from_slice(&[0x6e, 0x08, 0x0c, 0x14, 0x06, 0x1f, 0x08]);
        }

        // ST terminator.
        system_use.extend_from_slice(&[b'S', b'T', 4, 1]);

        let data_stream: DataStreamReference = open_fake_data_stream(&[]);
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &system_use, 0)?;

        assert!(attributes.susp_detected);
        assert_eq!(attributes.susp_skip_length, 0);
        assert!(attributes.has_rock_ridge());
        assert_eq!(attributes.extensions, vec!["IEEE_P1282".to_string()]);
        assert_eq!(attributes.file_mode, Some(0o100644));
        assert_eq!(attributes.number_of_links, Some(2));
        assert_eq!(attributes.owner_identifier, Some(1000));
        assert_eq!(attributes.group_identifier, Some(1001));
        assert_eq!(attributes.inode_number, Some(42));
        assert_eq!(attributes.device_number, Some((8, 1)));
        assert_eq!(
            attributes.alternate_name,
            Some(ByteString {
                encoding: CharacterEncoding::Utf8,
                elements: b"LongFile.txt!".to_vec(),
            })
        );
        assert_eq!(attributes.child_link_sector, Some(50));
        assert_eq!(attributes.parent_link_sector, Some(25));
        assert!(attributes.is_relocated_directory);
        assert!(attributes.creation_time.is_some());
        assert!(attributes.modification_time.is_some());
        assert!(attributes.access_time.is_some());
        assert!(attributes.change_time.is_some());
        assert!(attributes.backup_time.is_some());
        assert!(attributes.expiration_time.is_some());
        assert!(attributes.effective_time.is_some());

        Ok(())
    }

    #[test]
    fn test_read_data_with_symbolic_link_and_continuation() -> Result<(), ErrorTrace> {
        // Put SL entry inside sector 1 as a CE continuation area.
        let mut continuation: Vec<u8> = Vec::new();
        // SL entry: root ("/"), "usr", "lo" + continue, "cal", parent (".."), current (".").
        let mut symbolic_link_body: Vec<u8> = vec![0x00];
        // Root component.
        symbolic_link_body.extend_from_slice(&[0x08, 0]);
        symbolic_link_body.extend_from_slice(&[0x00, 3, b'u', b's', b'r']);
        // Continued component.
        symbolic_link_body.extend_from_slice(&[0x01, 2, b'l', b'o']);
        symbolic_link_body.extend_from_slice(&[0x00, 3, b'c', b'a', b'l']);
        // Parent component.
        symbolic_link_body.extend_from_slice(&[0x04, 0]);
        // Current component.
        symbolic_link_body.extend_from_slice(&[0x02, 0]);
        continuation.extend_from_slice(&[b'S', b'L', (4 + symbolic_link_body.len()) as u8, 1]);
        continuation.extend_from_slice(&symbolic_link_body);

        // Long-form TF entry inside continuation area.
        continuation.extend_from_slice(&[
            b'T',
            b'F',
            5 + 17,
            1,
            CDFS_RRIP_TIME_STAMP_FLAG_LONG_FORM | CDFS_RRIP_TIME_STAMP_FLAG_MODIFY,
        ]);
        continuation.extend_from_slice(b"20260929095333000");

        let mut image_data: Vec<u8> = vec![0; 2 * 2048];
        image_data[2048..2048 + continuation.len()].copy_from_slice(&continuation);
        let data_stream: DataStreamReference = open_fake_data_stream(&image_data);

        // System Use area contains CE pointing to sector 1, offset 0, length = continuation.len().
        let mut system_use: Vec<u8> = vec![b'C', b'E', 28, 1];
        extend_both_u32(&mut system_use, 1);
        extend_both_u32(&mut system_use, 0);
        extend_both_u32(&mut system_use, continuation.len() as u32);

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &system_use, 0)?;

        assert_eq!(
            attributes.symbolic_link_target,
            Some(ByteString {
                encoding: CharacterEncoding::Utf8,
                elements: b"/usr/local/../.".to_vec(),
            })
        );
        assert_eq!(attributes.modification_time.unwrap().year, 2026);

        Ok(())
    }

    #[test]
    fn test_read_data_with_edge_cases() -> Result<(), ErrorTrace> {
        let data_stream: DataStreamReference = open_fake_data_stream(&[]);

        // Test with skip_length >= system_use_data.len().
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &[b'P', b'D', 4, 1], 4)?;
        assert!(!attributes.has_rock_ridge());

        // Test with invalid entry size (< 4) stopping iteration cleanly.
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &[b'P', b'D', 2, 1], 0)?;
        assert!(!attributes.has_rock_ridge());

        // Test with root-only symbolic link and NM current/parent special flags.
        let mut system_use: Vec<u8> = Vec::new();
        system_use.extend_from_slice(&[b'S', b'L', 7, 1, 0x00, 0x08, 0]);
        system_use.extend_from_slice(&[b'N', b'M', 5, 1, 0x02]);
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &system_use, 0)?;
        assert_eq!(attributes.symbolic_link_target.unwrap().elements, b"/");
        assert_eq!(attributes.alternate_name.unwrap().elements, b".");

        let system_use_parent: Vec<u8> = vec![b'N', b'M', 5, 1, 0x04];
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &system_use_parent, 0)?;
        assert_eq!(attributes.alternate_name.unwrap().elements, b"..");

        // Test with SL entry containing a truncated component.
        let truncated_symbolic_link: Vec<u8> = vec![b'S', b'L', 7, 1, 0x00, 0x00, 10];
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &truncated_symbolic_link, 0)?;
        assert!(attributes.symbolic_link_target.is_none());

        // Test with SL continued across two SL entries.
        let mut split_symbolic_link: Vec<u8> = Vec::new();
        split_symbolic_link.extend_from_slice(&[b'S', b'L', 9, 1, 0x01, 0x01, 2, b'a', b'b']);
        split_symbolic_link.extend_from_slice(&[b'S', b'L', 9, 1, 0x00, 0x00, 2, b'c', b'd']);
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.read_data(&data_stream, 2048, &split_symbolic_link, 0)?;
        assert_eq!(attributes.symbolic_link_target.unwrap().elements, b"abcd");

        Ok(())
    }

    #[test]
    fn test_read_data_with_errors() {
        let data_stream: DataStreamReference = open_fake_data_stream(&[]);

        // Test read_both_u32 with short slice.
        let result: Result<u32, ErrorTrace> = CdFsRockRidgeAttributes::read_both_u32(&[0; 4], 0);
        assert!(result.is_err());

        // Test mismatched both-byte values in CE entry fields.
        for offset in [4, 12, 20] {
            let mut invalid_continuation: Vec<u8> = vec![0; 28];
            invalid_continuation[0..4].copy_from_slice(&[b'C', b'E', 28, 1]);
            invalid_continuation[offset] = 0xff;
            let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
            let result: Result<(), ErrorTrace> =
                attributes.read_data(&data_stream, 2048, &invalid_continuation, 0);
            assert!(result.is_err());
        }

        // Test mismatched both-byte values in PX entry fields (mode, links, owner, group, inode).
        for offset in [4, 12, 20, 28, 36] {
            let mut invalid_posix: Vec<u8> = vec![0; 44];
            invalid_posix[0..4].copy_from_slice(&[b'P', b'X', 44, 1]);
            invalid_posix[offset] = 0xff;
            let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
            let result: Result<(), ErrorTrace> =
                attributes.read_data(&data_stream, 2048, &invalid_posix, 0);
            assert!(result.is_err());
        }

        // Test mismatched both-byte values in POSIX device number entry (major, minor).
        for offset in [4, 12] {
            let mut invalid_device: Vec<u8> = vec![0; 20];
            invalid_device[0..4].copy_from_slice(&[0x50, 0x4e, 20, 1]);
            invalid_device[offset] = 0xff;
            let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
            let result: Result<(), ErrorTrace> =
                attributes.read_data(&data_stream, 2048, &invalid_device, 0);
            assert!(result.is_err());
        }

        // Test mismatched both-byte values in CL and PL entries.
        for signature in [b'C', b'P'] {
            let mut invalid_link: Vec<u8> = vec![0; 12];
            invalid_link[0..4].copy_from_slice(&[signature, b'L', 12, 1]);
            invalid_link[4] = 0xff;
            let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
            let result: Result<(), ErrorTrace> =
                attributes.read_data(&data_stream, 2048, &invalid_link, 0);
            assert!(result.is_err());
        }

        // Test truncated TF entry.
        let invalid_time_stamp: Vec<u8> =
            vec![b'T', b'F', 6, 1, CDFS_RRIP_TIME_STAMP_FLAG_MODIFY, 0];
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        let result: Result<(), ErrorTrace> =
            attributes.read_data(&data_stream, 2048, &invalid_time_stamp, 0);
        assert!(result.is_err());

        // Test invalid long-form TF timestamp string.
        let mut invalid_long_time_stamp: Vec<u8> = vec![
            b'T',
            b'F',
            5 + 17,
            1,
            CDFS_RRIP_TIME_STAMP_FLAG_LONG_FORM | CDFS_RRIP_TIME_STAMP_FLAG_MODIFY,
        ];
        invalid_long_time_stamp.extend_from_slice(b"INVALIDTIMESTAMP!");
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        let result: Result<(), ErrorTrace> =
            attributes.read_data(&data_stream, 2048, &invalid_long_time_stamp, 0);
        assert!(result.is_err());

        // Test self-referencing CE continuation loop exceeding maximum depth.
        let mut loop_image: Vec<u8> = vec![0; 2048];
        let mut continuation_entry: Vec<u8> = vec![b'C', b'E', 28, 1];
        extend_both_u32(&mut continuation_entry, 0);
        extend_both_u32(&mut continuation_entry, 0);
        extend_both_u32(&mut continuation_entry, 28);
        loop_image[0..28].copy_from_slice(&continuation_entry);
        let loop_stream: DataStreamReference = open_fake_data_stream(&loop_image);

        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        let result: Result<(), ErrorTrace> =
            attributes.read_data(&loop_stream, 2048, &continuation_entry, 0);
        assert!(result.is_err());
    }
}
