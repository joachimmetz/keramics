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

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_datetime::CdFsDateTime;
use keramics_encodings::CharacterEncoding;
use keramics_types::ByteString;

use crate::block_stream::BlockStream;
use crate::indexed_hash_map::IndexedHashMap;
use crate::path_component::PathComponent;
use crate::traits::{BlockReader, FileEntryIterator};

use super::constants::*;
use super::directory_record::CdFsDirectoryRecord;
use super::extended_attribute_record::CdFsExtendedAttributeRecord;
use super::file_name::CdFsFileName;
use super::rock_ridge::CdFsRockRidgeAttributes;
use super::udf::{UdfDirectoryEntry, UdfFileEntry, UdfVolume};

/// Block reader for contiguous ISO 9660 / Joliet / Rock Ridge file extents.
struct CdFsExtentReader {
    /// Underlying data stream.
    data_stream: DataStreamReference,

    /// Start byte offset of the extent.
    offset: u64,

    /// Size of the extent in bytes.
    size: u64,
}

impl CdFsExtentReader {
    /// Creates a new extent reader.
    fn new(data_stream: &DataStreamReference, offset: u64, size: u64) -> Self {
        Self {
            data_stream: data_stream.clone(),
            offset,
            size,
        }
    }
}

impl BlockReader for CdFsExtentReader {
    /// Retrieves the size of the data.
    fn get_size(&self) -> u64 {
        self.size
    }

    /// Reads data from the extent at the specified offset.
    fn read_data_from_blocks(&mut self, data: &mut [u8], offset: u64) -> Result<usize, ErrorTrace> {
        if data.is_empty() || offset >= self.size {
            return Ok(0);
        }
        let read_size: usize = min(data.len(), (self.size - offset) as usize);

        keramics_core::data_stream_read_exact_at_position!(
            &self.data_stream,
            &mut data[..read_size],
            SeekFrom::Start(self.offset + offset)
        );
        Ok(read_size)
    }
}

/// Parsed ISO 9660 / Joliet / Rock Ridge sub-directory entry.
#[derive(Clone)]
struct CdFsIsoSubEntry {
    /// Byte offset of the directory record.
    directory_record_offset: u64,

    /// Underlying directory record.
    directory_record: CdFsDirectoryRecord,

    /// Rock Ridge attributes, if present.
    rock_ridge_attributes: Option<CdFsRockRidgeAttributes>,

    /// Effective entry name (Rock Ridge `NM` if present, otherwise directory record name).
    effective_name: ByteString,

    /// Value indicating whether the entry uses a Rock Ridge alternate name.
    has_alternate_name: bool,

    /// Effective data start sector (resolved through Rock Ridge `CL` if relocated).
    effective_start_sector: u32,

    /// Effective data size (resolved through Rock Ridge `CL` if relocated).
    effective_data_size: u32,

    /// Value indicating whether the entry is a directory.
    is_directory: bool,
}

/// Backing storage for a [`CdFsFileEntry`].
enum CdFsFileEntryData {
    /// ISO 9660, Joliet, or Rock Ridge file entry.
    Iso9660 {
        /// The data stream.
        data_stream: DataStreamReference,

        /// Bytes per sector.
        bytes_per_sector: u16,

        /// Character encoding used for directory record names.
        encoding: CharacterEncoding,

        /// Value indicating whether Rock Ridge extensions are active.
        rock_ridge_enabled: bool,

        /// SUSP system use skip length (`LEN_SKP`).
        susp_skip_length: usize,

        /// The offset of the directory record.
        directory_record_offset: u64,

        /// The directory record.
        directory_record: CdFsDirectoryRecord,

        /// Parsed Rock Ridge attributes, if present.
        rock_ridge_attributes: Option<CdFsRockRidgeAttributes>,

        /// Effective name (Rock Ridge `NM` if present, else directory record name).
        effective_name: Option<ByteString>,

        /// Effective data start sector (may differ from directory record when relocated via `CL`).
        effective_start_sector: u32,

        /// Effective data size (may differ from directory record when relocated via `CL`).
        effective_data_size: u32,

        /// Value indicating whether the entry is a directory.
        is_directory: bool,

        /// The extended attribute record.
        extended_attribute_record: Option<CdFsExtendedAttributeRecord>,

        /// The sub directory entries.
        sub_entries: IndexedHashMap<ByteString, CdFsIsoSubEntry>,

        /// Value to indicate the sub directory entries were read.
        sub_entries_read: bool,
    },

    /// UDF file entry.
    Udf {
        /// UDF volume reader.
        udf_volume: UdfVolume,

        /// Parsed UDF file entry descriptor.
        file_entry: UdfFileEntry,

        /// Entry name (`None` for the root directory).
        name: Option<ByteString>,

        /// Value indicating whether this entry is the root directory.
        is_root: bool,

        /// Symbolic link target, if this entry is a symbolic link.
        symbolic_link_target: Option<ByteString>,

        /// Sub directory entries.
        sub_entries: IndexedHashMap<ByteString, UdfDirectoryEntry>,

        /// Value to indicate the sub directory entries were read.
        sub_entries_read: bool,
    },
}

/// CD file system (CDFS) file entry.
pub struct CdFsFileEntry {
    /// Internal entry representation.
    data: CdFsFileEntryData,
}

impl CdFsFileEntry {
    /// Creates a new ISO 9660 file entry.
    // Used for testing.
    #[cfg(test)]
    pub(super) fn new(
        data_stream: &DataStreamReference,
        bytes_per_sector: u16,
        directory_record_offset: u64,
        directory_record: CdFsDirectoryRecord,
    ) -> Self {
        Self::new_with_options(
            data_stream,
            bytes_per_sector,
            CharacterEncoding::Ascii,
            false,
            0,
            directory_record_offset,
            directory_record,
            None,
        )
    }

    /// Creates a new UDF file entry.
    pub(super) fn new_udf(
        udf_volume: UdfVolume,
        file_entry: UdfFileEntry,
        name: Option<ByteString>,
        is_root: bool,
        symbolic_link_target: Option<ByteString>,
    ) -> Self {
        Self {
            data: CdFsFileEntryData::Udf {
                udf_volume,
                file_entry,
                name,
                is_root,
                symbolic_link_target,
                sub_entries: IndexedHashMap::new(),
                sub_entries_read: false,
            },
        }
    }

