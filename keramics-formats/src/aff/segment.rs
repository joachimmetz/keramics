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

/// AFF segment.
#[derive(Clone, Debug)]
pub(super) struct AffSegment {
    /// Segment name.
    name: String,

    /// Segment flag.
    flag: u32,

    /// Offset in bytes of the segment data in the data stream.
    data_offset: u64,

    /// Size in bytes of the segment data.
    data_length: u32,
}

impl AffSegment {
    /// Creates a new AFF segment.
    pub fn new(name: String, flag: u32, data_offset: u64, data_length: u32) -> Self {
        Self {
            name,
            flag,
            data_offset,
            data_length,
        }
    }

    /// Retrieves the data offset.
    pub fn get_data_offset(&self) -> u64 {
        self.data_offset
    }

    /// Retrieves the data length.
    pub fn get_data_length(&self) -> u32 {
        self.data_length
    }

    /// Retrieves the flag.
    pub fn get_flag(&self) -> u32 {
        self.flag
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let segment: AffSegment = AffSegment::new(String::from("page0"), 1, 878, 10109);

        assert_eq!(segment.get_name(), "page0");
        assert_eq!(segment.get_flag(), 1);
        assert_eq!(segment.get_data_offset(), 878);
        assert_eq!(segment.get_data_length(), 10109);
    }
}
