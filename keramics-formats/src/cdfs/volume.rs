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
use std::sync::Arc;

use keramics_core::{ByteOrder, DataStreamReference, ErrorTrace};
use keramics_encodings::CharacterEncoding;
use keramics_types::ByteString;

use super::constants::*;
use super::directory_record::CdFsDirectoryRecord;
use super::file_system::CdFsFileSystem;
use super::path_table::CdFsPathTable;
use super::rock_ridge::CdFsRockRidgeAttributes;
use super::udf::UdfVolume;
use super::volume_descriptor::CdFsVolumeDescriptor;

/// CD file system (CDFS) volume.
#[derive(Clone)]
pub struct CdFsVolume {
    /// Data stream.
    data_stream: Option<DataStreamReference>,

    /// Format version.
    format_version: u8,

    /// System identifier.
    system_identifier: Option<ByteString>,

    /// Volume identifier.
    volume_identifier: Option<ByteString>,

    /// Bytes per sector.
    bytes_per_sector: u16,

    /// Volume size.
    pub(super) volume_size: u64,

    /// Number of volumes in set.
    volumes_in_set: u16,

    /// Index of the volume in the set.
    volume_set_index: u16,

    /// Volume set identifier.
    volume_set_identifier: Option<ByteString>,

    /// Primary root directory record.
    root_directory_record: CdFsDirectoryRecord,

    /// Offset of the primary root directory record in the data stream.
    root_directory_record_offset: u64,

    /// Joliet UCS-2 level (1, 2, or 3), if a Joliet Supplementary Volume Descriptor is present.
    joliet_level: Option<u8>,

    /// Joliet system identifier (UTF-16BE).
    joliet_system_identifier: Option<ByteString>,

    /// Joliet volume identifier (UTF-16BE).
    joliet_volume_identifier: Option<ByteString>,

    /// Joliet volume set identifier (UTF-16BE).
    joliet_volume_set_identifier: Option<ByteString>,

    /// Joliet root directory record, if present.
    joliet_root_directory_record: Option<CdFsDirectoryRecord>,

    /// Offset of the Joliet root directory record in the data stream.
    joliet_root_directory_record_offset: u64,

    /// Value indicating whether Rock Ridge extensions were detected.
    has_rock_ridge: bool,

    /// SUSP skip length (`LEN_SKP`) from the root directory's `"SP"` entry.
    susp_skip_length: usize,

    /// UDF volume metadata, if present (pure UDF or ISO 9660 / UDF Bridge).
    udf_volume: Option<UdfVolume>,
}

impl CdFsVolume {
    /// Creates a new volume.
    pub fn new() -> Self {
        Self {
            data_stream: None,
            format_version: 0,
            system_identifier: None,
            volume_identifier: None,
            bytes_per_sector: 0,
            volume_size: 0,
            volumes_in_set: 0,
            volume_set_index: 0,
            volume_set_identifier: None,
            root_directory_record: CdFsDirectoryRecord::new(),
            root_directory_record_offset: 0,
            joliet_level: None,
            joliet_system_identifier: None,
            joliet_volume_identifier: None,
            joliet_volume_set_identifier: None,
            joliet_root_directory_record: None,
            joliet_root_directory_record_offset: 0,
            has_rock_ridge: false,
            susp_skip_length: 0,
            udf_volume: None,
        }
    }

