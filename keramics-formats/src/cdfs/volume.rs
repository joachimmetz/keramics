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
use keramics_types::ByteString;

use super::constants::*;
use super::directory_record::CdFsDirectoryRecord;
use super::file_system::CdFsFileSystem;
use super::path_table::CdFsPathTable;
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

    /// Root directory.
    root_directory_record: CdFsDirectoryRecord,

    /// Offset of the root directory record in the data stream.
    root_directory_record_offset: u64,
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
        }
    }

    /// Retrieves the bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u16 {
        self.bytes_per_sector
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

    /// Retrieves the root directory.
    pub(super) fn get_root_directory(&self) -> &CdFsDirectoryRecord {
        &self.root_directory_record
    }

    /// Retrieves the offset of the root directory record in the data stream.
    pub(super) fn get_root_directory_record_offset(&self) -> u64 {
        self.root_directory_record_offset
    }

    /// Retrieves the system identifier.
    pub fn get_system_identifier(&self) -> Option<&ByteString> {
        self.system_identifier.as_ref()
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

    /// Reads the volume from a data stream.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        // TODO: refactor into read_volume_descriptor_set
        let mut data: Vec<u8> = vec![0; 2048];
        let mut offset: u64 = 32768;

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

        self.bytes_per_sector = 2048;

        loop {
            keramics_core::data_stream_read_exact_at_position!(
                data_stream,
                &mut data,
                SeekFrom::Start(offset)
            );
            if &data[0..6] == CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR {
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
            offset += self.bytes_per_sector as u64;

            if volume_descriptor.type_indicator == 1 {
                format_version = volume_descriptor.format_version;
                volume_size =
                    (volume_descriptor.volume_size as u64) * (self.bytes_per_sector as u64);
                root_directory_record_offset = offset - self.bytes_per_sector as u64 + 156;
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
        // TODO: compare path tables

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

    use crate::tests::get_test_data_path;

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
        let term_offset: usize = 17 * 2048;
        data[term_offset..term_offset + 6].copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR);
        data[term_offset + 6] = 1;

        // Little-endian path table at sector 18 (10 bytes).
        let le_pt_offset: usize = 18 * 2048;
        data[le_pt_offset..le_pt_offset + 10]
            .copy_from_slice(&[0x01, 0x00, 0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);

        // Big-endian path table at sector 19 (10 bytes).
        let be_pt_offset: usize = 19 * 2048;
        data[be_pt_offset..be_pt_offset + 10]
            .copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00]);

        data
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
    }
}
