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

use keramics_core::ErrorTrace;

/// CD file system (CDFS) date and time value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CdFsDateTime {
    /// Year.
    pub year: u16,

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

    /// Fraction in 100th of a second.
    pub fraction: Option<u8>,

    /// UTC offset in number of 15 minute intervals, where negative values
    /// represent West of UTC.
    pub utc_offset: i8,
}

impl CdFsDateTime {
    /// Creates a new date and time value.
    pub fn new(year: u8, month: u8, day_of_month: u8, hours: u8, minutes: u8, seconds: u8) -> Self {
        Self {
            year: 1900 + year as u16,
            month,
            day_of_month,
            hours,
            minutes,
            seconds,
            fraction: None,
            utc_offset: 0,
        }
    }

    /// Reads a date and time value from a byte sequence.
    pub fn from_bytes(data: &[u8]) -> Self {
        Self {
            year: 1900 + data[0] as u16,
            month: data[1],
            day_of_month: data[2],
            hours: data[3],
            minutes: data[4],
            seconds: data[5],
            fraction: None,
            utc_offset: data[6] as i8,
        }
    }

    /// Reads a date and time value from a string.
    ///
    /// A date and time string is 17 bytes in size and consists of:
    /// * year (4-byte numeric string)
    /// * month (2-byte numeric string),
    /// * day of month (2-byte numeric string),
    /// * hours (2-byte numeric string),
    /// * minutes (2-byte numeric string),
    /// * seconds (2-byte numeric string),
    /// * fraction in 100th of a seconds (2-byte numeric string)
    /// * UTC offset in number of 15 minute intervals (1-byte numeric string,
    ///   where the most significant bit indicates West of UTC when set).
    pub fn from_string(string: &[u8]) -> Result<Self, ErrorTrace> {
        let string: &str = match core::str::from_utf8(string) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert string to UTF-8",
                    error
                ));
            }
        };
        let string_size: usize = string.len();

        if string_size != 17 {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported string size: {}",
                string_size
            )));
        }
        let year: u16 = match u16::from_str_radix(&string[0..4], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert year string to integer",
                    error
                ));
            }
        };
        let month: u8 = match u8::from_str_radix(&string[4..6], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert month string to integer",
                    error
                ));
            }
        };
        let day_of_month: u8 = match u8::from_str_radix(&string[6..8], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert day of month string to integer",
                    error
                ));
            }
        };
        let hours: u8 = match u8::from_str_radix(&string[8..10], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert hours string to integer",
                    error
                ));
            }
        };
        let minutes: u8 = match u8::from_str_radix(&string[10..12], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert minutes string to integer",
                    error
                ));
            }
        };
        let seconds: u8 = match u8::from_str_radix(&string[12..14], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert seconds string to integer",
                    error
                ));
            }
        };
        let fraction: u8 = match u8::from_str_radix(&string[14..16], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert fraction string to integer",
                    error
                ));
            }
        };
        let utc_offset: i8 = match i8::from_str_radix(&string[16..17], 10) {
            Ok(value) => value,
            Err(error) => {
                return Err(keramics_core::error_trace_new_with_error!(
                    "Unable to convert UTC offset string to integer",
                    error
                ));
            }
        };
        Ok(Self {
            year,
            month,
            day_of_month,
            hours,
            minutes,
            seconds,
            fraction: Some(fraction),
            utc_offset,
        })
    }

    /// Determines if the date and time value is set.
    pub fn is_set(&self) -> bool {
        self.year != 0
            || self.month != 0
            || self.day_of_month != 0
            || self.hours != 0
            || self.minutes != 0
            || self.seconds != 0
            || self.fraction.is_some()
            || self.utc_offset != 0
    }

    /// Sets the UTC offset in number of 15 minute intervals, where negative
    /// values represent West of UTC.
    pub fn set_utc_offset(&mut self, utc_offset: i8) {
        self.utc_offset = utc_offset
    }

    /// Retrieves an ISO 8601 string representation of the date and time value.
    pub fn to_iso8601_string(&self) -> String {
        let (time_zone_sign, time_zone_intervals): (char, u8) = if self.utc_offset < 0 {
            ('-', (-(self.utc_offset) as u8))
        } else {
            ('+', self.utc_offset as u8)
        };
        let time_zone_hours: u8 = time_zone_intervals / 4;
        let time_zone_minutes: u8 = (time_zone_intervals % 4) * 15;

        let fraction: String = match &self.fraction {
            Some(fraction) => format!(".{:02}", fraction),
            None => String::new(),
        };
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{}{:02}:{:02}",
            self.year,
            self.month,
            self.day_of_month,
            self.hours,
            self.minutes,
            self.seconds,
            fraction,
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
        assert_eq!(test_struct.year, 2010);
        assert_eq!(test_struct.month, 0x08);
        assert_eq!(test_struct.day_of_month, 0x0c);
        assert_eq!(test_struct.hours, 0x14);
        assert_eq!(test_struct.minutes, 0x06);
        assert_eq!(test_struct.seconds, 0x1f);
        assert_eq!(test_struct.fraction, None);
        assert_eq!(test_struct.utc_offset, 8);
    }

    #[test]
    fn test_cdfs_date_time_from_string() -> Result<(), ErrorTrace> {
        let test_string: String = String::from("20100812200631008");

        let test_struct: CdFsDateTime = CdFsDateTime::from_string(test_string.as_bytes())?;
        assert_eq!(test_struct.year, 2010);
        assert_eq!(test_struct.month, 8);
        assert_eq!(test_struct.day_of_month, 12);
        assert_eq!(test_struct.hours, 20);
        assert_eq!(test_struct.minutes, 6);
        assert_eq!(test_struct.seconds, 31);
        assert_eq!(test_struct.fraction, Some(0));
        assert_eq!(test_struct.utc_offset, 8);

        Ok(())
    }

    #[test]
    fn test_cdfs_date_time_from_string_with_unsupported_string_size() {
        let test_string: [u8; 16] = [
            0x32, 0x30, 0x31, 0x30, 0x30, 0x38, 0x31, 0x32, 0x32, 0x30, 0x30, 0x36, 0x33, 0x31,
            0x30, 0x30,
        ];

        let result: Result<CdFsDateTime, ErrorTrace> = CdFsDateTime::from_string(&test_string);
        assert!(result.is_err());
    }

    #[test]
    fn test_cdfs_date_time_from_string_with_invalid_digit() {
        let test_string: [u8; 17] = [
            0x32, 0x30, 0x31, 0x30, 0x30, 0x38, 0x31, 0x61, 0x32, 0x30, 0x30, 0x36, 0x33, 0x31,
            0x30, 0x30, 0x38,
        ];

        let result: Result<CdFsDateTime, ErrorTrace> = CdFsDateTime::from_string(&test_string);
        assert!(result.is_err());
    }

    #[test]
    fn test_cdfs_date_time_is_set() {
        let test_struct: CdFsDateTime = CdFsDateTime::new(0x6e, 0x08, 0x0c, 0x14, 0x06, 0x1f);

        assert!(test_struct.is_set());

        let test_struct: CdFsDateTime = CdFsDateTime::default();
        assert!(!test_struct.is_set());
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
    fn test_cdfs_date_time_to_iso8601_string_with_fraction() -> Result<(), ErrorTrace> {
        let test_string: String = String::from("20100812200631258");

        let test_struct: CdFsDateTime = CdFsDateTime::from_string(test_string.as_bytes())?;
        let string: String = test_struct.to_iso8601_string();
        assert_eq!(string.as_str(), "2010-08-12T20:06:31.25+02:00");

        Ok(())
    }

    #[test]
    fn test_cdfs_date_time_to_string() {
        let test_struct: CdFsDateTime = CdFsDateTime::new(0x6e, 0x08, 0x0c, 0x14, 0x06, 0x1f);

        let string: String = test_struct.to_string();
        assert_eq!(string.as_str(), "2010-08-12T20:06:31+00:00");
    }
}
