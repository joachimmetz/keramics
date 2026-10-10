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

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_formats::cdfs::CdFsVolume;

/// Information about CD file system (CDFS) format.
pub struct CdFsInfo {}

impl CdFsInfo {
    /// Opens a volume.
    pub fn open_volume(data_stream: &DataStreamReference) -> Result<CdFsVolume, ErrorTrace> {
        let mut cdfs_volume: CdFsVolume = CdFsVolume::new();

        match cdfs_volume.read_data_stream(data_stream) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open CDFS volume");
                return Err(error);
            }
        }
        Ok(cdfs_volume)
    }

    /// Prints information about a volume.
    pub fn print_volume(data_stream: &DataStreamReference) -> Result<(), ErrorTrace> {
        let cdfs_volume: CdFsVolume = match Self::open_volume(data_stream) {
            Ok(cdfs_volume) => cdfs_volume,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open volume");
                return Err(error);
            }
        };
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    fn get_volume_system() -> Result<CdFsVolume, ErrorTrace> {
        let path_buf: PathBuf = PathBuf::from("../test_data/cdfs/level3.iso");
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        CdFsInfo::open_volume(&data_stream)
    }

    // TODO: add tests for open_volume
    // TODO: add tests for print_volume
}
