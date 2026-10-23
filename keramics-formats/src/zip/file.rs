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

use keramics_checksums::ReversedCrc32Context;
use keramics_core::{DataStreamReference, ErrorTrace};

use super::constants::*;
use super::entry::ZipEntry;
use super::entry_header::ZipEntryHeader;
use super::eocd::ZipEndOfCentralDirectoryRecord;
use super::eocd64::Zip64EndOfCentralDirectoryRecord;
use super::eocd64_locator::Zip64EndOfCentralDirectoryLocator;

/// Zip (ZIP) file.
#[derive(Clone)]
pub struct ZipFile {
    /// Data stream.
    data_stream: Option<DataStreamReference>,

    /// Zip file comment.
    file_comment: String,

    /// Central directory entries.
    entries: Vec<ZipEntry>,
}

impl Default for ZipFile {
    /// Creates a new default Zip file.
    fn default() -> Self {
        Self::new()
    }
}

impl ZipFile {
    /// Creates a new Zip file.
    pub fn new() -> Self {
        Self {
            data_stream: None,
            file_comment: String::new(),
            entries: Vec::new(),
        }
    }

    /// Retrieves an entry with the specified name.
    pub fn get_entry(&self, file_name: &str) -> Option<&ZipEntry> {
        self.entries
            .iter()
            .find(|entry| entry.file_name == file_name)
    }

    /// Retrieves the entries.
    pub fn get_entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// Retrieves the Zip file comment.
    pub fn get_file_comment(&self) -> &str {
        &self.file_comment
    }

