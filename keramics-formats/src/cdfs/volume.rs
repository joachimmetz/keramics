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

use keramics_core::{ByteOrder, DataStreamReference, ErrorTrace};

use super::constants::*;
use super::path_table::CdFsPathTable;
use super::volume_descriptor::CdFsVolumeDescriptor;

/// CD file system (CDFS) volume.
pub struct CdFsVolume {
    /// Data stream.
    data_stream: Option<DataStreamReference>,

    /// Bytes per sector.
    bytes_per_sector: u16,
}

impl CdFsVolume {
    /// Creates a new volume.
    pub fn new() -> Self {
        Self {
            data_stream: None,
            bytes_per_sector: 0,
        }
    }

    /// Reads the volume from a data stream.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        // TODO: refactor into read_volume_descriptor_set
        let mut data: Vec<u8> = vec![0; 2048];
        let mut offset: u64 = 32768;

        let mut path_table_size: u32 = 0;
        let mut path_table_start_sector_be: u32 = 0;
        let mut path_table_start_sector_le: u32 = 0;

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
                path_table_size = volume_descriptor.path_table_size;
                path_table_start_sector_be = volume_descriptor.path_table_start_sector_be;
                path_table_start_sector_le = volume_descriptor.path_table_start_sector_le;
            }
            // TODO: compare volume set identifier
            // TODO: check if volume is part of a set
        }
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

    use keramics_core::open_os_data_stream;

    use crate::tests::get_test_data_path;

    #[test]
    fn test_read_data_stream() -> Result<(), ErrorTrace> {
        keramics_core::mediator::Mediator { debug_output: true }.make_current();

        let mut volume: CdFsVolume = CdFsVolume::new();

        let path_string: String = get_test_data_path("cdfs/level3.iso");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        volume.read_data_stream(&data_stream)?;

        // assert_eq!(volume.format_version, 1);

        Ok(())
    }
}
