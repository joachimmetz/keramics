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

use crate::block_stream::BlockStream;

use super::block_reader::UdifBlockReader;

/// Universal Disk Image Format (UDIF) block stream.
pub type UdifBlockStream = BlockStream<UdifBlockReader>;

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::SeekFrom;
    use std::path::PathBuf;

    use keramics_core::{DataStream, DataStreamReference, ErrorTrace, open_os_data_stream};
    use keramics_hashes::{DigestHashContext, Sha256Context};

    use crate::tests::get_test_data_path;
    use crate::udif::block_range::{UdifBlockRange, UdifBlockRangeType};
    use crate::udif::enums::UdifCompressionMethod;

    fn get_block_stream() -> Result<UdifBlockStream, ErrorTrace> {
        let path_string: String = get_test_data_path("udif/hfsplus_zlib.dmg");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let block_ranges: [UdifBlockRange; 14] = [
            UdifBlockRange {
                media_offset: 0,
                data_offset: 6392,
                size: 512,
                compressed_data_size: 31,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 512,
                data_offset: 6315,
                size: 512,
                compressed_data_size: 77,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1024,
                data_offset: 0,
                size: 16384,
                compressed_data_size: 177,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 17408,
                data_offset: 0,
                size: 3072,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 20480,
                data_offset: 10251,
                size: 1029120,
                compressed_data_size: 6158,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1049600,
                data_offset: 0,
                size: 19456,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1069056,
                data_offset: 0,
                size: 843776,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1912832,
                data_offset: 6423,
                size: 28672,
                compressed_data_size: 3358,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1941504,
                data_offset: 0,
                size: 3072,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1944576,
                data_offset: 9781,
                size: 512,
                compressed_data_size: 132,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1945088,
                data_offset: 0,
                size: 512,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1945600,
                data_offset: 0,
                size: 1536,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1947136,
                data_offset: 9996,
                size: 16384,
                compressed_data_size: 177,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1963520,
                data_offset: 10173,
                size: 512,
                compressed_data_size: 78,
                range_type: UdifBlockRangeType::Compressed,
            },
        ];
        Ok(UdifBlockStream::new(UdifBlockReader::new(
            &data_stream,
            &block_ranges,
            &UdifCompressionMethod::Zlib,
            1964032,
        )))
    }

    #[test]
    fn test_get_offset() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;

        block_stream.seek(SeekFrom::Start(1024))?;

        let offset: u64 = block_stream.get_offset()?;
        assert_eq!(offset, 1024);

        Ok(())
    }

    #[test]
    fn test_get_size() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;

        let size: u64 = block_stream.get_size()?;
        assert_eq!(size, 1964032);

        Ok(())
    }

    #[test]
    fn test_seek_from_start() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;

        let offset: u64 = block_stream.seek(SeekFrom::Start(1024))?;
        assert_eq!(offset, 1024);

        Ok(())
    }

    #[test]
    fn test_seek_from_end() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;
        let size: u64 = block_stream.get_size()?;

        let offset: u64 = block_stream.seek(SeekFrom::End(-512))?;
        assert_eq!(offset, size - 512);

        Ok(())
    }

    #[test]
    fn test_seek_from_current() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;

        let offset = block_stream.seek(SeekFrom::Start(1024))?;
        assert_eq!(offset, 1024);

        let offset: u64 = block_stream.seek(SeekFrom::Current(-512))?;
        assert_eq!(offset, 512);

        Ok(())
    }

    #[test]
    fn test_seek_before_zero() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;

        let result: Result<u64, ErrorTrace> = block_stream.seek(SeekFrom::Current(-512));
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_seek_beyond_size() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;
        let size: u64 = block_stream.get_size()?;

        let offset: u64 = block_stream.seek(SeekFrom::End(512))?;
        assert_eq!(offset, size + 512);

        Ok(())
    }

    #[test]
    fn test_seek_and_read() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;
        block_stream.seek(SeekFrom::Start(1024))?;

        let mut data: Vec<u8> = vec![0; 512];
        let read_size: usize = block_stream.read(&mut data)?;
        assert_eq!(read_size, 512);

        let expected_data: Vec<u8> = vec![
            0x00, 0x53, 0x46, 0x48, 0x00, 0x00, 0xaa, 0x11, 0xaa, 0x11, 0x00, 0x30, 0x65, 0x43,
            0xec, 0xac, 0xb2, 0xb3, 0x80, 0x60, 0xbe, 0x78, 0xa9, 0x4d, 0x8b, 0x19, 0x2f, 0xcc,
            0x48, 0x39, 0xca, 0x2d, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd7, 0x0e,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x64, 0x00, 0x69, 0x00, 0x73, 0x00, 0x6b, 0x00, 0x20, 0x00, 0x69, 0x00, 0x6d, 0x00,
            0x61, 0x00, 0x67, 0x00, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(data, expected_data);

        Ok(())
    }

    #[test]
    fn test_seek_and_read_beyond_size() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_block_stream()?;
        block_stream.seek(SeekFrom::End(512))?;

        let mut data: Vec<u8> = vec![0; 512];
        let read_size: usize = block_stream.read(&mut data)?;
        assert_eq!(read_size, 0);

        Ok(())
    }

    fn get_lzma_block_stream() -> Result<UdifBlockStream, ErrorTrace> {
        let path_string: String = get_test_data_path("udif/hfsplus_lzma.dmg");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let block_ranges: [UdifBlockRange; 14] = [
            UdifBlockRange {
                media_offset: 0,
                data_offset: 1612,
                size: 512,
                compressed_data_size: 84,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 512,
                data_offset: 1480,
                size: 512,
                compressed_data_size: 132,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1024,
                data_offset: 0,
                size: 16384,
                compressed_data_size: 172,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 17408,
                data_offset: 0,
                size: 3072,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 20480,
                data_offset: 4868,
                size: 1029120,
                compressed_data_size: 1412,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1049600,
                data_offset: 0,
                size: 19456,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1069056,
                data_offset: 0,
                size: 843776,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1912832,
                data_offset: 1696,
                size: 28672,
                compressed_data_size: 2940,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1941504,
                data_offset: 0,
                size: 3072,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1944576,
                data_offset: 560,
                size: 512,
                compressed_data_size: 160,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1945088,
                data_offset: 0,
                size: 512,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1945600,
                data_offset: 0,
                size: 1536,
                compressed_data_size: 0,
                range_type: UdifBlockRangeType::Sparse,
            },
            UdifBlockRange {
                media_offset: 1947136,
                data_offset: 256,
                size: 16384,
                compressed_data_size: 172,
                range_type: UdifBlockRangeType::Compressed,
            },
            UdifBlockRange {
                media_offset: 1963520,
                data_offset: 428,
                size: 512,
                compressed_data_size: 132,
                range_type: UdifBlockRangeType::Compressed,
            },
        ];
        Ok(UdifBlockStream::new(UdifBlockReader::new(
            &data_stream,
            &block_ranges,
            &UdifCompressionMethod::Lzma,
            1964032,
        )))
    }

    #[test]
    fn test_lzma_get_size() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_lzma_block_stream()?;

        let size: u64 = block_stream.get_size()?;
        assert_eq!(size, 1964032);

        Ok(())
    }

    #[test]
    fn test_lzma_seek_and_read() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_lzma_block_stream()?;
        block_stream.seek(SeekFrom::Start(20480))?;

        let mut data: Vec<u8> = vec![0; 512];
        let read_size: usize = block_stream.read(&mut data)?;
        assert_eq!(read_size, 512);

        // The first sector of the HFS+ volume.
        let mut sha256_context: Sha256Context = Sha256Context::new();
        sha256_context.update(&data);
        let expected_checksum: [u8; 32] = [
            0x07, 0x6a, 0x27, 0xc7, 0x9e, 0x5a, 0xce, 0x2a, 0x3d, 0x47, 0xf9, 0xdd, 0x2e, 0x83,
            0xe4, 0xff, 0x6e, 0xa8, 0x87, 0x2b, 0x3c, 0x22, 0x18, 0xf6, 0x6c, 0x92, 0xb8, 0x9b,
            0x55, 0xf3, 0x65, 0x60,
        ];
        assert_eq!(sha256_context.finalize(), expected_checksum.to_vec());

        Ok(())
    }

    #[test]
    fn test_lzma_read_entire_media() -> Result<(), ErrorTrace> {
        let mut block_stream: UdifBlockStream = get_lzma_block_stream()?;
        let size: u64 = block_stream.get_size()?;
        block_stream.seek(SeekFrom::Start(0))?;

        let mut data: Vec<u8> = vec![0; size as usize];
        let read_size: usize = block_stream.read(&mut data)?;
        assert_eq!(read_size, size as usize);

        // The decompressed media is a UEFI GPT disk image containing an HFS+ volume.
        assert_eq!(u16::from_be_bytes([data[510], data[511]]), 0x55AA);
        assert_eq!(&data[512..520], b"EFI PART");
        assert_eq!(&data[21504..21506], b"H+");
        assert_eq!(&data[1963520..1963528], b"EFI PART");

        let mut sha256_context: Sha256Context = Sha256Context::new();
        sha256_context.update(&data);
        let expected_checksum: [u8; 32] = [
            0xb1, 0x44, 0xd0, 0xfe, 0xdf, 0xa6, 0x3d, 0x6c, 0x4d, 0xfa, 0x65, 0x90, 0x4b, 0xca,
            0x85, 0xfd, 0xa6, 0x7a, 0xa4, 0x0b, 0x16, 0x6f, 0x1d, 0x14, 0x7e, 0xf5, 0xde, 0x28,
            0xb7, 0xdb, 0x8b, 0xc8,
        ];
        assert_eq!(sha256_context.finalize(), expected_checksum.to_vec());

        Ok(())
    }
}