    /// Reads the data of a stored (not compressed) or deflate compressed entry.
    ///
    /// The data buffer must be sized to the uncompressed size of the entry.
    pub fn read_entry_data(&mut self, entry: &ZipEntry, data: &mut [u8]) -> Result<(), ErrorTrace> {
        let data_stream: &DataStreamReference = match self.data_stream.as_ref() {
            Some(data_stream) => data_stream,
            None => {
                return Err(keramics_core::error_trace_new!("Missing data stream"));
            }
        };
        if (entry.general_purpose_flags & 0x0001) != 0 {
            return Err(keramics_core::error_trace_new!(format!(
                "Encrypted entry data of entry: {} is not supported",
                entry.file_name
            )));
        }
        let mut entry_header: ZipEntryHeader = ZipEntryHeader::new();

        let entry_offset: u64 = entry.entry_offset as u64;

        match entry_header.read_at_position(data_stream, SeekFrom::Start(entry_offset)) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read entry header of entry: {} at offset: {} (0x{:08x})",
                        entry.file_name, entry_offset, entry_offset
                    )
                );
                return Err(error);
            }
        }
        if entry_header.file_name_length != entry.file_name_length {
            return Err(keramics_core::error_trace_new!(format!(
                "Mismatch between entry header and central directory entry for entry: {}",
                entry.file_name
            )));
        }
        let entry_data_offset: u64 = entry_offset
            + 30
            + (entry_header.file_name_length as u64)
            + (entry_header.extra_field_length as u64);
        // The sizes in the local header may be zero, when the general purpose
        // bit flag indicates a data descriptor. The central directory entry is
        // the authoritative source.
        let compressed_size: u64 = entry.compressed_size as u64;
        let uncompressed_size: u64 = entry.uncompressed_size as u64;

        match entry.compression_method {
            ZIP_COMPRESSION_METHOD_STORED => {
                if compressed_size as usize != uncompressed_size as usize
                    || uncompressed_size as usize != data.len()
                {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Unsupported compressed size: {} or uncompressed size: {} in data size: {} for entry: {}",
                        compressed_size,
                        uncompressed_size,
                        data.len(),
                        entry.file_name
                    )));
                }
                keramics_core::data_stream_read_exact_at_position!(
                    data_stream,
                    data,
                    SeekFrom::Start(entry_data_offset)
                );
            }
            ZIP_COMPRESSION_METHOD_DEFLATE => {
                let mut compressed_data: Vec<u8> = vec![0; compressed_size as usize];

                keramics_core::data_stream_read_exact_at_position!(
                    data_stream,
                    &mut compressed_data,
                    SeekFrom::Start(entry_data_offset)
                );
                _ = crate::deflate_decompress!(
                    &compressed_data,
                    data,
                    "Unable to decompress entry data"
                );
            }
            _ => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported compression method: {} for entry: {}",
                    entry.compression_method, entry.file_name
                )));
            }
        }
        let mut crc32_context: ReversedCrc32Context = ReversedCrc32Context::new(0xedb88320, 0);
        crc32_context.update(data);
        let calculated_crc_32: u32 = crc32_context.finalize();

        if calculated_crc_32 != entry.crc_32 {
            return Err(keramics_core::error_trace_new!(format!(
                "Mismatch between stored: 0x{:08x} and calculated: 0x{:08x} CRC-32 checksum for entry: {}",
                entry.crc_32, calculated_crc_32, entry.file_name
            )));
        }
        Ok(())
    }

    /// Reads a Zip file.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        let file_size: u64 = keramics_core::data_stream_get_size!(data_stream);

        if file_size < 22 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported data size too small for end of central directory record"
            ));
        }
        let (eocd_offset, eocd) = match self.find_end_of_central_directory(data_stream, file_size) {
            Ok(record) => record,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to find end of central directory record"
                );
                return Err(error);
            }
        };
        let (central_directory_offset, central_directory_size, number_of_entries) = match self
            .read_end_of_central_directory_record(data_stream, &eocd, eocd_offset, file_size)
        {
            Ok(record) => record,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read end of central directory record"
                );
                return Err(error);
            }
        };
        if central_directory_size != 0
            && central_directory_offset + central_directory_size > file_size
        {
            return Err(keramics_core::error_trace_new!(
                "Unsupported central directory size value out of bounds"
            ));
        }
        match self.read_entries(data_stream, central_directory_offset, number_of_entries) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read central directory entries"
                );
                return Err(error);
            }
        }
        if eocd.file_comment_length != 0 {
            let mut comment_data: Vec<u8> = vec![0; eocd.file_comment_length as usize];

            keramics_core::data_stream_read_exact_at_position!(
                data_stream,
                &mut comment_data,
                SeekFrom::Start(eocd_offset + 22)
            );
            self.file_comment = match String::from_utf8(comment_data) {
                Ok(string) => string,
                Err(error) => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Unsupported encoding of Zip file comment with error: {}",
                        error
                    )));
                }
            };
        }
        self.data_stream = Some(data_stream.clone());

        Ok(())
    }

    /// Reads the central directory entries.
    fn read_entries(
        &mut self,
        data_stream: &DataStreamReference,
        central_directory_offset: u64,
        number_of_entries: u64,
    ) -> Result<(), ErrorTrace> {
        let mut entry_offset: u64 = central_directory_offset;

        for _ in 0..number_of_entries {
            let mut entry: ZipEntry = ZipEntry::new();

            match entry.read_at_position(data_stream, SeekFrom::Start(entry_offset)) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read central directory entry at offset: {} (0x{:08x})",
                            entry_offset, entry_offset
                        )
                    );
                    return Err(error);
                }
            }
            let file_name_length: usize = entry.file_name_length as usize;

            if file_name_length != 0 {
                let mut file_name_data: Vec<u8> = vec![0; file_name_length];

                keramics_core::data_stream_read_exact_at_position!(
                    data_stream,
                    &mut file_name_data,
                    SeekFrom::Start(entry_offset + 46)
                );
                entry.file_name = match String::from_utf8(file_name_data) {
                    Ok(string) => string,
                    Err(error) => {
                        return Err(keramics_core::error_trace_new!(format!(
                            "Unsupported encoding of entry file name with error: {}",
                            error
                        )));
                    }
                };
            }
            let entry_length: u64 = 46
                + (entry.file_name_length as u64)
                + (entry.extra_field_length as u64)
                + (entry.file_comment_length as u64);
            self.entries.push(entry);
            entry_offset += entry_length;
        }
        Ok(())
    }

    /// Finds the end of central directory record, which is located at the end
    /// of the file and is followed by the Zip file comment (if present).
    fn find_end_of_central_directory(
        &self,
        data_stream: &DataStreamReference,
        file_size: u64,
    ) -> Result<(u64, ZipEndOfCentralDirectoryRecord), ErrorTrace> {
        let search_start_offset: u64 = file_size.saturating_sub(22 + 65535);

        let mut eocd_offset: u64 = file_size - 22;

        loop {
            let mut eocd: ZipEndOfCentralDirectoryRecord = ZipEndOfCentralDirectoryRecord::new();

            if eocd
                .read_at_position(data_stream, SeekFrom::Start(eocd_offset))
                .is_ok()
                && eocd_offset + 22 + (eocd.file_comment_length as u64) == file_size
            {
                return Ok((eocd_offset, eocd));
            }
            if eocd_offset <= search_start_offset {
                break;
            }
            eocd_offset -= 1;
        }
        Err(keramics_core::error_trace_new!(
            "Missing end of central directory record"
        ))
    }

    /// Reads the zip64 end of central directory records (if present) and
    /// retrieves the offsets and sizes of the central directory.
    fn read_end_of_central_directory_record(
        &mut self,
        data_stream: &DataStreamReference,
        eocd: &ZipEndOfCentralDirectoryRecord,
        eocd_offset: u64,
        file_size: u64,
    ) -> Result<(u64, u64, u64), ErrorTrace> {
        let mut central_directory_offset: u64 = eocd.central_directory_offset as u64;
        let mut central_directory_size: u64 = eocd.central_directory_size as u64;
        let mut number_of_entries: u64 = eocd.number_of_entries as u64;

        if eocd.number_of_entries == 0xffff
            || eocd.central_directory_size == 0xffffffff
            || eocd.central_directory_offset == 0xffffffff
        {
            if eocd_offset < 20 {
                return Err(keramics_core::error_trace_new!(
                    "Missing zip64 end of central directory locator"
                ));
            }
            let mut eocd64_locator: Zip64EndOfCentralDirectoryLocator =
                Zip64EndOfCentralDirectoryLocator::new();

            match eocd64_locator.read_at_position(data_stream, SeekFrom::Start(eocd_offset - 20)) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read zip64 end of central directory locator"
                    );
                    return Err(error);
                }
            }
            if eocd64_locator.eocd64_offset + 56 > file_size
                || eocd64_locator.eocd64_offset >= eocd_offset
            {
                return Err(keramics_core::error_trace_new!(
                    "Unsupported zip64 end of central directory record offset value out of bounds"
                ));
            }
            let mut eocd64: Zip64EndOfCentralDirectoryRecord =
                Zip64EndOfCentralDirectoryRecord::new();

            match eocd64
                .read_at_position(data_stream, SeekFrom::Start(eocd64_locator.eocd64_offset))
            {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read zip64 end of central directory record at offset: {} (0x{:08x})",
                            eocd64_locator.eocd64_offset, eocd64_locator.eocd64_offset
                        )
                    );
                    return Err(error);
                }
            }
            number_of_entries = eocd64.number_of_entries;
            central_directory_size = eocd64.central_directory_size;
            central_directory_offset = eocd64.central_directory_offset;
        }
        Ok((
            central_directory_offset,
            central_directory_size,
            number_of_entries,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    use crate::tests::get_test_data_path;

    fn get_data_stream() -> Result<DataStreamReference, ErrorTrace> {
        let path_string: String = get_test_data_path("aff4/ext2.aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        open_os_data_stream(&path_buf)
    }

    #[test]
    fn test_read_data_stream() -> Result<(), ErrorTrace> {
        let mut file: ZipFile = ZipFile::new();

        let data_stream: DataStreamReference = get_data_stream()?;
        file.read_data_stream(&data_stream)?;

        let entries: &[ZipEntry] = file.get_entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(
            file.get_file_comment(),
            "aff4://00000000-0000-1003-8000-000000000004"
        );

        Ok(())
    }

    #[test]
    fn test_get_entry() -> Result<(), ErrorTrace> {
        let mut file: ZipFile = ZipFile::new();

        let data_stream: DataStreamReference = get_data_stream()?;
        file.read_data_stream(&data_stream)?;

        let entry: Option<&ZipEntry> = file.get_entry("version.txt");
        let Some(entry) = entry else {
            panic!("Missing version entry");
        };
        assert_eq!(entry.file_name, "version.txt");
        assert_eq!(entry.compression_method, ZIP_COMPRESSION_METHOD_STORED);
        assert_eq!(entry.uncompressed_size, 45);
        assert_eq!(entry.compressed_size, 45);
        assert_eq!(entry.file_name_length, 11);

        assert!(file.get_entry("version.txt.missing").is_none());

        Ok(())
    }

    #[test]
    fn test_read_entry_data_stored() -> Result<(), ErrorTrace> {
        let mut file: ZipFile = ZipFile::new();

        let data_stream: DataStreamReference = get_data_stream()?;
        file.read_data_stream(&data_stream)?;

        let entry: ZipEntry = match file.get_entry("version.txt") {
            Some(entry) => entry.clone(),
            None => panic!("Missing version entry"),
        };
        let mut data: Vec<u8> = vec![0; entry.uncompressed_size as usize];
        file.read_entry_data(&entry, &mut data)?;

        assert_eq!(
            String::from_utf8_lossy(&data),
            "major=1\nminor=0\ntool=libaff4 2.0 pre-release\n"
        );

        Ok(())
    }

    #[test]
    fn test_read_entry_data_deflate() -> Result<(), ErrorTrace> {
        let mut file: ZipFile = ZipFile::new();

        let data_stream: DataStreamReference = get_data_stream()?;
        file.read_data_stream(&data_stream)?;

        let entry: ZipEntry = match file.get_entry("test_data/ext/ext2.raw") {
            Some(entry) => entry.clone(),
            None => panic!("Missing image entry"),
        };
        assert_eq!(entry.compression_method, ZIP_COMPRESSION_METHOD_DEFLATE);
        assert_eq!(entry.uncompressed_size, 4194304);

        let mut data: Vec<u8> = vec![0; entry.uncompressed_size as usize];
        file.read_entry_data(&entry, &mut data)?;

        let raw_path_string: String = crate::tests::get_test_data_path("ext/ext2.raw");
        let raw_path_buf: PathBuf = PathBuf::from(raw_path_string.as_str());
        let raw_image_data: Vec<u8> = match fs::read(raw_path_buf) {
            Ok(raw_image_data) => raw_image_data,
            Err(error) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unable to read raw image data: {}",
                    error
                )));
            }
        };
        assert_eq!(data.len(), raw_image_data.len());
        assert_eq!(data, raw_image_data);

        Ok(())
    }

    // TODO: add tests for read_entries
}