    /// Creates a new ISO 9660 / Joliet / Rock Ridge file entry with extension options.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_with_options(
        data_stream: &DataStreamReference,
        bytes_per_sector: u16,
        encoding: CharacterEncoding,
        rock_ridge_enabled: bool,
        susp_skip_length: usize,
        directory_record_offset: u64,
        directory_record: CdFsDirectoryRecord,
        rock_ridge_attributes: Option<CdFsRockRidgeAttributes>,
    ) -> Self {
        let effective_name: Option<ByteString> = match &rock_ridge_attributes {
            Some(attributes) if attributes.alternate_name.is_some() => {
                attributes.alternate_name.clone()
            }
            _ => {
                if directory_record.name.is_empty() {
                    None
                } else {
                    Some(directory_record.name.clone())
                }
            }
        };
        let effective_start_sector: u32 = directory_record.data_start_sector;
        let effective_data_size: u32 = directory_record.data_size;
        let is_directory: bool = (directory_record.file_flags & CDFS_FILE_FLAG_DIRECTORY) != 0;

        Self {
            data: CdFsFileEntryData::Iso9660 {
                data_stream: data_stream.clone(),
                bytes_per_sector,
                encoding,
                rock_ridge_enabled,
                susp_skip_length,
                directory_record_offset,
                directory_record,
                rock_ridge_attributes,
                effective_name,
                effective_start_sector,
                effective_data_size,
                is_directory,
                extended_attribute_record: None,
                sub_entries: IndexedHashMap::new(),
                sub_entries_read: false,
            },
        }
    }

    /// Builds a child [`CdFsFileEntry`] from an ISO 9660 / Joliet / Rock Ridge sub-entry.
    fn create_sub_entry_from_iso(
        data_stream: &DataStreamReference,
        bytes_per_sector: u16,
        encoding: &CharacterEncoding,
        rock_ridge_enabled: bool,
        susp_skip_length: usize,
        sub_entry: &CdFsIsoSubEntry,
    ) -> Self {
        Self {
            data: CdFsFileEntryData::Iso9660 {
                data_stream: data_stream.clone(),
                bytes_per_sector,
                encoding: encoding.clone(),
                rock_ridge_enabled,
                susp_skip_length,
                directory_record_offset: sub_entry.directory_record_offset,
                directory_record: sub_entry.directory_record.clone(),
                rock_ridge_attributes: sub_entry.rock_ridge_attributes.clone(),
                effective_name: Some(sub_entry.effective_name.clone()),
                effective_start_sector: sub_entry.effective_start_sector,
                effective_data_size: sub_entry.effective_data_size,
                is_directory: sub_entry.is_directory,
                extended_attribute_record: None,
                sub_entries: IndexedHashMap::new(),
                sub_entries_read: false,
            },
        }
    }

    /// Builds a child [`CdFsFileEntry`] from a UDF directory entry.
    fn create_sub_entry_from_udf(
        udf_volume: &UdfVolume,
        directory_entry: &UdfDirectoryEntry,
    ) -> Result<Self, ErrorTrace> {
        let file_entry: UdfFileEntry = match udf_volume.read_file_entry(&directory_entry.icb) {
            Ok(entry) => entry,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read UDF sub file entry");
                return Err(error);
            }
        };
        let symbolic_link_target: Option<ByteString> =
            if file_entry.file_type == UDF_FILE_TYPE_SYMBOLIC_LINK {
                match udf_volume.read_symbolic_link_target(&file_entry) {
                    Ok(target) => target,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to read UDF symbolic link target"
                        );
                        return Err(error);
                    }
                }
            } else {
                None
            };
        Ok(Self::new_udf(
            udf_volume.clone(),
            file_entry,
            Some(directory_entry.name.clone()),
            false,
            symbolic_link_target,
        ))
    }

    /// Retrieves the access time (from Rock Ridge `"TF"` or UDF).
    pub fn get_access_time(&self) -> Option<&CdFsDateTime> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.access_time.as_ref()),
            CdFsFileEntryData::Udf { file_entry, .. } => file_entry.access_time.as_ref(),
        }
    }

    /// Retrieves the attribute change time (from Rock Ridge `"TF"` or UDF).
    pub fn get_change_time(&self) -> Option<&CdFsDateTime> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.change_time.as_ref()),
            CdFsFileEntryData::Udf { file_entry, .. } => file_entry.change_time.as_ref(),
        }
    }

    /// Retrieves the creation time.
    pub fn get_creation_time(&self) -> Option<&CdFsDateTime> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                extended_attribute_record,
                ..
            } => {
                if let Some(attributes) = rock_ridge_attributes {
                    if let Some(creation_time) = &attributes.creation_time {
                        return Some(creation_time);
                    }
                }
                extended_attribute_record
                    .as_ref()
                    .map(|record| &record.creation_time)
            }
            CdFsFileEntryData::Udf { file_entry, .. } => file_entry.creation_time.as_ref(),
        }
    }

    /// Retrieves the default data stream for a regular file entry.
    pub fn get_data_stream(&self) -> Result<Option<DataStreamReference>, ErrorTrace> {
        if self.is_directory() || self.is_symbolic_link() {
            return Ok(None);
        }
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                data_stream,
                bytes_per_sector,
                directory_record,
                effective_start_sector,
                effective_data_size,
                ..
            } => {
                let extent_offset: u64 = ((*effective_start_sector as u64)
                    + (directory_record.extended_attribute_record_size as u64))
                    * (*bytes_per_sector as u64);
                let reader: CdFsExtentReader =
                    CdFsExtentReader::new(data_stream, extent_offset, *effective_data_size as u64);
                Ok(Some(Arc::new(RwLock::new(BlockStream::new(reader)))))
            }
            CdFsFileEntryData::Udf {
                udf_volume,
                file_entry,
                ..
            } => Ok(Some(udf_volume.get_file_entry_data_stream(file_entry))),
        }
    }

    /// Retrieves the POSIX device major and minor numbers (from Rock Ridge device number entry).
    pub fn get_device_number(&self) -> Option<(u32, u32)> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.device_number),
            CdFsFileEntryData::Udf { .. } => None,
        }
    }

    /// Retrieves the effective time.
    pub fn get_effective_time(&self) -> Option<&CdFsDateTime> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                extended_attribute_record,
                ..
            } => {
                if let Some(attributes) = rock_ridge_attributes {
                    if let Some(effective_time) = &attributes.effective_time {
                        return Some(effective_time);
                    }
                }
                extended_attribute_record
                    .as_ref()
                    .map(|record| &record.effective_time)
            }
            CdFsFileEntryData::Udf { .. } => None,
        }
    }

    /// Retrieves the expiration time.
    pub fn get_expiration_time(&self) -> Option<&CdFsDateTime> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                extended_attribute_record,
                ..
            } => {
                if let Some(attributes) = rock_ridge_attributes {
                    if let Some(expiration_time) = &attributes.expiration_time {
                        return Some(expiration_time);
                    }
                }
                extended_attribute_record
                    .as_ref()
                    .map(|record| &record.expiration_time)
            }
            CdFsFileEntryData::Udf { .. } => None,
        }
    }

    /// Retrieves the POSIX file mode (from Rock Ridge `"PX"` or UDF).
    pub fn get_file_mode(&self) -> Option<u32> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.file_mode),
            CdFsFileEntryData::Udf { file_entry, .. } => Some(file_entry.file_mode),
        }
    }

    /// Retrieves the group identifier.
    pub fn get_group_identifier(&self) -> Option<u16> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                extended_attribute_record,
                rock_ridge_attributes,
                ..
            } => {
                if let Some(record) = extended_attribute_record {
                    return Some(record.group_identifier);
                }
                rock_ridge_attributes
                    .as_ref()
                    .and_then(|attributes| attributes.group_identifier)
                    .and_then(|value| u16::try_from(value).ok())
            }
            CdFsFileEntryData::Udf { file_entry, .. } => {
                u16::try_from(file_entry.group_identifier).ok()
            }
        }
    }

    /// Retrieves the identifier.
    pub fn get_identifier(&self) -> u64 {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                directory_record_offset,
                ..
            } => *directory_record_offset,
            CdFsFileEntryData::Udf { file_entry, .. } => file_entry.offset,
        }
    }

    /// Retrieves the modification time.
    pub fn get_modification_time(&self) -> Option<&CdFsDateTime> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                extended_attribute_record,
                ..
            } => {
                if let Some(attributes) = rock_ridge_attributes {
                    if let Some(modification_time) = &attributes.modification_time {
                        return Some(modification_time);
                    }
                }
                extended_attribute_record
                    .as_ref()
                    .map(|record| &record.modification_time)
            }
            CdFsFileEntryData::Udf { file_entry, .. } => file_entry.modification_time.as_ref(),
        }
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> Option<&ByteString> {
        match &self.data {
            CdFsFileEntryData::Iso9660 { effective_name, .. } => effective_name.as_ref(),
            CdFsFileEntryData::Udf { name, .. } => name.as_ref(),
        }
    }

    /// Retrieves the link count (from Rock Ridge `"PX"` or UDF).
    pub fn get_number_of_links(&self) -> Option<u32> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.number_of_links),
            CdFsFileEntryData::Udf { file_entry, .. } => Some(file_entry.number_of_links as u32),
        }
    }

    /// Retrieves the owner identifier.
    pub fn get_owner_identifier(&self) -> Option<u16> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                extended_attribute_record,
                rock_ridge_attributes,
                ..
            } => {
                if let Some(record) = extended_attribute_record {
                    return Some(record.owner_identifier);
                }
                rock_ridge_attributes
                    .as_ref()
                    .and_then(|attributes| attributes.owner_identifier)
                    .and_then(|value| u16::try_from(value).ok())
            }
            CdFsFileEntryData::Udf { file_entry, .. } => {
                u16::try_from(file_entry.owner_identifier).ok()
            }
        }
    }

    /// Retrieves the permissions.
    pub fn get_permissions(&self) -> Option<u16> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                extended_attribute_record,
                ..
            } => extended_attribute_record
                .as_ref()
                .map(|record| record.permissions),
            CdFsFileEntryData::Udf { file_entry, .. } => {
                Some((file_entry.permissions & 0xffff) as u16)
            }
        }
    }

    /// Retrieves the 32-bit POSIX group identifier (from Rock Ridge `"PX"` or UDF).
    pub fn get_posix_group_identifier(&self) -> Option<u32> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.group_identifier),
            CdFsFileEntryData::Udf { file_entry, .. } => Some(file_entry.group_identifier),
        }
    }

    /// Retrieves the 32-bit POSIX owner identifier (from Rock Ridge `"PX"` or UDF).
    pub fn get_posix_owner_identifier(&self) -> Option<u32> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.owner_identifier),
            CdFsFileEntryData::Udf { file_entry, .. } => Some(file_entry.owner_identifier),
        }
    }

    /// Retrieves the size.
    pub fn get_size(&self) -> u64 {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                effective_data_size,
                ..
            } => *effective_data_size as u64,
            CdFsFileEntryData::Udf { file_entry, .. } => file_entry.size,
        }
    }

    /// Retrieves a specific sub file entry by name.
    pub fn get_sub_file_entry_by_name(
        &mut self,
        sub_file_entry_name: &PathComponent,
    ) -> Result<Option<Self>, ErrorTrace> {
        if self.is_directory() {
            match self.read_sub_entries() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory records"
                    );
                    return Err(error);
                }
            }
        }
        match &mut self.data {
            CdFsFileEntryData::Iso9660 {
                data_stream,
                bytes_per_sector,
                encoding,
                rock_ridge_enabled,
                susp_skip_length,
                sub_entries,
                ..
            } => {
                match sub_entries.iter_mut().find(|(_, sub_entry)| {
                    let entry_name: CdFsFileName = if sub_entry.has_alternate_name {
                        CdFsFileName::new_exact(sub_entry.effective_name.clone())
                    } else {
                        CdFsFileName::new(sub_entry.effective_name.clone())
                    };
                    entry_name.compare(sub_file_entry_name)
                }) {
                    Some((_, sub_entry)) => Ok(Some(Self::create_sub_entry_from_iso(
                        data_stream,
                        *bytes_per_sector,
                        encoding,
                        *rock_ridge_enabled,
                        *susp_skip_length,
                        sub_entry,
                    ))),
                    None => Ok(None),
                }
            }
            CdFsFileEntryData::Udf {
                udf_volume,
                sub_entries,
                ..
            } => {
                match sub_entries.iter_mut().find(|(_, directory_entry)| {
                    CdFsFileName::new_exact(directory_entry.name.clone())
                        .compare(sub_file_entry_name)
                }) {
                    Some((_, directory_entry)) => {
                        let sub_file_entry: Self =
                            match Self::create_sub_entry_from_udf(udf_volume, directory_entry) {
                                Ok(entry) => entry,
                                Err(mut error) => {
                                    keramics_core::error_trace_add_frame!(
                                        error,
                                        "Unable to create UDF sub file entry"
                                    );
                                    return Err(error);
                                }
                            };
                        Ok(Some(sub_file_entry))
                    }
                    None => Ok(None),
                }
            }
        }
    }

    /// Retrieves the symbolic link target (from Rock Ridge `"SL"` or UDF).
    pub fn get_symbolic_link_target(&self) -> Option<&ByteString> {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => rock_ridge_attributes
                .as_ref()
                .and_then(|attributes| attributes.symbolic_link_target.as_ref()),
            CdFsFileEntryData::Udf {
                symbolic_link_target,
                ..
            } => symbolic_link_target.as_ref(),
        }
    }

    /// Determines if the file entry is a directory.
    pub fn is_directory(&self) -> bool {
        match &self.data {
            CdFsFileEntryData::Iso9660 { is_directory, .. } => *is_directory,
            CdFsFileEntryData::Udf { file_entry, .. } => {
                file_entry.file_type == UDF_FILE_TYPE_DIRECTORY
            }
        }
    }

    /// Determines if the file entry is the root directory.
    pub fn is_root_directory(&self) -> bool {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                directory_record, ..
            } => directory_record.name.is_empty(),
            CdFsFileEntryData::Udf { is_root, .. } => *is_root,
        }
    }

    /// Determines if the file entry is a symbolic link.
    pub fn is_symbolic_link(&self) -> bool {
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                rock_ridge_attributes,
                ..
            } => match rock_ridge_attributes {
                Some(attributes) => {
                    attributes.symbolic_link_target.is_some()
                        || attributes
                            .file_mode
                            .map(|mode| (mode & 0o170000) == 0o120000)
                            .unwrap_or(false)
                }
                None => false,
            },
            CdFsFileEntryData::Udf { file_entry, .. } => {
                file_entry.file_type == UDF_FILE_TYPE_SYMBOLIC_LINK
            }
        }
    }

    /// Reads the extended attribute record.
    // Used for testing.
    #[cfg(test)]
    fn read_extended_attribute_record(
        &mut self,
    ) -> Result<CdFsExtendedAttributeRecord, ErrorTrace> {
        let (data_stream, bytes_per_sector, directory_record): (
            &DataStreamReference,
            u16,
            &CdFsDirectoryRecord,
        ) = match &self.data {
            CdFsFileEntryData::Iso9660 {
                data_stream,
                bytes_per_sector,
                directory_record,
                ..
            } => (data_stream, *bytes_per_sector, directory_record),
            CdFsFileEntryData::Udf { .. } => {
                return Err(keramics_core::error_trace_new!(
                    "Extended attribute records are not supported for UDF file entries"
                ));
            }
        };
        if directory_record.data_size == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported directory record data size"
            ));
        }
        let data_start_offset: u64 =
            (directory_record.data_start_sector as u64) * (bytes_per_sector as u64);
        let data_size: usize = directory_record.extended_attribute_record_size as usize;

        // TODO: read in blocks of self.bytes_per_sector
        let mut data: Vec<u8> = vec![0; data_size];

        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut data,
            SeekFrom::Start(data_start_offset)
        );
        keramics_core::debug_trace_data!(
            "CdFsExtendedAttributeRecord",
            data_start_offset,
            &data,
            data.len(),
        );
        let mut extended_attribute_record: CdFsExtendedAttributeRecord =
            CdFsExtendedAttributeRecord::new();

        match extended_attribute_record.read_data(&data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read extended attribute record"
                );
                return Err(error);
            }
        }
        Ok(extended_attribute_record)
    }

    /// Reads the `"."` directory record of a Rock Ridge relocated directory (`"CL"` target).
    fn read_relocated_directory_record(
        data_stream: &DataStreamReference,
        bytes_per_sector: u16,
        encoding: &CharacterEncoding,
        target_sector: u32,
    ) -> Result<CdFsDirectoryRecord, ErrorTrace> {
        let sector_offset: u64 = (target_sector as u64) * (bytes_per_sector as u64);
        let mut sector_data: Vec<u8> = vec![0; bytes_per_sector as usize];

        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut sector_data,
            SeekFrom::Start(sector_offset)
        );
        let record_size: usize = sector_data[0] as usize;
        if record_size < 33 || record_size > sector_data.len() {
            return Err(keramics_core::error_trace_new!(
                "Unsupported Rock Ridge relocated directory record size"
            ));
        }
        let mut directory_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        match directory_record.read_data_with_encoding(&sector_data[..record_size], encoding) {
            Ok(_) => Ok(directory_record),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read Rock Ridge relocated directory record"
                );
                Err(error)
            }
        }
    }

    /// Reads the sub directory records for an ISO 9660 / Joliet / Rock Ridge directory.
    fn read_sub_directory_records(&mut self) -> Result<(), ErrorTrace> {
        let (
            data_stream,
            bytes_per_sector,
            encoding,
            rock_ridge_enabled,
            susp_skip_length,
            effective_start_sector,
            effective_data_size,
            sub_entries,
            sub_entries_read,
        ) = match &mut self.data {
            CdFsFileEntryData::Iso9660 {
                data_stream,
                bytes_per_sector,
                encoding,
                rock_ridge_enabled,
                susp_skip_length,
                effective_start_sector,
                effective_data_size,
                sub_entries,
                sub_entries_read,
                ..
            } => (
                data_stream,
                *bytes_per_sector,
                encoding,
                *rock_ridge_enabled,
                *susp_skip_length,
                *effective_start_sector,
                *effective_data_size,
                sub_entries,
                sub_entries_read,
            ),
            CdFsFileEntryData::Udf { .. } => return Ok(()),
        };
        if *sub_entries_read {
            return Ok(());
        }
        if effective_data_size == 0 {
            return Err(keramics_core::error_trace_new!(
                "Missing directory record data size"
            ));
        }
        let offset: u64 = (effective_start_sector as u64) * (bytes_per_sector as u64);

        let mut data: Vec<u8> = vec![0; effective_data_size as usize];

        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut data,
            SeekFrom::Start(offset)
        );
        keramics_core::debug_trace_data!(
            "CdFsDirectoryRecords",
            offset,
            &data,
            effective_data_size,
        );
        let mut data_offset: usize = 0;
        let data_size: usize = data.len();
        let sector_size: usize = bytes_per_sector as usize;

        while data_offset < data_size {
            let record_size: usize = data[data_offset] as usize;

            if record_size == 0 {
                if sector_size == 0 {
                    break;
                }
                let next_sector_offset: usize = ((data_offset / sector_size) + 1) * sector_size;
                if next_sector_offset >= data_size {
                    break;
                }
                data_offset = next_sector_offset;
                continue;
            }
            let record_end_offset: usize = data_offset + record_size;

            if record_end_offset > data_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported directory record size: {} out of bounds at offset: {}",
                    record_size, data_offset
                )));
            }
            let mut directory_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

            match directory_record
                .read_data_with_encoding(&data[data_offset..record_end_offset], encoding)
            {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read directory record at offset: {} (0x{:08x})",
                            data_offset, data_offset
                        )
                    );
                    return Err(error);
                }
            }
            // Skip the directory itself (0x00 -> empty) and parent directory (0x01) entries.
            let is_self_or_parent: bool = directory_record.name.is_empty()
                || (directory_record.name.encoding == CharacterEncoding::Ascii
                    && directory_record.name.elements == [0x01]);

            if !is_self_or_parent {
                let mut rock_ridge_attributes: Option<CdFsRockRidgeAttributes> = None;
                let mut effective_name: ByteString = directory_record.name.clone();
                let mut has_alternate_name: bool = false;
                let mut sub_start_sector: u32 = directory_record.data_start_sector;
                let mut sub_data_size: u32 = directory_record.data_size;
                let mut sub_is_directory: bool =
                    (directory_record.file_flags & CDFS_FILE_FLAG_DIRECTORY) != 0;
                let mut skip_relocated: bool = false;

                if rock_ridge_enabled && !directory_record.system_use_data.is_empty() {
                    let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
                    match attributes.read_data(
                        data_stream,
                        bytes_per_sector,
                        &directory_record.system_use_data,
                        susp_skip_length,
                    ) {
                        Ok(_) => {}
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                "Unable to read Rock Ridge attributes"
                            );
                            return Err(error);
                        }
                    }
                    if attributes.is_relocated_directory {
                        skip_relocated = true;
                    } else {
                        if let Some(alternate_name) = &attributes.alternate_name {
                            effective_name = alternate_name.clone();
                            has_alternate_name = true;
                        }
                        if let Some(child_link_sector) = attributes.child_link_sector {
                            let target_record: CdFsDirectoryRecord =
                                match Self::read_relocated_directory_record(
                                    data_stream,
                                    bytes_per_sector,
                                    encoding,
                                    child_link_sector,
                                ) {
                                    Ok(record) => record,
                                    Err(mut error) => {
                                        keramics_core::error_trace_add_frame!(
                                            error,
                                            "Unable to read Rock Ridge child link target directory"
                                        );
                                        return Err(error);
                                    }
                                };
                            sub_start_sector = child_link_sector;
                            sub_data_size = target_record.data_size;
                            sub_is_directory = true;
                        }
                        rock_ridge_attributes = Some(attributes);
                    }
                }
                if !skip_relocated {
                    let entry_key: ByteString = effective_name.clone();
                    sub_entries.insert(
                        entry_key,
                        CdFsIsoSubEntry {
                            directory_record_offset: offset + (data_offset as u64),
                            directory_record,
                            rock_ridge_attributes,
                            effective_name,
                            has_alternate_name,
                            effective_start_sector: sub_start_sector,
                            effective_data_size: sub_data_size,
                            is_directory: sub_is_directory,
                        },
                    );
                }
            }
            data_offset = record_end_offset;
        }
        *sub_entries_read = true;

        Ok(())
    }

    /// Reads sub entries for either ISO 9660 / Joliet / Rock Ridge or UDF directories.
    fn read_sub_entries(&mut self) -> Result<(), ErrorTrace> {
        match &self.data {
            CdFsFileEntryData::Iso9660 { .. } => self.read_sub_directory_records(),
            CdFsFileEntryData::Udf { .. } => self.read_udf_sub_entries(),
        }
    }

    /// Reads the sub directory entries for a UDF directory.
    fn read_udf_sub_entries(&mut self) -> Result<(), ErrorTrace> {
        let (udf_volume, file_entry, sub_entries, sub_entries_read) = match &mut self.data {
            CdFsFileEntryData::Udf {
                udf_volume,
                file_entry,
                sub_entries,
                sub_entries_read,
                ..
            } => (udf_volume, file_entry, sub_entries, sub_entries_read),
            CdFsFileEntryData::Iso9660 { .. } => return Ok(()),
        };
        if *sub_entries_read {
            return Ok(());
        }
        let entries: Vec<UdfDirectoryEntry> = match udf_volume.read_directory_entries(file_entry) {
            Ok(entries) => entries,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read UDF sub directory entries"
                );
                return Err(error);
            }
        };
        for entry in entries {
            sub_entries.insert(entry.name.clone(), entry);
        }
        *sub_entries_read = true;

        Ok(())
    }
}

