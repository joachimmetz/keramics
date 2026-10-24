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

use std::collections::HashMap;
use std::io::SeekFrom;

use keramics_core::{DataStreamReference, ErrorTrace, open_fake_data_stream};

use crate::file_resolver::FileResolverReference;
use crate::path_component::PathComponent;

use super::constants::*;
use super::segment::AffSegment;

/// Retrieves the zero-based page number of a page from a segment name.
fn get_page_index(name: &str) -> Option<usize> {
    let page_name: &str = name.strip_prefix("page")?;

    if page_name.is_empty() {
        return None;
    }
    let page_index: u64 = page_name.parse().ok()?;
    usize::try_from(page_index).ok()
}

/// Reads the data of a segment from a data stream.
fn read_segment_data(
    data_stream: &DataStreamReference,
    segment: &AffSegment,
) -> Result<Vec<u8>, ErrorTrace> {
    let data_length: u32 = segment.get_data_length();

    if data_length == 0 {
        return Ok(Vec::new());
    }
    let mut data: Vec<u8> = vec![0; data_length as usize];
    keramics_core::data_stream_read_exact_at_position!(
        data_stream.clone(),
        &mut data,
        SeekFrom::Start(segment.get_data_offset())
    );

    Ok(data)
}

/// Reads a 64-bit value from the data of a segment.
///
/// The value is stored as two 32-bit values, the lower 32 bits first, followed by the upper 32
/// bits, each stored in big-endian order.
fn read_value_64bit(
    data_stream: &DataStreamReference,
    segment: &AffSegment,
) -> Result<u64, ErrorTrace> {
    let data: Vec<u8> = read_segment_data(data_stream, segment)?;

    if data.len() >= 8 {
        let mut low_data: [u8; 4] = [0; 4];
        low_data.copy_from_slice(&data[0..4]);
        let mut high_data: [u8; 4] = [0; 4];
        high_data.copy_from_slice(&data[4..8]);

        let low_value: u32 = u32::from_be_bytes(low_data);
        let high_value: u32 = u32::from_be_bytes(high_data);

        return Ok((low_value as u64) | ((high_value as u64) << 32));
    }
    if data.len() >= 4 {
        let mut value_data: [u8; 4] = [0; 4];
        value_data.copy_from_slice(&data[0..4]);

        return Ok(u32::from_be_bytes(value_data) as u64);
    }
    Err(keramics_core::error_trace_new!(format!(
        "Unable to read 64-bit value of segment: {} (data size: {})",
        segment.get_name(),
        data.len()
    )))
}

/// AFF file.
#[derive(Clone)]
pub(super) struct AffFile {
    /// Data stream of the AFF file.
    data_stream: DataStreamReference,

    /// Parsed segments.
    segments: HashMap<String, AffSegment>,

    /// Media size in bytes.
    media_size: u64,

    /// Number of bytes per sector.
    bytes_per_sector: u32,

    /// Page size in bytes.
    page_size: u64,

    /// Number of stored pages of the image.
    number_of_pages: usize,

    /// Number of bad sectors.
    number_of_bad_sectors: u64,

    /// Pattern used to mark a bad sector.
    bad_flag: Vec<u8>,
}

impl AffFile {
    /// Creates a new AFF file.
    pub fn new() -> Self {
        Self {
            data_stream: open_fake_data_stream(&[]),
            segments: HashMap::new(),
            media_size: 0,
            bytes_per_sector: AFF_DEFAULT_BYTES_PER_SECTOR,
            page_size: AFF_DEFAULT_PAGE_SIZE,
            number_of_pages: 0,
            number_of_bad_sectors: 0,
            bad_flag: Vec::new(),
        }
    }

