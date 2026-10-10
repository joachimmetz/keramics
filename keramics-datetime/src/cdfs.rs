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

/// CD file system (CDFS) date and time value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CdFsDateTime {
    /// Year, where 0 represents 1900.
    pub year: u8,

    /// Month, starting with 1.
    pub month: u8,

    /// Day of month, starting with 1.
    pub day_of_month: u8,

    /// Hours.
    pub hours: u8,

    /// Minutes.
    pub minutes: u8,

    /// Seconds.
    pub seconds: u8,

    /// UTC offset in number of 15 minute intervals, where negative values
    /// represent West of UTC.
    pub utc_offset: i8,
}

impl CdFsDateTime {
    /// Creates a new date and time value.
    pub fn new(year: u8, month: u8, day_of_month: u8, hours: u8, minutes: u8, seconds: u8) -> Self {
        Self {
            year,
            month,
            day_of_month,
            hours,
            minutes,
            seconds,
            utc_offset: 0,
        }
    }

    /// Reads a date and time value from a byte sequence.
    pub fn from_bytes(data: &[u8]) -> Self {
        Self {
            year: data[0],
            month: data[1],
            day_of_month: data[2],
            hours: data[3],
            minutes: data[4],
            seconds: data[5],
            utc_offset: data[6] as i8,
        }
    }

    /// Sets the UTC offset in number of 15 minute intervals, where negative
    /// values represent West of UTC.
    pub fn set_utc_offset(&mut self, utc_offset: i8) {
        self.utc_offset = utc_offset
    }

    /// Retrieves an ISO 8601 string representation of the date and time value.
    pub fn to_iso8601_string(&self) -> String {
        let year: i16 = 1900 + (self.year as i16);

        let (time_zone_sign, time_zone_intervals): (char, u8) = if self.utc_offset < 0 {
            ('-', (-(self.utc_offset) as u8))
        } else {
            ('+', self.utc_offset as u8)
        };
        let time_zone_hours: u8 = time_zone_intervals / 4;
        let time_zone_minutes: u8 = (time_zone_intervals % 4) * 15;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
            year,
            self.month,
            self.day_of_month,
            self.hours,
            self.minutes,
            self.seconds,
            time_zone_sign,
            time_zone_hours,
            time_zone_minutes
        )
    }
}

impl fmt::Display for CdFsDateTime {
    /// Formats the date and time value for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.to_iso8601_string(),)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdfs_date_time_from_bytes() {
        let test_data: [u8; 7] = [0x6e, 0x08, 0x0c, 0x14, 0x06, 0x1f, 0x08];

        let test_struct: CdFsDateTime = CdFsDateTime::from_bytes(&test_data);
        assert_eq!(test_struct.year, 0x6e);
        assert_eq!(test_struct.month, 0x08);
        assert_eq!(test_struct.day_of_month, 0x0c);
        assert_eq!(test_struct.hours, 0x14);
        assert_eq!(test_struct.minutes, 0x06);
        assert_eq!(test_struct.seconds, 0x1f);
        assert_eq!(test_struct.utc_offset, 8);
    }

    #[test]
    fn test_cdfs_date_time_to_iso8601_string() {
        let mut test_struct: CdFsDateTime = CdFsDateTime::new(0x6e, 0x08, 0x0c, 0x14, 0x06, 0x1f);

        let string: String = test_struct.to_iso8601_string();
        assert_eq!(string.as_str(), "2010-08-12T20:06:31+00:00");

        test_struct.set_utc_offset(8);

        let string: String = test_struct.to_iso8601_string();
        assert_eq!(string.as_str(), "2010-08-12T20:06:31+02:00");

        test_struct.set_utc_offset(-10);

        let string: String = test_struct.to_iso8601_string();
        assert_eq!(string.as_str(), "2010-08-12T20:06:31-02:30");
    }

    #[test]
    fn test_cdfs_date_time_to_string() {
        let test_struct: CdFsDateTime = CdFsDateTime::new(0x6e, 0x08, 0x0c, 0x14, 0x06, 0x1f);

        let string: String = test_struct.to_string();
        assert_eq!(string.as_str(), "2010-08-12T20:06:31+00:00");
    }
}