    /// Retrieves the bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u16 {
        self.bytes_per_sector
    }

    /// Retrieves the character encoding used for ISO 9660 / Joliet directory records.
    pub(super) fn get_character_encoding(&self) -> CharacterEncoding {
        if self.joliet_level.is_some() && !self.has_rock_ridge {
            CharacterEncoding::Utf16BigEndian
        } else {
            CharacterEncoding::Ascii
        }
    }

    /// Retrieves the data stream.
    pub(super) fn get_data_stream(&self) -> Option<&DataStreamReference> {
        self.data_stream.as_ref()
    }

    /// Retrieves the file system.
    pub fn get_file_system(&self) -> Result<CdFsFileSystem, ErrorTrace> {
        if self.data_stream.is_none() {
            return Err(keramics_core::error_trace_new!("Missing data stream"));
        }
        let volumes: [Arc<CdFsVolume>; 1] = [Arc::new(self.clone())];

        Ok(CdFsFileSystem::new(&volumes))
    }

    /// Retrieves the format version.
    pub fn get_format_version(&self) -> u8 {
        self.format_version
    }

    /// Retrieves the Joliet UCS-2 level (1, 2, or 3), if present.
    pub fn get_joliet_level(&self) -> Option<u8> {
        self.joliet_level
    }

    /// Retrieves the Joliet volume identifier (UTF-16BE), if present.
    pub fn get_joliet_volume_identifier(&self) -> Option<&ByteString> {
        self.joliet_volume_identifier.as_ref()
    }

    /// Retrieves the active ISO 9660 or Joliet root directory record.
    pub(super) fn get_root_directory(&self) -> &CdFsDirectoryRecord {
        if !self.has_rock_ridge {
            if let Some(joliet_root) = &self.joliet_root_directory_record {
                return joliet_root;
            }
        }
        &self.root_directory_record
    }

    /// Retrieves the offset of the active ISO 9660 or Joliet root directory record.
    pub(super) fn get_root_directory_record_offset(&self) -> u64 {
        if !self.has_rock_ridge && self.joliet_root_directory_record.is_some() {
            self.joliet_root_directory_record_offset
        } else {
            self.root_directory_record_offset
        }
    }

    /// Retrieves the SUSP skip length (`LEN_SKP`).
    pub(super) fn get_susp_skip_length(&self) -> usize {
        self.susp_skip_length
    }

    /// Retrieves the system identifier.
    pub fn get_system_identifier(&self) -> Option<&ByteString> {
        self.system_identifier.as_ref()
    }

    /// Retrieves the UDF ECMA-167 NSR version (`2` for NSR02, `3` for NSR03), if present.
    pub fn get_udf_nsr_version(&self) -> Option<u8> {
        self.udf_volume.as_ref().map(|volume| volume.nsr_version)
    }

    /// Retrieves the UDF volume metadata, if present.
    pub(super) fn get_udf_volume(&self) -> Option<&UdfVolume> {
        self.udf_volume.as_ref()
    }

    /// Retrieves the volume identifier.
    pub fn get_volume_identifier(&self) -> Option<&ByteString> {
        self.volume_identifier.as_ref()
    }

    /// Retrieves the volume set identifier.
    pub fn get_volume_set_identifier(&self) -> Option<&ByteString> {
        self.volume_set_identifier.as_ref()
    }

    /// Retrieves the index of the volume in the set.
    pub fn get_volume_set_index(&self) -> u16 {
        self.volume_set_index
    }

    /// Retrieves the volume size in bytes.
    pub fn get_volume_size(&self) -> u64 {
        self.volume_size
    }

    /// Retrieves the number of volumes in set.
    pub fn get_volumes_in_set(&self) -> u16 {
        self.volumes_in_set
    }

    /// Determines if the volume has a Joliet Supplementary Volume Descriptor.
    pub fn has_joliet(&self) -> bool {
        self.joliet_level.is_some()
    }

    /// Determines if the volume has Rock Ridge extensions.
    pub fn has_rock_ridge(&self) -> bool {
        self.has_rock_ridge
    }

    /// Determines if the volume has a UDF file system (pure UDF or ISO 9660 / UDF Bridge).
    pub fn has_udf(&self) -> bool {
        self.udf_volume.is_some()
    }

    /// Probes the primary root directory's `"."` entry for SUSP and Rock Ridge extensions.
    fn probe_rock_ridge(&mut self, data_stream: &DataStreamReference) -> Result<(), ErrorTrace> {
        if self.root_directory_record.data_size < 34 {
            return Ok(());
        }
        let sector_offset: u64 =
            (self.root_directory_record.data_start_sector as u64) * (self.bytes_per_sector as u64);
        let stream_size: u64 = keramics_core::data_stream_get_size!(data_stream);
        let read_size: usize = self.bytes_per_sector as usize;

        if sector_offset + (read_size as u64) > stream_size {
            return Ok(());
        }
        let mut sector_data: Vec<u8> = vec![0; read_size];
        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut sector_data,
            SeekFrom::Start(sector_offset)
        );
        let record_size: usize = sector_data[0] as usize;
        if record_size < 34 || record_size > read_size {
            return Ok(());
        }
        let mut dot_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        if dot_record.read_data(&sector_data[..record_size]).is_err() {
            return Ok(());
        }
        if dot_record.system_use_data.is_empty() {
            return Ok(());
        }
        let mut attributes: CdFsRockRidgeAttributes = CdFsRockRidgeAttributes::new();
        if attributes
            .read_data(
                data_stream,
                self.bytes_per_sector,
                &dot_record.system_use_data,
                0,
            )
            .is_ok()
            && attributes.susp_detected
            && attributes.has_rock_ridge()
        {
            self.has_rock_ridge = true;
            self.susp_skip_length = attributes.susp_skip_length as usize;
        }
        Ok(())
    }

    /// Reads the volume from a data stream.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        let mut data: Vec<u8> = vec![0; 2048];
        let mut offset: u64 = 32768;

        self.bytes_per_sector = 2048;

        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut data,
            SeekFrom::Start(offset)
        );

        // Detect pure UDF disc (Volume Recognition Sequence starts directly with BEA01).
        if &data[1..6] == UDF_VRS_SIGNATURE_BEA01 {
            let nsr_version: u8 = match UdfVolume::detect_volume_recognition_sequence(
                data_stream,
                offset,
                self.bytes_per_sector as u64,
            ) {
                Ok(Some(version)) => version,
                Ok(None) => {
                    return Err(keramics_core::error_trace_new!(
                        "Missing UDF NSR volume structure descriptor"
                    ));
                }
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to detect UDF Volume Recognition Sequence"
                    );
                    return Err(error);
                }
            };
            let udf_volume: UdfVolume = match UdfVolume::read_data_stream(
                data_stream,
                nsr_version,
                self.bytes_per_sector as u64,
            ) {
                Ok(volume) => volume,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read UDF volume");
                    return Err(error);
                }
            };
            self.format_version = nsr_version;
            self.bytes_per_sector = udf_volume.block_size as u16;
            self.volume_size = keramics_core::data_stream_get_size!(data_stream);
            self.volumes_in_set = 1;
            self.volume_set_index = 1;
            self.volume_identifier = udf_volume
                .logical_volume_identifier
                .clone()
                .or_else(|| udf_volume.volume_identifier.clone());
            self.volume_set_identifier = udf_volume.volume_set_identifier.clone();
            self.udf_volume = Some(udf_volume);
            self.data_stream = Some(data_stream.clone());

            return Ok(());
        }

        let mut format_version: u8 = 0;
        let mut system_identifier: Option<ByteString> = None;
        let mut volume_identifier: Option<ByteString> = None;
        let mut volume_size: u64 = 0;
        let mut path_table_size: u32 = 0;
        let mut path_table_start_sector_be: u32 = 0;
        let mut path_table_start_sector_le: u32 = 0;
        let mut volumes_in_set: u16 = 0;
        let mut volume_set_index: u16 = 0;
        let mut volume_set_identifier: Option<ByteString> = None;
        let mut root_directory_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
        let mut root_directory_record_offset: u64 = 0;
        let mut root_directory_record_read: bool = false;

        let mut joliet_level: Option<u8> = None;
        let mut joliet_system_identifier: Option<ByteString> = None;
        let mut joliet_volume_identifier: Option<ByteString> = None;
        let mut joliet_volume_set_identifier: Option<ByteString> = None;
        let mut joliet_root_directory_record: Option<CdFsDirectoryRecord> = None;
        let mut joliet_root_directory_record_offset: u64 = 0;

        let mut first_sector: bool = true;

        loop {
            if !first_sector {
                keramics_core::data_stream_read_exact_at_position!(
                    data_stream,
                    &mut data,
                    SeekFrom::Start(offset)
                );
            }
            first_sector = false;

            if &data[0..6] == CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR {
                offset += self.bytes_per_sector as u64;
                break;
            }
            keramics_core::debug_trace_data_and_structure!(
                "CdFsVolumeDescriptor",
                offset,
                &data,
                2048,
                CdFsVolumeDescriptor::debug_read_data(&data)
            );
            let mut volume_descriptor: CdFsVolumeDescriptor = CdFsVolumeDescriptor::new();

            match volume_descriptor.read_data(&data) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read volume descriptor at offset: {} (0x{:08x})",
                            offset, offset
                        )
                    );
                    return Err(error);
                }
            }
            let descriptor_offset: u64 = offset;
            offset += self.bytes_per_sector as u64;

            if volume_descriptor.type_indicator == CDFS_VOLUME_DESCRIPTOR_TYPE_PRIMARY {
                format_version = volume_descriptor.format_version;
                volume_size =
                    (volume_descriptor.volume_size as u64) * (self.bytes_per_sector as u64);
                root_directory_record_offset = descriptor_offset + 156;
                system_identifier = CdFsVolumeDescriptor::read_trimmed_string(&data, 8, 32);
                volume_identifier = CdFsVolumeDescriptor::read_trimmed_string(&data, 40, 32);
                path_table_size = volume_descriptor.path_table_size;
                path_table_start_sector_be = volume_descriptor.path_table_start_sector_be;
                path_table_start_sector_le = volume_descriptor.path_table_start_sector_le;
                volumes_in_set = volume_descriptor.volumes_in_set;
                volume_set_index = volume_descriptor.volume_set_index;
                volume_set_identifier = CdFsVolumeDescriptor::read_trimmed_string(&data, 190, 128);
                match CdFsDirectoryRecord::read_data(&mut root_directory_record, &data[156..190]) {
                    Ok(_) => root_directory_record_read = true,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to read root directory record"
                        );
                        return Err(error);
                    }
                }
            } else if volume_descriptor.type_indicator == CDFS_VOLUME_DESCRIPTOR_TYPE_SUPPLEMENTARY
            {
                if let Some(level) = CdFsVolumeDescriptor::detect_joliet_level(&data) {
                    if joliet_level.map(|current| level > current).unwrap_or(true) {
                        let mut svd_root_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();
                        match svd_root_record.read_data_with_encoding(
                            &data[156..190],
                            &CharacterEncoding::Utf16BigEndian,
                        ) {
                            Ok(_) => {
                                joliet_level = Some(level);
                                joliet_system_identifier =
                                    CdFsVolumeDescriptor::read_trimmed_ucs2_string(&data, 8, 32);
                                joliet_volume_identifier =
                                    CdFsVolumeDescriptor::read_trimmed_ucs2_string(&data, 40, 32);
                                joliet_volume_set_identifier =
                                    CdFsVolumeDescriptor::read_trimmed_ucs2_string(&data, 190, 128);
                                joliet_root_directory_record = Some(svd_root_record);
                                joliet_root_directory_record_offset = descriptor_offset + 156;
                            }
                            Err(mut error) => {
                                keramics_core::error_trace_add_frame!(
                                    error,
                                    "Unable to read Joliet root directory record"
                                );
                                return Err(error);
                            }
                        }
                    }
                }
            }
        }
        if !root_directory_record_read {
            return Err(keramics_core::error_trace_new!(
                "Missing root directory record"
            ));
        }
        self.format_version = format_version;
        self.system_identifier = system_identifier;
        self.volume_identifier = volume_identifier;
        self.volume_size = volume_size;
        self.volumes_in_set = volumes_in_set;
        self.volume_set_index = volume_set_index;
        self.volume_set_identifier = volume_set_identifier;
        self.root_directory_record = root_directory_record;
        self.root_directory_record_offset = root_directory_record_offset;
        self.joliet_level = joliet_level;
        self.joliet_system_identifier = joliet_system_identifier;
        self.joliet_volume_identifier = joliet_volume_identifier;
        self.joliet_volume_set_identifier = joliet_volume_set_identifier;
        self.joliet_root_directory_record = joliet_root_directory_record;
        self.joliet_root_directory_record_offset = joliet_root_directory_record_offset;

        if path_table_size == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported path table size"
            ));
        }
        if path_table_start_sector_be == 0 {
            return Err(keramics_core::error_trace_new!(
                "Missing big-endian path table start sector"
            ));
        }
        let path_table_offset: u64 =
            (path_table_start_sector_be as u64) * (self.bytes_per_sector as u64);

        let mut path_table: CdFsPathTable = CdFsPathTable::new(ByteOrder::BigEndian);

        match path_table.read_at_position(
            data_stream,
            SeekFrom::Start(path_table_offset),
            path_table_size as usize,
        ) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read big-endian path table at offset: {} (0x{:08x})",
                        path_table_offset, path_table_offset
                    )
                );
                return Err(error);
            }
        }
        if path_table_start_sector_le == 0 {
            return Err(keramics_core::error_trace_new!(
                "Missing little-endian path table start sector"
            ));
        }
        let path_table_offset: u64 =
            (path_table_start_sector_le as u64) * (self.bytes_per_sector as u64);

        let mut path_table: CdFsPathTable = CdFsPathTable::new(ByteOrder::LittleEndian);

        match path_table.read_at_position(
            data_stream,
            SeekFrom::Start(path_table_offset),
            path_table_size as usize,
        ) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read little-endian path table at offset: {} (0x{:08x})",
                        path_table_offset, path_table_offset
                    )
                );
                return Err(error);
            }
        }
        // Probe for Rock Ridge extensions on the primary root directory.
        match self.probe_rock_ridge(data_stream) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to probe Rock Ridge extensions"
                );
                return Err(error);
            }
        }
        // Check for an ISO 9660 / UDF Bridge Volume Recognition Sequence after the terminator.
        if let Ok(Some(nsr_version)) = UdfVolume::detect_volume_recognition_sequence(
            data_stream,
            offset,
            self.bytes_per_sector as u64,
        ) {
            if let Ok(udf_volume) =
                UdfVolume::read_data_stream(data_stream, nsr_version, self.bytes_per_sector as u64)
            {
                self.udf_volume = Some(udf_volume);
            }
        }

        self.data_stream = Some(data_stream.clone());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::{open_fake_data_stream, open_os_data_stream};
    use keramics_encodings::CharacterEncoding;

    use crate::cdfs::file_entry::CdFsFileEntry;
    use crate::cdfs::udf::tests::build_test_udf_image;
    use crate::path::Path;
    use crate::tests::get_test_data_path;
    use crate::traits::FileEntryIterator;

    fn build_test_volume_data(
        volumes_in_set: u16,
        volume_set_index: u16,
        volume_set_identifier: &[u8],
    ) -> Vec<u8> {
        let mut data: Vec<u8> = vec![0; 21 * 2048];
        let pvd_offset: usize = 16 * 2048;

        // Primary volume descriptor at sector 16.
        data[pvd_offset] = 1;
        data[pvd_offset + 1..pvd_offset + 6].copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SIGNATURE);
        data[pvd_offset + 6] = 1;
        data[pvd_offset + 8..pvd_offset + 13].copy_from_slice(b"LINUX");
        data[pvd_offset + 40..pvd_offset + 45].copy_from_slice(b"CDROM");
        data[pvd_offset + 80..pvd_offset + 84].copy_from_slice(&21u32.to_le_bytes());
        data[pvd_offset + 84..pvd_offset + 88].copy_from_slice(&21u32.to_be_bytes());
        data[pvd_offset + 120..pvd_offset + 122].copy_from_slice(&volumes_in_set.to_le_bytes());
        data[pvd_offset + 122..pvd_offset + 124].copy_from_slice(&volumes_in_set.to_be_bytes());
        data[pvd_offset + 124..pvd_offset + 126].copy_from_slice(&volume_set_index.to_le_bytes());
        data[pvd_offset + 126..pvd_offset + 128].copy_from_slice(&volume_set_index.to_be_bytes());
        data[pvd_offset + 128..pvd_offset + 130].copy_from_slice(&2048u16.to_le_bytes());
        data[pvd_offset + 130..pvd_offset + 132].copy_from_slice(&2048u16.to_be_bytes());
        data[pvd_offset + 132..pvd_offset + 136].copy_from_slice(&10u32.to_le_bytes());
        data[pvd_offset + 136..pvd_offset + 140].copy_from_slice(&10u32.to_be_bytes());
        data[pvd_offset + 140..pvd_offset + 144].copy_from_slice(&18u32.to_le_bytes());
        data[pvd_offset + 148..pvd_offset + 152].copy_from_slice(&19u32.to_be_bytes());

        // Root directory record at PVD offset 156 (34 bytes).
        let root_offset: usize = pvd_offset + 156;
        data[root_offset] = 34;
        data[root_offset + 2..root_offset + 6].copy_from_slice(&20u32.to_le_bytes());
        data[root_offset + 6..root_offset + 10].copy_from_slice(&20u32.to_be_bytes());
        data[root_offset + 10..root_offset + 14].copy_from_slice(&2048u32.to_le_bytes());
        data[root_offset + 14..root_offset + 18].copy_from_slice(&2048u32.to_be_bytes());
        data[root_offset + 25] = CDFS_FILE_FLAG_DIRECTORY;
        data[root_offset + 28..root_offset + 30].copy_from_slice(&volume_set_index.to_le_bytes());
        data[root_offset + 30..root_offset + 32].copy_from_slice(&volume_set_index.to_be_bytes());
        data[root_offset + 32] = 1;

        if !volume_set_identifier.is_empty() {
            data[pvd_offset + 190..pvd_offset + 190 + volume_set_identifier.len()]
                .copy_from_slice(volume_set_identifier);
        }

        // Volume descriptor set terminator at sector 17.
        let terminator_offset: usize = 17 * 2048;
        data[terminator_offset..terminator_offset + 6]
            .copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR);
        data[terminator_offset + 6] = 1;

        // Little-endian path table at sector 18 (10 bytes).
        let little_endian_path_table_offset: usize = 18 * 2048;
        data[little_endian_path_table_offset..little_endian_path_table_offset + 10]
            .copy_from_slice(&[0x01, 0x00, 0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);

        // Big-endian path table at sector 19 (10 bytes).
        let big_endian_path_table_offset: usize = 19 * 2048;
        data[big_endian_path_table_offset..big_endian_path_table_offset + 10]
            .copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00]);

        data
    }

    fn build_directory_record(
        start_sector: u32,
        data_size: u32,
        file_flags: u8,
        name: &[u8],
        system_use: &[u8],
    ) -> Vec<u8> {
        let base_end: usize = 33 + name.len();
        let pad_len: usize = base_end & 1;
        let total_len: usize = base_end + pad_len + system_use.len();
        let mut record: Vec<u8> = vec![0; total_len];

        record[0] = total_len as u8;
        record[2..6].copy_from_slice(&start_sector.to_le_bytes());
        record[6..10].copy_from_slice(&start_sector.to_be_bytes());
        record[10..14].copy_from_slice(&data_size.to_le_bytes());
        record[14..18].copy_from_slice(&data_size.to_be_bytes());
        record[18..25].copy_from_slice(&[126, 9, 29, 10, 20, 30, 0]);
        record[25] = file_flags;
        record[28..30].copy_from_slice(&1u16.to_le_bytes());
        record[30..32].copy_from_slice(&1u16.to_be_bytes());
        record[32] = name.len() as u8;
        record[33..base_end].copy_from_slice(name);
        if !system_use.is_empty() {
            record[base_end + pad_len..total_len].copy_from_slice(system_use);
        }
        record
    }

    fn extend_both_u32(data: &mut Vec<u8>, value: u32) {
        data.extend_from_slice(&value.to_le_bytes());
        data.extend_from_slice(&value.to_be_bytes());
    }

    fn encode_utf16_be(text: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        for code_unit in text.encode_utf16() {
            bytes.extend_from_slice(&code_unit.to_be_bytes());
        }
        bytes
    }

    fn get_volume() -> Result<CdFsVolume, ErrorTrace> {
        let mut volume: CdFsVolume = CdFsVolume::new();

        let path_string: String = get_test_data_path("cdfs/level3.iso");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        volume.read_data_stream(&data_stream)?;

        Ok(volume)
    }

    #[test]
    fn test_get_volumes_in_set() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        let volumes_in_set: u16 = volume.get_volumes_in_set();
        assert_eq!(volumes_in_set, 1);

        Ok(())
    }

    #[test]
    fn test_get_volume_set_index() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        let volume_set_index: u16 = volume.get_volume_set_index();
        assert_eq!(volume_set_index, 1);

        Ok(())
    }

    #[test]
    fn test_get_volume_set_identifier() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        // The test volume set identifier is empty, hence not set.
        assert_eq!(volume.get_volume_set_identifier(), None);

        Ok(())
    }

    #[test]
    fn test_get_format_version() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        let format_version: u8 = volume.get_format_version();
        assert_eq!(format_version, 1);

        Ok(())
    }

    #[test]
    fn test_get_system_identifier() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        let system_identifier: Option<&ByteString> = volume.get_system_identifier();
        let system_identifier: &ByteString = system_identifier
            .ok_or_else(|| keramics_core::error_trace_new!("Missing system identifier"))?;
        assert_eq!(system_identifier.encoding, CharacterEncoding::Ascii);
        assert_eq!(system_identifier.elements.as_slice(), b"LINUX");

        Ok(())
    }

    #[test]
    fn test_get_volume_identifier() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        let volume_identifier: Option<&ByteString> = volume.get_volume_identifier();
        let volume_identifier: &ByteString = volume_identifier
            .ok_or_else(|| keramics_core::error_trace_new!("Missing volume identifier"))?;
        assert_eq!(volume_identifier.encoding, CharacterEncoding::Ascii);
        assert_eq!(volume_identifier.elements.as_slice(), b"CDROM");

        Ok(())
    }

    #[test]
    fn test_get_volume_size() -> Result<(), ErrorTrace> {
        let volume: CdFsVolume = get_volume()?;

        let volume_size: u64 = volume.get_volume_size();
        assert_eq!(volume_size, 2482176);

        Ok(())
    }

    #[test]
    fn test_read_data_stream() -> Result<(), ErrorTrace> {
        keramics_core::mediator::Mediator { debug_output: true }.make_current();

        let mut volume: CdFsVolume = CdFsVolume::new();

        let path_string: String = get_test_data_path("cdfs/level3.iso");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        volume.read_data_stream(&data_stream)?;

        assert_eq!(volume.get_bytes_per_sector(), 2048);
        assert!(!volume.has_joliet());
        assert_eq!(volume.get_joliet_level(), None);
        assert!(!volume.has_rock_ridge());
        assert!(!volume.has_udf());
        assert_eq!(volume.get_udf_nsr_version(), None);

        Ok(())
    }

    #[test]
    fn test_get_file_system() -> Result<(), ErrorTrace> {
        let unread_volume: CdFsVolume = CdFsVolume::new();
        let result: Result<CdFsFileSystem, ErrorTrace> = unread_volume.get_file_system();
        assert!(result.is_err());

        let volume: CdFsVolume = get_volume()?;
        _ = volume.get_file_system()?;

        Ok(())
    }

    #[test]
    fn test_joliet_volume_and_file_system() -> Result<(), ErrorTrace> {
        // Build an ISO 9660 + Joliet Level 3 image:
        //  16: PVD, 17: Joliet SVD (root at sector 21), 18: Terminator,
        //  19: LE path table, 20: BE path table, 21: Joliet root directory, 22: file data.
        let mut data: Vec<u8> = vec![0; 24 * 2048];
        let base_data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[16 * 2048..17 * 2048].copy_from_slice(&base_data[16 * 2048..17 * 2048]);
        // Update PVD path tables to sectors 19 and 20, volume size to 24.
        let primary_offset: usize = 16 * 2048;
        data[primary_offset + 80..primary_offset + 84].copy_from_slice(&24u32.to_le_bytes());
        data[primary_offset + 84..primary_offset + 88].copy_from_slice(&24u32.to_be_bytes());
        data[primary_offset + 140..primary_offset + 144].copy_from_slice(&19u32.to_le_bytes());
        data[primary_offset + 148..primary_offset + 152].copy_from_slice(&20u32.to_be_bytes());

        // Joliet Level 3 SVD at sector 17.
        let supplementary_offset: usize = 17 * 2048;
        data[supplementary_offset] = CDFS_VOLUME_DESCRIPTOR_TYPE_SUPPLEMENTARY;
        data[supplementary_offset + 1..supplementary_offset + 6]
            .copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SIGNATURE);
        data[supplementary_offset + 6] = 1;
        let volume_identifier_utf16: Vec<u8> = encode_utf16_be("JolietDisc");
        data[supplementary_offset + 40..supplementary_offset + 40 + volume_identifier_utf16.len()]
            .copy_from_slice(&volume_identifier_utf16);
        data[supplementary_offset + 80..supplementary_offset + 84]
            .copy_from_slice(&24u32.to_le_bytes());
        data[supplementary_offset + 84..supplementary_offset + 88]
            .copy_from_slice(&24u32.to_be_bytes());
        data[supplementary_offset + 88..supplementary_offset + 91]
            .copy_from_slice(CDFS_JOLIET_ESCAPE_SEQUENCE_LEVEL3);
        data[supplementary_offset + 120..supplementary_offset + 122]
            .copy_from_slice(&1u16.to_le_bytes());
        data[supplementary_offset + 122..supplementary_offset + 124]
            .copy_from_slice(&1u16.to_be_bytes());
        data[supplementary_offset + 124..supplementary_offset + 126]
            .copy_from_slice(&1u16.to_le_bytes());
        data[supplementary_offset + 126..supplementary_offset + 128]
            .copy_from_slice(&1u16.to_be_bytes());
        data[supplementary_offset + 128..supplementary_offset + 130]
            .copy_from_slice(&2048u16.to_le_bytes());
        data[supplementary_offset + 130..supplementary_offset + 132]
            .copy_from_slice(&2048u16.to_be_bytes());
        data[supplementary_offset + 132..supplementary_offset + 136]
            .copy_from_slice(&10u32.to_le_bytes());
        data[supplementary_offset + 136..supplementary_offset + 140]
            .copy_from_slice(&10u32.to_be_bytes());
        let supplementary_root: Vec<u8> =
            build_directory_record(21, 2048, CDFS_FILE_FLAG_DIRECTORY, &[0], &[]);
        data[supplementary_offset + 156..supplementary_offset + 156 + supplementary_root.len()]
            .copy_from_slice(&supplementary_root);

        // Terminator at sector 18.
        let terminator_offset: usize = 18 * 2048;
        data[terminator_offset..terminator_offset + 6]
            .copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR);
        data[terminator_offset + 6] = 1;

        // Path tables at sectors 19 and 20.
        data[19 * 2048..19 * 2048 + 10]
            .copy_from_slice(&[0x01, 0x00, 0x15, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
        data[20 * 2048..20 * 2048 + 10]
            .copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x15, 0x00, 0x01, 0x00, 0x00]);

        // Joliet root directory at sector 21 with ".", "..", and UTF-16BE file "Test\u{00e9} File.txt;1".
        let payload: &[u8] = b"Joliet payload!";
        data[22 * 2048..22 * 2048 + payload.len()].copy_from_slice(payload);

        let mut directory_data: Vec<u8> = Vec::new();
        directory_data.extend_from_slice(&build_directory_record(
            21,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            &[0],
            &[],
        ));
        directory_data.extend_from_slice(&build_directory_record(
            21,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            &[1],
            &[],
        ));
        let file_name_utf16: Vec<u8> = encode_utf16_be("Test\u{00e9} File.txt;1");
        directory_data.extend_from_slice(&build_directory_record(
            22,
            payload.len() as u32,
            0,
            &file_name_utf16,
            &[],
        ));
        data[21 * 2048..21 * 2048 + directory_data.len()].copy_from_slice(&directory_data);

        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        volume.read_data_stream(&data_stream)?;

        assert!(volume.has_joliet());
        assert_eq!(volume.get_joliet_level(), Some(3));
        assert_eq!(
            volume.get_joliet_volume_identifier().unwrap().to_string(),
            "JolietDisc"
        );

        let file_system: CdFsFileSystem = volume.get_file_system()?;
        let mut root: CdFsFileEntry = file_system.get_root_file_entry()?;
        assert_eq!(root.get_number_of_sub_file_entries()?, 1);

        let path: Path = Path::from("/test\u{00e9} file.txt");
        let file_entry: CdFsFileEntry = file_system.get_file_entry_by_path(&path)?.unwrap();
        assert_eq!(
            file_entry.get_name().unwrap().to_string(),
            "Test\u{00e9} File.txt;1"
        );
        assert_eq!(file_entry.get_size(), payload.len() as u64);

        let file_stream: DataStreamReference = file_entry.get_data_stream()?.unwrap();
        let mut content: Vec<u8> = vec![0; payload.len()];
        keramics_core::data_stream_read_exact_at_position!(
            &file_stream,
            &mut content,
            SeekFrom::Start(0)
        );
        assert_eq!(content, payload);

        Ok(())
    }

    #[test]
    fn test_rock_ridge_volume_and_file_system() -> Result<(), ErrorTrace> {
        // Build an ISO 9660 + Rock Ridge image:
        //  16: PVD (root at sector 20), 17: Terminator, 18: LE PT, 19: BE PT,
        //  20: Root dir (SP+ER on ".", file with NM/PX/PN/TF, symlink with SL,
        //      relocated placeholder with CL -> sector 22, hidden RR_MOVED with RE),
        //  21: File payload,
        //  22: Relocated directory contents ("deep.txt").
        let mut data: Vec<u8> = vec![0; 24 * 2048];
        let base: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[0..base.len()].copy_from_slice(&base);
        data[16 * 2048 + 80..16 * 2048 + 84].copy_from_slice(&24u32.to_le_bytes());
        data[16 * 2048 + 84..16 * 2048 + 88].copy_from_slice(&24u32.to_be_bytes());

        let payload: &[u8] = b"Rock Ridge data";
        data[21 * 2048..21 * 2048 + payload.len()].copy_from_slice(payload);

        // Root "." System Use: SP + ER("IEEE_P1282").
        let mut root_system_use: Vec<u8> = vec![b'S', b'P', 7, 1, 0xbe, 0xef, 0];
        let extension_identifier: &[u8] = b"IEEE_P1282";
        root_system_use.extend_from_slice(&[
            b'E',
            b'R',
            (8 + extension_identifier.len()) as u8,
            1,
            extension_identifier.len() as u8,
            0,
            0,
            1,
        ]);
        root_system_use.extend_from_slice(extension_identifier);

        // File System Use: NM("MixedCase.txt") + PX(mode=0100755, links=1, uid=1000, gid=1001) + device(1, 3) + TF(modify).
        let mut file_system_use: Vec<u8> = Vec::new();
        file_system_use.extend_from_slice(&[b'N', b'M', 5 + 13, 1, 0x00]);
        file_system_use.extend_from_slice(b"MixedCase.txt");
        file_system_use.extend_from_slice(&[b'P', b'X', 36, 1]);
        extend_both_u32(&mut file_system_use, 0o100755);
        extend_both_u32(&mut file_system_use, 1);
        extend_both_u32(&mut file_system_use, 1000);
        extend_both_u32(&mut file_system_use, 1001);
        file_system_use.extend_from_slice(&[0x50, 0x4e, 20, 1]);
        extend_both_u32(&mut file_system_use, 1);
        extend_both_u32(&mut file_system_use, 3);
        file_system_use.extend_from_slice(&[
            b'T',
            b'F',
            5 + 7,
            1,
            CDFS_RRIP_TIME_STAMP_FLAG_MODIFY,
        ]);
        file_system_use.extend_from_slice(&[126, 9, 29, 12, 0, 0, 0]);

        // Symbolic link System Use: NM("my_link") + PX(0120777) + SL("MixedCase.txt").
        let mut symbolic_link_system_use: Vec<u8> = Vec::new();
        symbolic_link_system_use.extend_from_slice(&[b'N', b'M', 5 + 7, 1, 0x00]);
        symbolic_link_system_use.extend_from_slice(b"my_link");
        symbolic_link_system_use.extend_from_slice(&[b'P', b'X', 36, 1]);
        extend_both_u32(&mut symbolic_link_system_use, 0o120777);
        extend_both_u32(&mut symbolic_link_system_use, 1);
        extend_both_u32(&mut symbolic_link_system_use, 0);
        extend_both_u32(&mut symbolic_link_system_use, 0);
        symbolic_link_system_use.extend_from_slice(&[b'S', b'L', 5 + 2 + 13, 1, 0x00, 0x00, 13]);
        symbolic_link_system_use.extend_from_slice(b"MixedCase.txt");

        // Relocated directory placeholder with CL -> sector 22 and NM("deep_dir").
        let mut child_link_system_use: Vec<u8> = Vec::new();
        child_link_system_use.extend_from_slice(&[b'N', b'M', 5 + 8, 1, 0x00]);
        child_link_system_use.extend_from_slice(b"deep_dir");
        child_link_system_use.extend_from_slice(&[b'C', b'L', 12, 1]);
        extend_both_u32(&mut child_link_system_use, 22);

        // Hidden RR_MOVED directory with RE flag.
        let relocated_system_use: Vec<u8> = vec![b'R', b'E', 4, 1];

        let mut root_directory: Vec<u8> = Vec::new();
        root_directory.extend_from_slice(&build_directory_record(
            20,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            &[0],
            &root_system_use,
        ));
        root_directory.extend_from_slice(&build_directory_record(
            20,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            &[1],
            &[],
        ));
        root_directory.extend_from_slice(&build_directory_record(
            21,
            payload.len() as u32,
            0,
            b"MIXEDCAS.TXT;1",
            &file_system_use,
        ));
        root_directory.extend_from_slice(&build_directory_record(
            0,
            0,
            0,
            b"MY_LINK.;1",
            &symbolic_link_system_use,
        ));
        root_directory.extend_from_slice(&build_directory_record(
            0,
            0,
            0,
            b"DEEP_DIR.;1",
            &child_link_system_use,
        ));
        root_directory.extend_from_slice(&build_directory_record(
            22,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            b"RR_MOVED",
            &relocated_system_use,
        ));
        data[20 * 2048..20 * 2048 + root_directory.len()].copy_from_slice(&root_directory);

        // Relocated directory at sector 22 containing "nested.txt".
        let mut nested_system_use: Vec<u8> = vec![b'N', b'M', 5 + 10, 1, 0x00];
        nested_system_use.extend_from_slice(b"nested.txt");
        let mut deep_directory: Vec<u8> = Vec::new();
        deep_directory.extend_from_slice(&build_directory_record(
            22,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            &[0],
            &[],
        ));
        deep_directory.extend_from_slice(&build_directory_record(
            20,
            2048,
            CDFS_FILE_FLAG_DIRECTORY,
            &[1],
            &[],
        ));
        deep_directory.extend_from_slice(&build_directory_record(
            21,
            payload.len() as u32,
            0,
            b"NESTED.TXT;1",
            &nested_system_use,
        ));
        data[22 * 2048..22 * 2048 + deep_directory.len()].copy_from_slice(&deep_directory);

        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        volume.read_data_stream(&data_stream)?;

        assert!(volume.has_rock_ridge());

        let file_system: CdFsFileSystem = volume.get_file_system()?;
        let mut root: CdFsFileEntry = file_system.get_root_file_entry()?;
        // RR_MOVED is hidden via RE, leaving MixedCase.txt, my_link, and deep_dir.
        assert_eq!(root.get_number_of_sub_file_entries()?, 3);

        let file_entry: CdFsFileEntry = file_system
            .get_file_entry_by_path(&Path::from("/MixedCase.txt"))?
            .unwrap();
        assert_eq!(file_entry.get_name().unwrap().to_string(), "MixedCase.txt");
        assert_eq!(file_entry.get_file_mode(), Some(0o100755));
        assert_eq!(file_entry.get_number_of_links(), Some(1));
        assert_eq!(file_entry.get_posix_owner_identifier(), Some(1000));
        assert_eq!(file_entry.get_posix_group_identifier(), Some(1001));
        assert_eq!(file_entry.get_owner_identifier(), Some(1000));
        assert_eq!(file_entry.get_group_identifier(), Some(1001));
        assert_eq!(file_entry.get_device_number(), Some((1, 3)));
        assert_eq!(file_entry.get_modification_time().unwrap().year, 2026);
        assert!(!file_entry.is_symbolic_link());

        let symbolic_link_entry: CdFsFileEntry = file_system
            .get_file_entry_by_path(&Path::from("/my_link"))?
            .unwrap();
        assert!(symbolic_link_entry.is_symbolic_link());
        assert_eq!(
            symbolic_link_entry
                .get_symbolic_link_target()
                .unwrap()
                .to_string(),
            "MixedCase.txt"
        );
        assert!(symbolic_link_entry.get_data_stream()?.is_none());

        let nested_entry: CdFsFileEntry = file_system
            .get_file_entry_by_path(&Path::from("/deep_dir/nested.txt"))?
            .unwrap();
        assert_eq!(nested_entry.get_size(), payload.len() as u64);

        Ok(())
    }

    #[test]
    fn test_pure_udf_and_bridge_volume() -> Result<(), ErrorTrace> {
        // 1. Pure UDF image opened directly through CdFsVolume -> CdFsFileSystem -> CdFsFileEntry.
        let udf_image: Vec<u8> = build_test_udf_image();
        let udf_stream: DataStreamReference = open_fake_data_stream(&udf_image);
        let mut volume: CdFsVolume = CdFsVolume::new();
        volume.read_data_stream(&udf_stream)?;

        assert!(volume.has_udf());
        assert_eq!(volume.get_udf_nsr_version(), Some(3));
        assert_eq!(volume.get_bytes_per_sector(), 2048);
        assert_eq!(
            volume.get_volume_identifier().unwrap().to_string(),
            "UDF_LOGVOL"
        );
        assert_eq!(
            volume.get_volume_set_identifier().unwrap().to_string(),
            "UDF_SET"
        );

        let file_system: CdFsFileSystem = volume.get_file_system()?;
        let mut root: CdFsFileEntry = file_system.get_root_file_entry()?;
        assert!(root.is_directory());
        assert!(root.is_root_directory());
        assert_eq!(root.get_number_of_sub_file_entries()?, 2);
        assert!(root.get_access_time().is_some());
        assert!(root.get_change_time().is_some());
        assert!(root.get_creation_time().is_some());
        assert!(root.get_modification_time().is_some());

        let file_entry: CdFsFileEntry = file_system
            .get_file_entry_by_path(&Path::from("/hello.txt"))?
            .unwrap();
        assert_eq!(file_entry.get_size(), 10);
        assert_eq!(file_entry.get_posix_owner_identifier(), Some(500));
        assert_eq!(file_entry.get_posix_group_identifier(), Some(501));
        assert_eq!(file_entry.get_owner_identifier(), Some(500));
        assert_eq!(file_entry.get_group_identifier(), Some(501));
        assert_eq!(file_entry.get_number_of_links(), Some(1));
        assert!(file_entry.get_file_mode().is_some());
        assert!(file_entry.get_permissions().is_some());

        let file_by_id: CdFsFileEntry =
            file_system.get_file_entry_by_identifier(file_entry.get_identifier())?;
        assert_eq!(file_by_id.get_name(), file_entry.get_name());

        let file_stream: DataStreamReference = file_entry.get_data_stream()?.unwrap();
        let mut content: Vec<u8> = vec![0; 10];
        keramics_core::data_stream_read_exact_at_position!(
            &file_stream,
            &mut content,
            SeekFrom::Start(0)
        );
        assert_eq!(content, b"Hello UDF!");

        let link_entry: CdFsFileEntry = file_system
            .get_file_entry_by_path(&Path::from("/link"))?
            .unwrap();
        assert!(link_entry.is_symbolic_link());
        assert_eq!(
            link_entry.get_symbolic_link_target().unwrap().to_string(),
            "/target"
        );
        assert!(link_entry.get_data_stream()?.is_none());

        // 2. ISO 9660 / UDF Bridge image (PVD at 16, terminator at 17, VRS at 18..20, PT at 21..22).
        let mut bridge: Vec<u8> = build_test_udf_image();
        let iso_base: Vec<u8> = build_test_volume_data(1, 1, b"BRIDGE");
        bridge[16 * 2048..17 * 2048].copy_from_slice(&iso_base[16 * 2048..17 * 2048]);
        bridge[16 * 2048 + 140..16 * 2048 + 144].copy_from_slice(&21u32.to_le_bytes());
        bridge[16 * 2048 + 148..16 * 2048 + 152].copy_from_slice(&22u32.to_be_bytes());
        bridge[17 * 2048..18 * 2048].copy_from_slice(&iso_base[17 * 2048..18 * 2048]);
        // VRS at sectors 18..20.
        bridge[18 * 2048..21 * 2048].fill(0);
        bridge[18 * 2048 + 1..18 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_BEA01);
        bridge[18 * 2048 + 6] = 1;
        bridge[19 * 2048 + 1..19 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_NSR03);
        bridge[19 * 2048 + 6] = 1;
        bridge[20 * 2048 + 1..20 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_TEA01);
        bridge[20 * 2048 + 6] = 1;
        // Path tables at sectors 21 and 22.
        bridge[21 * 2048..21 * 2048 + 10]
            .copy_from_slice(&[0x01, 0x00, 0x17, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
        bridge[22 * 2048..22 * 2048 + 10]
            .copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x17, 0x00, 0x01, 0x00, 0x00]);

        let bridge_stream: DataStreamReference = open_fake_data_stream(&bridge);
        let mut bridge_volume: CdFsVolume = CdFsVolume::new();
        bridge_volume.read_data_stream(&bridge_stream)?;

        assert!(bridge_volume.has_udf());
        let bridge_file_system: CdFsFileSystem = bridge_volume.get_file_system()?;
        let bridge_file: CdFsFileEntry = bridge_file_system
            .get_file_entry_by_path(&Path::from("/hello.txt"))?
            .unwrap();
        assert_eq!(bridge_file.get_size(), 10);

        Ok(())
    }

    #[test]
    fn test_read_data_stream_with_errors() {
        let pvd_offset: usize = 16 * 2048;

        // Test with invalid volume descriptor signature.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"VOLSET");
        data[pvd_offset + 1] = 0xff;
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with missing primary volume descriptor (immediate terminator).
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[pvd_offset..pvd_offset + 6].copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR);
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with invalid root directory record.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[pvd_offset + 156 + 2] = 0xff;
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with zero path table size.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[pvd_offset + 132..pvd_offset + 140].fill(0);
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with missing big-endian path table start sector.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[pvd_offset + 148..pvd_offset + 152].fill(0);
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with invalid big-endian path table record.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[19 * 2048] = 0x80;
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with missing little-endian path table start sector.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[pvd_offset + 140..pvd_offset + 144].fill(0);
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test with invalid little-endian path table record.
        let mut data: Vec<u8> = build_test_volume_data(1, 1, b"");
        data[18 * 2048] = 0x80;
        let data_stream: DataStreamReference = open_fake_data_stream(&data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&data_stream);
        assert!(result.is_err());

        // Test pure UDF VRS missing NSR descriptor.
        let mut invalid_udf_data: Vec<u8> = vec![0; 20 * 2048];
        invalid_udf_data[16 * 2048 + 1..16 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_BEA01);
        invalid_udf_data[17 * 2048 + 1..17 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_TEA01);
        let invalid_udf_stream: DataStreamReference = open_fake_data_stream(&invalid_udf_data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&invalid_udf_stream);
        assert!(result.is_err());

        // Test pure UDF VRS with NSR03 but missing AVDP.
        let mut missing_avdp_data: Vec<u8> = vec![0; 20 * 2048];
        missing_avdp_data[16 * 2048 + 1..16 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_BEA01);
        missing_avdp_data[17 * 2048 + 1..17 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_NSR03);
        missing_avdp_data[18 * 2048 + 1..18 * 2048 + 6].copy_from_slice(UDF_VRS_SIGNATURE_TEA01);
        let missing_avdp_stream: DataStreamReference = open_fake_data_stream(&missing_avdp_data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&missing_avdp_stream);
        assert!(result.is_err());

        // Test Joliet SVD with invalid root directory record.
        let mut invalid_joliet_data: Vec<u8> = vec![0; 22 * 2048];
        let base_data: Vec<u8> = build_test_volume_data(1, 1, b"");
        invalid_joliet_data[16 * 2048..17 * 2048].copy_from_slice(&base_data[16 * 2048..17 * 2048]);
        let supplementary_offset: usize = 17 * 2048;
        invalid_joliet_data[supplementary_offset] = CDFS_VOLUME_DESCRIPTOR_TYPE_SUPPLEMENTARY;
        invalid_joliet_data[supplementary_offset + 1..supplementary_offset + 6]
            .copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SIGNATURE);
        invalid_joliet_data[supplementary_offset + 6] = 1;
        invalid_joliet_data[supplementary_offset + 88..supplementary_offset + 91]
            .copy_from_slice(CDFS_JOLIET_ESCAPE_SEQUENCE_LEVEL3);
        invalid_joliet_data[supplementary_offset + 156 + 2] = 0xff;
        let invalid_joliet_stream: DataStreamReference =
            open_fake_data_stream(&invalid_joliet_data);
        let mut volume: CdFsVolume = CdFsVolume::new();
        let result: Result<(), ErrorTrace> = volume.read_data_stream(&invalid_joliet_stream);
        assert!(result.is_err());
    }
}