    /// Opens the AFF file.
    pub fn open(
        &mut self,
        file_resolver: &FileResolverReference,
        file_name: &PathComponent,
    ) -> Result<(), ErrorTrace> {
        let path_components: [PathComponent; 1] = [file_name.clone()];

        let data_stream: DataStreamReference = match file_resolver.get_data_stream(&path_components)
        {
            Ok(Some(data_stream)) => data_stream,
            Ok(None) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing AFF file: {}",
                    file_name
                )));
            }
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to open file: {}", file_name)
                );
                return Err(error);
            }
        };
        match self.read_data_stream(&data_stream) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to read AFF file: {}", file_name)
                );
                return Err(error);
            }
        }
        Ok(())
    }

    /// Reads the segments of the AFF file.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        self.data_stream = data_stream.clone();

        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());

        // A zero length file is a valid AFF file, without any segments.
        if size == 0 {
            return Ok(());
        }
        if size < AFF_FILE_SIGNATURE_SIZE {
            return Err(keramics_core::error_trace_new!(format!(
                "AFF file size: {} is smaller than the file header size: {}",
                size, AFF_FILE_SIGNATURE_SIZE
            )));
        }
        let mut file_signature: [u8; AFF_FILE_SIGNATURE_SIZE as usize] =
            [0; AFF_FILE_SIGNATURE_SIZE as usize];
        keramics_core::data_stream_read_exact_at_position!(
            data_stream.clone(),
            &mut file_signature,
            SeekFrom::Start(0)
        );
        if file_signature != AFF_FILE_SIGNATURE {
            return Err(keramics_core::error_trace_new!(
                "Invalid AFF file header signature"
            ));
        }

        let mut segments: HashMap<String, AffSegment> = HashMap::new();
        let mut current_offset: u64 = AFF_FILE_SIGNATURE_SIZE;

        while current_offset < size {
            let mut segment_header: [u8; AFF_SEGMENT_HEADER_SIZE as usize] =
                [0; AFF_SEGMENT_HEADER_SIZE as usize];
            keramics_core::data_stream_read_exact_at_position!(
                data_stream.clone(),
                &mut segment_header,
                SeekFrom::Start(current_offset)
            );
            if segment_header[0..4] != AFF_SEGMENT_SIGNATURE {
                return Err(keramics_core::error_trace_new!(format!(
                    "Invalid segment header signature at offset: {} (0x{:08x})",
                    current_offset, current_offset
                )));
            }
            let name_length: u32 = u32::from_be_bytes(segment_header[4..8].try_into().unwrap());
            let data_length: u32 = u32::from_be_bytes(segment_header[8..12].try_into().unwrap());
            let flag: u32 = u32::from_be_bytes(segment_header[12..16].try_into().unwrap());

            if name_length > AFF_MAX_SEGMENT_NAME_SIZE {
                return Err(keramics_core::error_trace_new!(format!(
                    "Segment name size: {} is larger than the maximum size: {} at offset: {} (0x{:08x})",
                    name_length, AFF_MAX_SEGMENT_NAME_SIZE, current_offset, current_offset
                )));
            }
            let name_offset: u64 = current_offset + AFF_SEGMENT_HEADER_SIZE;
            let name: String = if name_length > 0 {
                let mut name_data: Vec<u8> = vec![0; name_length as usize];
                keramics_core::data_stream_read_exact_at_position!(
                    data_stream.clone(),
                    &mut name_data,
                    SeekFrom::Start(name_offset)
                );
                match String::from_utf8(name_data) {
                    Ok(name) => name,
                    Err(_) => {
                        return Err(keramics_core::error_trace_new!(format!(
                            "Segment name is not valid UTF-8 at offset: {} (0x{:08x})",
                            name_offset, name_offset
                        )));
                    }
                }
            } else {
                // A segment with an empty name is an ignored segment.
                String::new()
            };
            let data_offset: u64 = name_offset + name_length as u64;

            let mut segment_tail: [u8; AFF_SEGMENT_TAIL_SIZE as usize] =
                [0; AFF_SEGMENT_TAIL_SIZE as usize];
            keramics_core::data_stream_read_exact_at_position!(
                data_stream.clone(),
                &mut segment_tail,
                SeekFrom::Start(data_offset + data_length as u64)
            );
            if segment_tail[0..4] != AFF_SEGMENT_TAIL_SIGNATURE {
                return Err(keramics_core::error_trace_new!(format!(
                    "Invalid segment tail signature of segment: {} at offset: {} (0x{:08x})",
                    name,
                    data_offset + data_length as u64,
                    data_offset + data_length as u64
                )));
            }
            let segment_size: u32 = u32::from_be_bytes(segment_tail[4..8].try_into().unwrap());
            let computed_size: u64 = AFF_SEGMENT_HEADER_SIZE
                + name_length as u64
                + data_length as u64
                + AFF_SEGMENT_TAIL_SIZE;
            if segment_size as u64 != computed_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Mismatched segment size: {} (0x{:08x}) expected: {} (0x{:08x}) of segment: {}",
                    segment_size, segment_size, computed_size, computed_size, name
                )));
            }
            segments.insert(
                name.clone(),
                AffSegment::new(name, flag, data_offset, data_length),
            );
            current_offset += computed_size;
        }

        // The order of the segments within an AFF file is not specified; pageN names contain the zero-based page number.
        let mut number_of_pages: usize = 0;
        for (segment_name, _) in segments.iter() {
            if get_page_index(segment_name).is_some() {
                number_of_pages += 1;
            }
        }
        let page_size: u64 = match segments.get("pagesize") {
            // The "segsize" name is the deprecated name for "pagesize".
            Some(segment) if segment.get_flag() != 0 => segment.get_flag() as u64,
            Some(segment) if segment.get_data_length() >= 4 => {
                read_value_64bit(data_stream, segment)?
            }
            _ => match segments.get("segsize") {
                Some(segment) if segment.get_flag() != 0 => segment.get_flag() as u64,
                Some(segment) if segment.get_data_length() >= 4 => {
                    read_value_64bit(data_stream, segment)?
                }
                _ => AFF_DEFAULT_PAGE_SIZE,
            },
        };
        let bytes_per_sector: u32 = match segments.get("sectorsize") {
            Some(segment) if segment.get_flag() != 0 => segment.get_flag(),
            Some(segment) if segment.get_data_length() >= 4 => {
                read_value_64bit(data_stream, segment)? as u32
            }
            _ => AFF_DEFAULT_BYTES_PER_SECTOR,
        };
        let media_size: u64 = match segments.get("imagesize") {
            Some(segment) => match read_value_64bit(data_stream, segment) {
                Ok(value) => value,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read imagesize value");
                    return Err(error);
                }
            },
            // When the image size is not stored, derive it from the stored pages.
            None => number_of_pages as u64 * page_size,
        };
        let number_of_bad_sectors: u64 = match segments.get("badsectors") {
            Some(segment) => read_value_64bit(data_stream, segment).unwrap_or_default(),
            None => 0,
        };
        let bad_flag: Vec<u8> = match segments.get("badflag") {
            Some(segment) => match read_segment_data(data_stream, segment) {
                Ok(data) => data,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read badflag data");
                    return Err(error);
                }
            },
            None => Vec::new(),
        };
        self.segments = segments;
        self.media_size = media_size;
        self.bytes_per_sector = bytes_per_sector;
        self.page_size = page_size;
        self.number_of_pages = number_of_pages;
        self.number_of_bad_sectors = number_of_bad_sectors;
        self.bad_flag = bad_flag;

        Ok(())
    }

    /// Retrieves the number of bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    /// Retrieves the pattern used to mark a bad sector.
    pub fn get_bad_flag(&self) -> &[u8] {
        &self.bad_flag
    }

    /// Retrieves the media size in bytes.
    pub fn get_media_size(&self) -> u64 {
        self.media_size
    }

    /// Retrieves the number of bad sectors.
    pub fn get_number_of_bad_sectors(&self) -> u64 {
        self.number_of_bad_sectors
    }

    /// Determines the number of parsed segments.
    pub fn get_number_of_segments(&self) -> usize {
        self.segments.len()
    }

    /// Determines the number of stored pages of the image.
    pub fn get_number_of_pages(&self) -> usize {
        self.number_of_pages
    }

    /// Retrieves the page size in bytes.
    pub fn get_page_size(&self) -> u64 {
        self.page_size
    }

    /// Retrieves a segment.
    pub fn get_segment(&self, name: &str) -> Option<&AffSegment> {
        self.segments.get(name)
    }

    /// Retrieves the data of a segment.
    pub fn get_segment_data(&self, name: &str) -> Result<Vec<u8>, ErrorTrace> {
        let segment: &AffSegment = match self.get_segment(name) {
            Some(segment) => segment,
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing segment: {}",
                    name
                )));
            }
        };
        match read_segment_data(&self.data_stream, segment) {
            Ok(data) => Ok(data),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to read segment data of segment: {}", name)
                );
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::os_file_resolver::OsFileResolver;

    use crate::tests::get_test_data_path;

    fn extend_u32_be(data: &mut Vec<u8>, value: u32) {
        data.extend_from_slice(&value.to_be_bytes());
    }

    fn get_file() -> Result<AffFile, ErrorTrace> {
        let mut file: AffFile = AffFile::new();

        let path_string: String = get_test_data_path("aff");
        let path_buf: PathBuf = PathBuf::from(path_string);
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let file_name: PathComponent = PathComponent::from("ext2.AFF");
        file.open(&file_resolver, &file_name)?;

        Ok(file)
    }

    #[test]
    fn test_get_number_of_segments() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let number_of_segments: usize = file.get_number_of_segments();
        assert_eq!(number_of_segments, 13);

        Ok(())
    }

    #[test]
    fn test_get_page_size() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let page_size: u64 = file.get_page_size();
        assert_eq!(page_size, 16777216);

        Ok(())
    }

    #[test]
    fn test_get_bytes_per_sector() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let bytes_per_sector: u32 = file.get_bytes_per_sector();
        assert_eq!(bytes_per_sector, 512);

        Ok(())
    }

    #[test]
    fn test_get_media_size() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let media_size: u64 = file.get_media_size();
        assert_eq!(media_size, 4194304);

        Ok(())
    }

    #[test]
    fn test_get_number_of_pages() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let number_of_pages: usize = file.get_number_of_pages();
        assert_eq!(number_of_pages, 1);

        Ok(())
    }

    #[test]
    fn test_get_number_of_bad_sectors() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let number_of_bad_sectors: u64 = file.get_number_of_bad_sectors();
        assert_eq!(number_of_bad_sectors, 0);

        Ok(())
    }

    #[test]
    fn test_get_bad_flag() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let bad_flag: &[u8] = file.get_bad_flag();
        assert_eq!(bad_flag.len(), 512);
        assert_eq!(&bad_flag[0..10], b"BAD SECTOR");

        Ok(())
    }

    #[test]
    fn test_get_segment() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let segment: &AffSegment = match file.get_segment("page0") {
            Some(segment) => segment,
            None => {
                return Err(keramics_core::error_trace_new!("Missing page0 segment"));
            }
        };
        assert_eq!(segment.get_flag(), 1);
        assert_eq!(segment.get_data_offset(), 878);
        assert_eq!(segment.get_data_length(), 10109);

        Ok(())
    }

    #[test]
    fn test_get_segment_data() -> Result<(), ErrorTrace> {
        let file: AffFile = get_file()?;

        let segment_data: Vec<u8> = file.get_segment_data("aff_file_type")?;
        assert_eq!(segment_data, b"AFF".to_vec());

        let segment_data: Vec<u8> = file.get_segment_data("afflib_version")?;
        assert_eq!(segment_data, b"\"3.7.22\"".to_vec());

        let segment_data: Vec<u8> = file.get_segment_data("md5")?;
        assert_eq!(
            segment_data,
            vec![
                0xb1, 0x76, 0x0d, 0x0b, 0x35, 0xa5, 0x12, 0xef, 0x56, 0x97, 0x0d, 0xf4, 0xe6, 0xf8,
                0xc5, 0xd6
            ]
        );

        Ok(())
    }

    #[test]
    fn test_get_segment_data_with_missing_segment() {
        let file: AffFile = get_file().expect("Unable to get file");

        let result: Result<Vec<u8>, ErrorTrace> = file.get_segment_data("sha256");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_zero_length() -> Result<(), ErrorTrace> {
        let mut file: AffFile = AffFile::new();
        let data_stream: DataStreamReference = open_fake_data_stream(b"");

        file.read_data_stream(&data_stream)?;

        assert_eq!(file.get_number_of_segments(), 0);
        assert_eq!(file.get_media_size(), 0);
        assert_eq!(file.get_page_size(), 16777216);
        assert_eq!(file.get_bytes_per_sector(), 512);

        Ok(())
    }

    #[test]
    fn test_read_data_stream_with_missing_data() {
        let mut file: AffFile = AffFile::new();
        let data_stream: DataStreamReference = open_fake_data_stream(b"AFF10");

        let result: Result<(), ErrorTrace> = file.read_data_stream(&data_stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_invalid_file_signature() {
        let mut file: AffFile = AffFile::new();
        let data_stream: DataStreamReference = open_fake_data_stream(b"XXXX\r\n\0");

        let result: Result<(), ErrorTrace> = file.read_data_stream(&data_stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_missing_segment_data() {
        let mut file: AffFile = AffFile::new();
        let mut test_data: Vec<u8> = b"AFF10\r\n\0".to_vec();
        test_data.extend_from_slice(b"ABCD");

        let data_stream: DataStreamReference = open_fake_data_stream(test_data.as_slice());
        let result: Result<(), ErrorTrace> = file.read_data_stream(&data_stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_invalid_segment_signature() {
        let mut file: AffFile = AffFile::new();
        let mut test_data: Vec<u8> = b"AFF10\r\n\0".to_vec();
        test_data.extend_from_slice(b"XXX\0");
        extend_u32_be(&mut test_data, 1);
        extend_u32_be(&mut test_data, 0);
        extend_u32_be(&mut test_data, 0);
        test_data.extend_from_slice(b"A");
        test_data.extend_from_slice(b"ATT\0");
        extend_u32_be(&mut test_data, 24);

        let data_stream: DataStreamReference = open_fake_data_stream(test_data.as_slice());
        let result: Result<(), ErrorTrace> = file.read_data_stream(&data_stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_invalid_segment_tail() {
        let mut file: AffFile = AffFile::new();
        let mut test_data: Vec<u8> = b"AFF10\r\n\0".to_vec();
        test_data.extend_from_slice(b"AFF\0");
        extend_u32_be(&mut test_data, 1);
        extend_u32_be(&mut test_data, 0);
        extend_u32_be(&mut test_data, 0);
        test_data.extend_from_slice(b"A");
        test_data.extend_from_slice(b"XXX\0");
        extend_u32_be(&mut test_data, 24);

        let data_stream: DataStreamReference = open_fake_data_stream(test_data.as_slice());
        let result: Result<(), ErrorTrace> = file.read_data_stream(&data_stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_mismatched_segment_size() {
        let mut file: AffFile = AffFile::new();
        let mut test_data: Vec<u8> = b"AFF10\r\n\0".to_vec();
        test_data.extend_from_slice(b"AFF\0");
        extend_u32_be(&mut test_data, 1);
        extend_u32_be(&mut test_data, 0);
        extend_u32_be(&mut test_data, 0);
        test_data.extend_from_slice(b"A");
        test_data.extend_from_slice(b"ATT\0");
        extend_u32_be(&mut test_data, 24);

        let data_stream: DataStreamReference = open_fake_data_stream(test_data.as_slice());
        let result: Result<(), ErrorTrace> = file.read_data_stream(&data_stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_stream_with_ignored_segment() -> Result<(), ErrorTrace> {
        let mut file: AffFile = AffFile::new();
        let mut test_data: Vec<u8> = b"AFF10\r\n\0".to_vec();
        test_data.extend_from_slice(b"AFF\0");
        extend_u32_be(&mut test_data, 0);
        extend_u32_be(&mut test_data, 0);
        extend_u32_be(&mut test_data, 0);
        test_data.extend_from_slice(b"ATT\0");
        extend_u32_be(&mut test_data, 24);

        let data_stream: DataStreamReference = open_fake_data_stream(test_data.as_slice());
        file.read_data_stream(&data_stream)?;

        assert_eq!(file.get_number_of_segments(), 1);
        assert!(file.get_segment("").is_some());

        Ok(())
    }
}
