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

use super::block_reader::FfsBlockReader;

/// Fast File System (FFS) block stream.
pub type FfsBlockStream = BlockStream<FfsBlockReader>;

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::SeekFrom;

    use keramics_core::{DataStream, DataStreamReference, ErrorTrace, open_fake_data_stream};

    use super::super::block_range::{FfsBlockRange, FfsBlockRangeType};

    #[test]
    fn test_read_and_seek() -> Result<(), ErrorTrace> {
        let mut backing_data: Vec<u8> = vec![0; 16384];
        backing_data[4096..4104].copy_from_slice(b"My file\n");
        let data_stream: DataStreamReference = open_fake_data_stream(&backing_data);

        let block_ranges: Vec<FfsBlockRange> = vec![
            FfsBlockRange::new(0, 4096, 8, FfsBlockRangeType::InFile),
            FfsBlockRange::new(8, 0, 8, FfsBlockRangeType::Sparse),
        ];
        let block_reader: FfsBlockReader = FfsBlockReader::new(&data_stream, &block_ranges, 16);
        let mut block_stream: FfsBlockStream = FfsBlockStream::new(block_reader);

        let stream_size: u64 = block_stream.get_size()?;
        assert_eq!(stream_size, 16);

        let mut buffer: [u8; 16] = [0xff; 16];
        let read_count: usize = block_stream.read(&mut buffer)?;
        assert_eq!(read_count, 16);
        assert_eq!(&buffer[0..8], b"My file\n");
        assert_eq!(&buffer[8..16], &[0; 8]);

        let offset: u64 = block_stream.seek(SeekFrom::Start(3))?;
        assert_eq!(offset, 3);

        let mut partial: [u8; 5] = [0; 5];
        let read_count: usize = block_stream.read(&mut partial)?;
        assert_eq!(read_count, 5);
        assert_eq!(&partial, b"file\n");

        Ok(())
    }
}
