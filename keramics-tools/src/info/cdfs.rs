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

use std::fmt;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_formats::FileEntryIterator;
use keramics_formats::Path;
use keramics_formats::cdfs::{CdFsFileEntry, CdFsFileSystem, CdFsVolume};

use crate::formatters::ByteSize;

/// CD file system (CDFS) volume information.
struct CdFsVolumeInfo<'a> {
    /// Volume.
    volume: &'a CdFsVolume,
}

impl<'a> CdFsVolumeInfo<'a> {
    /// Creates new volume information.
    fn new(volume: &'a CdFsVolume) -> Self {
        Self { volume }
    }
}

impl<'a> fmt::Display for CdFsVolumeInfo<'a> {
    /// Formats volume information for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        writeln!(formatter, "CD file system (CDFS) volume information:")?;
        if self.volume.get_format_version() != 0 {
            writeln!(
                formatter,
                "    Format version\t\t\t\t: {}",
                self.volume.get_format_version()
            )?;
        }
        if let Some(system_identifier) = self.volume.get_system_identifier() {
            writeln!(
                formatter,
                "    System identifier\t\t\t\t: {}",
                system_identifier
            )?;
        }
        if let Some(volume_identifier) = self.volume.get_volume_identifier() {
            writeln!(
                formatter,
                "    Volume identifier\t\t\t\t: {}",
                volume_identifier
            )?;
        }
        writeln!(
            formatter,
            "    Volumes in set\t\t\t\t: {}",
            self.volume.get_volumes_in_set()
        )?;
        writeln!(
            formatter,
            "    Volume set index\t\t\t\t: {}",
            self.volume.get_volume_set_index()
        )?;
        if let Some(volume_set_identifier) = self.volume.get_volume_set_identifier() {
            writeln!(
                formatter,
                "    Volume set identifier\t\t: {}",
                volume_set_identifier
            )?;
        }
        writeln!(formatter)?;
        writeln!(
            formatter,
            "    Bytes per sector\t\t\t\t: {}",
            self.volume.get_bytes_per_sector()
        )?;
        let byte_size: ByteSize = ByteSize::new(self.volume.get_volume_size(), 1024);
        writeln!(formatter, "    Size\t\t\t\t\t: {}", byte_size)?;
        writeln!(formatter)
    }
}

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

    /// Retrieves a file system.
    fn get_file_system(data_stream: &DataStreamReference) -> Result<CdFsFileSystem, ErrorTrace> {
        let cdfs_volume: CdFsVolume = match Self::open_volume(data_stream) {
            Ok(cdfs_volume) => cdfs_volume,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open volume");
                return Err(error);
            }
        };
        match cdfs_volume.get_file_system() {
            Ok(cdfs_file_system) => Ok(cdfs_file_system),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve file system");
                return Err(error);
            }
        }
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
        let volume_info: CdFsVolumeInfo = CdFsVolumeInfo::new(&cdfs_volume);

        print!("{}", volume_info);

        Ok(())
    }

    /// Prints the file entry by path.
    pub fn print_file_entry_by_path(
        data_stream: &DataStreamReference,
        path: &Path,
    ) -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = match Self::get_file_system(data_stream) {
            Ok(cdfs_file_system) => cdfs_file_system,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open file system");
                return Err(error);
            }
        };
        let file_entry: CdFsFileEntry = match cdfs_file_system.get_file_entry_by_path(path) {
            Ok(Some(file_entry)) => file_entry,
            Ok(None) => return Err(keramics_core::error_trace_new!("Missing file entry")),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve file entry");
                return Err(error);
            }
        };
        println!("CD file system (CDFS) file entry information:");

        println!("    Path\t\t\t\t\t: {}", path);

        if let Some(name) = file_entry.get_name() {
            println!("    Name\t\t\t\t\t: {}", name);
        }
        println!("    Size\t\t\t\t\t: {}", file_entry.get_size());

        Ok(())
    }

    /// Prints the file system hierarchy.
    pub fn print_hierarchy(
        data_stream: &DataStreamReference,
        path: Option<&String>,
    ) -> Result<(), ErrorTrace> {
        let cdfs_file_system: CdFsFileSystem = match Self::get_file_system(data_stream) {
            Ok(cdfs_file_system) => cdfs_file_system,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open file system");
                return Err(error);
            }
        };
        println!("CD file system (CDFS) hierarchy:");

        let mut file_entry: CdFsFileEntry = match path {
            Some(path) => match cdfs_file_system.get_file_entry_by_path(&Path::from(path)) {
                Ok(Some(file_entry)) => file_entry,
                Ok(None) => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing file entry for path: {}",
                        path
                    )));
                }
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!("Unable to retrieve file entry for path: {}", path)
                    );
                    return Err(error);
                }
            },
            None => match cdfs_file_system.get_root_file_entry() {
                Ok(file_entry) => file_entry,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to retrieve root file entry"
                    );
                    return Err(error);
                }
            },
        };
        let mut path_components: Vec<String> = Vec::new();
        let mut levels: Vec<bool> = Vec::new();

        match Self::print_hierarchy_file_entry(&mut file_entry, &mut path_components, &mut levels) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to print file entry hierarchy"
                );
                return Err(error);
            }
        }
        Ok(())
    }

    /// Prints the file entry hierarchy.
    fn print_hierarchy_file_entry(
        file_entry: &mut CdFsFileEntry,
        path_components: &mut Vec<String>,
        levels: &mut Vec<bool>,
    ) -> Result<(), ErrorTrace> {
        let path: String = if file_entry.is_root_directory() {
            String::from("/")
        } else {
            let name_string: String = match file_entry.get_name() {
                Some(name) => name.to_string(),
                None => String::new(),
            };
            path_components.push(name_string);
            format!("/{}", path_components.join("/"))
        };
        let prefix: String = crate::hierarchy::get_hierarchy_prefix(levels);
        println!("{}{}", prefix, path);

        if file_entry.is_directory() {
            let number_of_sub_file_entries: usize =
                match file_entry.get_number_of_sub_file_entries() {
                    Ok(number) => number,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!(
                                "Unable to retrieve number of sub file entries of path: {}",
                                path
                            )
                        );
                        return Err(error);
                    }
                };
            for sub_file_entry_index in 0..number_of_sub_file_entries {
                let mut sub_file_entry: CdFsFileEntry =
                    match file_entry.get_sub_file_entry_by_index(sub_file_entry_index) {
                        Ok(sub_file_entry) => sub_file_entry,
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                format!(
                                    "Unable to retrieve sub file entry: {} of path: {}",
                                    sub_file_entry_index, path
                                )
                            );
                            return Err(error);
                        }
                    };
                let is_last: bool = sub_file_entry_index + 1 == number_of_sub_file_entries;
                levels.push(is_last);
                match Self::print_hierarchy_file_entry(&mut sub_file_entry, path_components, levels)
                {
                    Ok(_) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!(
                                "Unable to print hierarchy of sub file entry: {} of path: {}",
                                sub_file_entry_index, path
                            )
                        );
                        return Err(error);
                    }
                }
                levels.pop();
            }
        }
        if !file_entry.is_root_directory() {
            path_components.pop();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    use crate::assert_lines_eq;

    fn get_volume_system() -> Result<CdFsVolume, ErrorTrace> {
        let path_buf: PathBuf = PathBuf::from("../test_data/cdfs/level3.iso");
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        CdFsInfo::open_volume(&data_stream)
    }

    #[test]
    fn test_volume_information_fmt() -> Result<(), ErrorTrace> {
        let cdfs_volume: CdFsVolume = get_volume_system()?;

        let test_struct: CdFsVolumeInfo = CdFsVolumeInfo::new(&cdfs_volume);

        let expected_string: &str = concat!(
            "CD file system (CDFS) volume information:\n",
            "    Format version\t\t\t\t: 1\n",
            "    System identifier\t\t\t\t: LINUX\n",
            "    Volume identifier\t\t\t\t: CDROM\n",
            "    Volumes in set\t\t\t\t: 1\n",
            "    Volume set index\t\t\t\t: 1\n",
            "\n",
            "    Bytes per sector\t\t\t\t: 2048\n",
            "    Size\t\t\t\t\t: 2.4 MiB (2482176 bytes)\n",
            "\n"
        );
        let string: String = test_struct.to_string();
        assert_lines_eq!(string.as_str(), expected_string);

        Ok(())
    }

    // TODO: add tests for open_volume
    // TODO: add tests for print_volume
}