impl FileEntryIterator for CdFsFileEntry {
    /// Retrieves the number of sub file entries.
    fn get_number_of_sub_file_entries(&mut self) -> Result<usize, ErrorTrace> {
        if self.is_directory() {
            match self.read_sub_entries() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory records"
                    );
                    return Err(error);
                }
            }
        }
        match &self.data {
            CdFsFileEntryData::Iso9660 { sub_entries, .. } => Ok(sub_entries.len()),
            CdFsFileEntryData::Udf { sub_entries, .. } => Ok(sub_entries.len()),
        }
    }

    /// Retrieves a specific sub file entry.
    fn get_sub_file_entry_by_index(
        &mut self,
        sub_file_entry_index: usize,
    ) -> Result<Self, ErrorTrace> {
        if self.is_directory() {
            match self.read_sub_entries() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory records"
                    );
                    return Err(error);
                }
            }
        }
        match &self.data {
            CdFsFileEntryData::Iso9660 {
                data_stream,
                bytes_per_sector,
                encoding,
                rock_ridge_enabled,
                susp_skip_length,
                sub_entries,
                ..
            } => match sub_entries.get_key_value_by_index(sub_file_entry_index) {
                Some((_, sub_entry)) => Ok(Self::create_sub_entry_from_iso(
                    data_stream,
                    *bytes_per_sector,
                    encoding,
                    *rock_ridge_enabled,
                    *susp_skip_length,
                    sub_entry,
                )),
                None => Err(keramics_core::error_trace_new!(format!(
                    "Unable to retrieve sub file entry: {}",
                    sub_file_entry_index
                ))),
            },
            CdFsFileEntryData::Udf {
                udf_volume,
                sub_entries,
                ..
            } => match sub_entries.get_key_value_by_index(sub_file_entry_index) {
                Some((_, directory_entry)) => {
                    Self::create_sub_entry_from_udf(udf_volume, directory_entry)
                }
                None => Err(keramics_core::error_trace_new!(format!(
                    "Unable to retrieve sub file entry: {}",
                    sub_file_entry_index
                ))),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::{open_fake_data_stream, open_os_data_stream};
    use keramics_encodings::CharacterEncoding;

    use crate::cdfs::file_system::CdFsFileSystem;
    use crate::cdfs::volume::CdFsVolume;
    use crate::path::Path;
    use crate::tests::get_test_data_path;

    fn get_file_system(path_string: &str) -> Result<CdFsFileSystem, ErrorTrace> {
        let mut volume: CdFsVolume = CdFsVolume::new();

        let test_data_path_string: String = get_test_data_path(path_string);
        let path_buf: PathBuf = PathBuf::from(test_data_path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        volume.read_data_stream(&data_stream)?;

        volume.get_file_system()
    }

    #[test]
    fn test_get_creation_time() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_creation_time(), None);

        Ok(())
    }

    #[test]
    fn test_get_data_stream() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let directory_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();
        assert!(directory_entry.get_data_stream()?.is_none());

        let path: Path = Path::from("/testdir1/testfile1");
        let file_entry: CdFsFileEntry = cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();
        let data_stream: DataStreamReference = file_entry.get_data_stream()?.unwrap();
        let mut data: Vec<u8> = vec![0; file_entry.get_size() as usize];
        keramics_core::data_stream_read_exact_at_position!(
            &data_stream,
            &mut data,
            SeekFrom::Start(0)
        );
        assert_eq!(data.len(), 9);

        Ok(())
    }

    #[test]
    fn test_get_effective_time() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_effective_time(), None);

        Ok(())
    }

    #[test]
    fn test_get_expiration_time() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_expiration_time(), None);

        Ok(())
    }

    #[test]
    fn test_get_group_identifier() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_group_identifier(), None);

        Ok(())
    }

    #[test]
    fn test_get_identifier() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_identifier(), 32924);

        let path: Path = Path::from("/emptyfile");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_identifier(), 47172);

        Ok(())
    }

    #[test]
    fn test_get_modification_time() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_modification_time(), None);

        Ok(())
    }

    #[test]
    fn test_get_name() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let name: Option<&ByteString> = cdfs_file_entry.get_name();
        assert_eq!(name, None);

        let path: Path = Path::from("/emptyfile");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let name: Option<&ByteString> = cdfs_file_entry.get_name();
        assert_eq!(
            name,
            Some(ByteString {
                encoding: CharacterEncoding::Ascii,
                elements: b"EMPTYFILE.;1".to_vec(),
            })
            .as_ref(),
        );

        Ok(())
    }

    #[test]
    fn test_get_owner_identifier() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_owner_identifier(), None);

        Ok(())
    }

    #[test]
    fn test_get_permissions() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_permissions(), None);

        Ok(())
    }

    #[test]
    fn test_get_size() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/emptyfile");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_size(), 0);

        let path: Path = Path::from("/testdir1/testfile1");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.get_size(), 9);

        Ok(())
    }

    #[test]
    fn test_get_number_of_sub_file_entries() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let mut cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let number_of_sub_file_entries: usize = cdfs_file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 8);

        let path: Path = Path::from("/testdir1");
        let mut cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let number_of_sub_file_entries: usize = cdfs_file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 10);

        let path: Path = Path::from("/testdir1/testfile1");
        let mut cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let number_of_sub_file_entries: usize = cdfs_file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 0);

        Ok(())
    }

    #[test]
    fn test_get_sub_file_entry_by_index() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let mut cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let sub_file_entry: CdFsFileEntry = cdfs_file_entry.get_sub_file_entry_by_index(0)?;

        let name: Option<&ByteString> = sub_file_entry.get_name();
        assert_eq!(
            name,
            Some(ByteString {
                encoding: CharacterEncoding::Ascii,
                elements: b"EMPTYFILE.;1".to_vec(),
            })
            .as_ref(),
        );

        let result: Result<CdFsFileEntry, ErrorTrace> =
            cdfs_file_entry.get_sub_file_entry_by_index(8);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_get_sub_file_entry_by_name() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let mut cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        let name: PathComponent = PathComponent::from("EMPTYFILE");
        let result: Option<CdFsFileEntry> = cdfs_file_entry.get_sub_file_entry_by_name(&name)?;
        assert!(result.is_some());

        let name: PathComponent = PathComponent::from("TESTDIR1");
        let result: Option<CdFsFileEntry> = cdfs_file_entry.get_sub_file_entry_by_name(&name)?;
        assert!(result.unwrap().is_directory());

        let name: PathComponent = PathComponent::from("NONEXISTENT");
        let result: Option<CdFsFileEntry> = cdfs_file_entry.get_sub_file_entry_by_name(&name)?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_is_directory() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.is_directory(), true);

        let path: Path = Path::from("/testdir1");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.is_directory(), true);

        let path: Path = Path::from("/testdir1/testfile1");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.is_directory(), false);

        Ok(())
    }

    #[test]
    fn test_is_root_directory() -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = get_file_system("cdfs/level3.iso")?;

        let path: Path = Path::from("/");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.is_root_directory(), true);

        let path: Path = Path::from("/testdir1");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.is_root_directory(), false);

        let path: Path = Path::from("/testdir1/testfile1");
        let cdfs_file_entry: CdFsFileEntry =
            cdfs_file_system.get_file_entry_by_path(&path)?.unwrap();

        assert_eq!(cdfs_file_entry.is_root_directory(), false);

        Ok(())
    }

    #[test]
    fn test_read_extended_attribute_record() -> Result<(), ErrorTrace> {
        let mut extended_attribute_record_data: Vec<u8> = vec![0; 2048];
        extended_attribute_record_data[0..10]
            .copy_from_slice(&[0x0a, 0x00, 0x00, 0x0a, 0x14, 0x00, 0x00, 0x14, 0x01, 0xff]);
        extended_attribute_record_data[10..27].copy_from_slice(b"20100812200631008");
        extended_attribute_record_data[27..44].copy_from_slice(b"20100812200631008");
        extended_attribute_record_data[44..61].copy_from_slice(b"20100812200631008");
        extended_attribute_record_data[61..78].copy_from_slice(b"20100812200631008");
        let data_stream: DataStreamReference =
            open_fake_data_stream(&extended_attribute_record_data);

        let mut directory_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        directory_record.data_start_sector = 0;
        directory_record.data_size = 2048;
        directory_record.extended_attribute_record_size = 78;

        let mut file_entry: CdFsFileEntry =
            CdFsFileEntry::new(&data_stream, 2048, 0, directory_record);
        let record: CdFsExtendedAttributeRecord = file_entry.read_extended_attribute_record()?;
        if let CdFsFileEntryData::Iso9660 {
            extended_attribute_record,
            ..
        } = &mut file_entry.data
        {
            *extended_attribute_record = Some(record);
        }

        assert_eq!(file_entry.get_owner_identifier(), Some(10));
        assert_eq!(file_entry.get_group_identifier(), Some(20));
        assert_eq!(file_entry.get_permissions(), Some(0x01ff));
        assert!(file_entry.get_creation_time().is_some());
        assert!(file_entry.get_modification_time().is_some());
        assert!(file_entry.get_expiration_time().is_some());
        assert!(file_entry.get_effective_time().is_some());

        // Test with zero data size.
        let mut empty_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        empty_record.extended_attribute_record_size = 78;
        let mut empty_entry: CdFsFileEntry =
            CdFsFileEntry::new(&data_stream, 2048, 0, empty_record);
        let result: Result<CdFsExtendedAttributeRecord, ErrorTrace> =
            empty_entry.read_extended_attribute_record();
        assert!(result.is_err());

        // Test with invalid extended attribute record data.
        let bad_stream: DataStreamReference = open_fake_data_stream(&[0xff; 2048]);
        let mut bad_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        bad_record.data_size = 2048;
        bad_record.extended_attribute_record_size = 78;
        let mut bad_entry: CdFsFileEntry = CdFsFileEntry::new(&bad_stream, 2048, 0, bad_record);
        let result: Result<CdFsExtendedAttributeRecord, ErrorTrace> =
            bad_entry.read_extended_attribute_record();
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_read_sub_directory_records_with_errors() {
        let empty_stream: DataStreamReference = open_fake_data_stream(&[]);

        // Test with zero data size on a directory entry.
        let mut zero_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        zero_record.file_flags = CDFS_FILE_FLAG_DIRECTORY;
        let mut zero_entry: CdFsFileEntry = CdFsFileEntry::new(&empty_stream, 2048, 0, zero_record);

        let result: Result<usize, ErrorTrace> = zero_entry.get_number_of_sub_file_entries();
        assert!(result.is_err());

        let result: Result<CdFsFileEntry, ErrorTrace> = zero_entry.get_sub_file_entry_by_index(0);
        assert!(result.is_err());

        let name: PathComponent = PathComponent::from("FILE");
        let result: Result<Option<CdFsFileEntry>, ErrorTrace> =
            zero_entry.get_sub_file_entry_by_name(&name);
        assert!(result.is_err());

        // Test with directory record size out of bounds.
        let mut out_of_bounds_data: Vec<u8> = vec![0; 34];
        out_of_bounds_data[0] = 0xff;
        let out_of_bounds_stream: DataStreamReference = open_fake_data_stream(&out_of_bounds_data);
        let mut out_of_bounds_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        out_of_bounds_record.file_flags = CDFS_FILE_FLAG_DIRECTORY;
        out_of_bounds_record.data_size = 34;
        let mut out_of_bounds_entry: CdFsFileEntry =
            CdFsFileEntry::new(&out_of_bounds_stream, 2048, 0, out_of_bounds_record);
        let result: Result<(), ErrorTrace> = out_of_bounds_entry.read_sub_directory_records();
        assert!(result.is_err());

        // Test with invalid directory record inside directory data.
        let mut invalid_directory_data: Vec<u8> = vec![0; 34];
        invalid_directory_data[0] = 34;
        invalid_directory_data[2] = 0xff;
        let invalid_directory_stream: DataStreamReference =
            open_fake_data_stream(&invalid_directory_data);
        let mut invalid_directory_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        invalid_directory_record.file_flags = CDFS_FILE_FLAG_DIRECTORY;
        invalid_directory_record.data_size = 34;
        let mut invalid_directory_entry: CdFsFileEntry =
            CdFsFileEntry::new(&invalid_directory_stream, 2048, 0, invalid_directory_record);
        let result: Result<(), ErrorTrace> = invalid_directory_entry.read_sub_directory_records();
        assert!(result.is_err());
    }

    #[test]
    fn test_extent_reader_and_multi_sector_directory() -> Result<(), ErrorTrace> {
        let backing: Vec<u8> = b"0123456789".to_vec();
        let data_stream: DataStreamReference = open_fake_data_stream(&backing);
        let mut reader: CdFsExtentReader = CdFsExtentReader::new(&data_stream, 2, 5);
        assert_eq!(reader.get_size(), 5);

        let mut empty: [u8; 0] = [];
        assert_eq!(reader.read_data_from_blocks(&mut empty, 0)?, 0);
        let mut buffer: [u8; 4] = [0; 4];
        assert_eq!(reader.read_data_from_blocks(&mut buffer, 5)?, 0);
        assert_eq!(reader.read_data_from_blocks(&mut buffer, 1)?, 4);
        assert_eq!(&buffer, b"3456");

        // Build a 2-sector directory with zero padding at end of sector 0 and a record in sector 1.
        let mut directory_data: Vec<u8> = vec![0; 4096];
        let mut record: Vec<u8> = vec![0; 38];
        record[0] = 38;
        record[2..6].copy_from_slice(&0u32.to_le_bytes());
        record[6..10].copy_from_slice(&0u32.to_be_bytes());
        record[10..14].copy_from_slice(&5u32.to_le_bytes());
        record[14..18].copy_from_slice(&5u32.to_be_bytes());
        record[28..30].copy_from_slice(&1u16.to_le_bytes());
        record[30..32].copy_from_slice(&1u16.to_be_bytes());
        record[32] = 5;
        record[33..38].copy_from_slice(b"A.TXT");
        directory_data[2048..2048 + record.len()].copy_from_slice(&record);

        let directory_stream: DataStreamReference = open_fake_data_stream(&directory_data);
        let mut root_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        root_record.file_flags = CDFS_FILE_FLAG_DIRECTORY;
        root_record.data_size = 4096;
        let mut root_entry: CdFsFileEntry =
            CdFsFileEntry::new(&directory_stream, 2048, 0, root_record);

        assert_eq!(root_entry.get_number_of_sub_file_entries()?, 1);
        let sub_entry: CdFsFileEntry = root_entry.get_sub_file_entry_by_index(0)?;
        assert_eq!(sub_entry.get_name().unwrap().to_string(), "A.TXT");

        Ok(())
    }

    #[test]
    fn test_rock_ridge_and_udf_getters_and_errors() -> Result<(), ErrorTrace> {
        let data_stream: DataStreamReference = open_fake_data_stream(&[]);

        // Test Rock Ridge getters for all timestamp fields and mode-only symbolic link detection.
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        attributes.file_mode = Some(0o120777);
        attributes.access_time = Some(CdFsDateTime::new(126, 9, 29, 1, 2, 3));
        attributes.change_time = Some(CdFsDateTime::new(126, 9, 29, 1, 2, 4));
        attributes.creation_time = Some(CdFsDateTime::new(126, 9, 29, 1, 2, 5));
        attributes.effective_time = Some(CdFsDateTime::new(126, 9, 29, 1, 2, 6));
        attributes.expiration_time = Some(CdFsDateTime::new(126, 9, 29, 1, 2, 7));

        let entry: CdFsFileEntry = CdFsFileEntry::new_with_options(
            &data_stream,
            2048,
            CharacterEncoding::Ascii,
            true,
            0,
            0,
            CdFsDirectoryRecord::new(),
            Some(attributes),
        );
        assert!(entry.get_access_time().is_some());
        assert!(entry.get_change_time().is_some());
        assert!(entry.get_creation_time().is_some());
        assert!(entry.get_effective_time().is_some());
        assert!(entry.get_expiration_time().is_some());
        assert!(entry.is_symbolic_link());
        assert_eq!(entry.get_device_number(), None);
        assert_eq!(entry.get_symbolic_link_target(), None);

        // Test plain ISO 9660 getters returning None / false.
        let plain_entry: CdFsFileEntry =
            CdFsFileEntry::new(&data_stream, 2048, 0, CdFsDirectoryRecord::new());
        assert_eq!(plain_entry.get_access_time(), None);
        assert_eq!(plain_entry.get_change_time(), None);
        assert_eq!(plain_entry.get_device_number(), None);
        assert_eq!(plain_entry.get_file_mode(), None);
        assert_eq!(plain_entry.get_number_of_links(), None);
        assert_eq!(plain_entry.get_posix_group_identifier(), None);
        assert_eq!(plain_entry.get_posix_owner_identifier(), None);
        assert_eq!(plain_entry.get_symbolic_link_target(), None);
        assert!(!plain_entry.is_symbolic_link());

        // Test UDF entry getters, sub-entry bounds error, missing name lookup, and XAR error.
        let udf_image: Vec<u8> = crate::cdfs::udf::tests::build_test_udf_image();
        let udf_stream: DataStreamReference = open_fake_data_stream(&udf_image);
        let mut volume: CdFsVolume = CdFsVolume::new();
        volume.read_data_stream(&udf_stream)?;
        let file_system: CdFsFileSystem = volume.get_file_system()?;
        let mut udf_root: CdFsFileEntry = file_system.get_root_file_entry()?;

        assert_eq!(udf_root.get_device_number(), None);
        assert_eq!(udf_root.get_effective_time(), None);
        assert_eq!(udf_root.get_expiration_time(), None);

        let result: Result<CdFsExtendedAttributeRecord, ErrorTrace> =
            udf_root.read_extended_attribute_record();
        assert!(result.is_err());

        let missing_name: PathComponent = PathComponent::from("missing.txt");
        let missing_entry: Option<CdFsFileEntry> =
            udf_root.get_sub_file_entry_by_name(&missing_name)?;
        assert!(missing_entry.is_none());

        let result: Result<CdFsFileEntry, ErrorTrace> = udf_root.get_sub_file_entry_by_index(99);
        assert!(result.is_err());

        // Test relocated directory target sector with invalid record size.
        let zero_sector_stream: DataStreamReference = open_fake_data_stream(&vec![0; 2048]);
        let result: Result<CdFsDirectoryRecord, ErrorTrace> =
            CdFsFileEntry::read_relocated_directory_record(
                &zero_sector_stream,
                2048,
                &CharacterEncoding::Ascii,
                0,
            );
        assert!(result.is_err());

        Ok(())
    }
}
