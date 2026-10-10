# CD file system (CDFS) format

The CD file system (CDFS) format, also known as ISO 9660 (or ECMA-119), is a file system primarily
used on optical discs, such as CD-ROMs. Drafts of ISO 9660 are sometimes referred to
as "High Sierra".

ISO 9660 defines the following interchange levels:

* level 1
  * each file consists of a single file section
  * file names are limited to 8.3 characters
* level 2
  * file names are limited to 31 characters
* level 3, which supports file fragmentation
  * files may consist of multiple file sections (or file fragmentation)

There are various extensions to ISO 9660, namely:

* Apple Hybrid CD extension, which combines CDFS and [HFS](hfs.md)
* Joliet, which adds Unicode (UCS-2) character support
* Rock Ridge, which adds support for POSIX permissions and overcome file name restrictions
* El Torito, which adds support for bootable CD-ROMs
* UDF (Universal Disk Format) (or ECMA-167)

## Overview

A CDFS volume consists of:

* 16 sector (32 KiB) system area, which is not used by ISO 9660 but can be used by extensions, such
  as El Torito
* Data area
  * Volume descriptor set
  * Path tables
    * one or more little-endian (type L) path tables
    * one or more big-endian (type M) path tables
  * directories and files

### Characteristics

| Characteristics | Description |
| --- | --- |
| Byte order | Combined big- and little-endian (or "both-byte" order) |
| Date and time values | A 17-byte date and time string or a 7-byte date and time value, in local time with a UTC offset |
| Character strings | Format specific |

The number of bytes per sector (or logical block) is typically 2048.

## Data types

### Alphabetical character set {#a_character_set}

The alphabetical character set (or a-characters set) consists of 57 specific ASCII characters
used for description strings within volume descriptors.

The a-character set is the following subset of ASCII characters:

* " " (0x20)
* "!" (0x21)
* '"' (0x22)
* "%" (0x25)
* "&" (0x26)
* "'" (0x27)
* "(" (0x28)
* ")" (0x29)
* "\*" (0x2a)
* "+" (0x2b)
* "," (0x2c)
* "-" (0x2d)
* "." (0x2e)
* "/" (0x2f)
* "[0-9]" ([0x30-0x39])
* ":" (0x3a)
* ";" (0x3b)
* "<" (0x3c)
* "=" (0x3d)
* ">" (0x3e)
* "?" (0x3f)
* "[A-Z]" ([0x41-0x5a])
* "\_" (0x5f)

### Coded graphic character set {#c_character_set}

The coded graphic character set (or c-character set) is the set of characters of an implementation
defined 8-bit coded character set, designated by the [escape sequences](#escape_sequences) in a
supplementary volume descriptor. It is used to interpret the descriptor fields that contain
characters within the directory hierarchy identified by that volume descriptor.

> Note that if the escape sequences field is not set (contains 0-byte values), the c-character set
> defaults to the [a-character set](#a_character_set) and the [d-character set](#d_character_set).

### Escape sequences {#escape_sequences}

The escape sequences are one or more escape sequences designated according to ECMA-35, that
designate the G0 graphic character set and, optionally, the G1 graphic character set to be used to
interpret descriptor fields that contain characters. Each escape sequence is stored with the escape
character omitted, and any unused byte positions are set to 0-byte values.

TODO: describe G0 and G1 graphic character sets.

### Data character set {#d_character_set}

The data character set (or d-character set) consists of 38 specific ASCII characters used for
standard file and directory names.

The d-character set is the following subset of ASCII characters:

* "[0-9]" ([0x30-0x39])
* "[A-Z]" ([0x41-0x5a])
* "\_" (0x5f)

### Date and time string {#date_time_string}

A date and time string is 17 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Year, as numeric string, starting with 1 |
| 4 | 2 | | Month, as numeric string, starting with 1 |
| 6 | 2 | | Day of month, as numeric string, starting with 1 |
| 8 | 2 | | Hours, as numeric string, starting with 0 |
| 10 | 2 | | Minutes, as numeric string, starting with 0 |
| 12 | 2 | | Seconds, as numeric string, starting with 0 |
| 14 | 2 | | Fraction in 100th of a seconds, as numeric string |
| 16 | 1 | | UTC offset in number of 15 minute intervals, as numeric string, where negative values represent West of UTC |

> Note that if all characters are the digit zero (or "0"), the date and time are not specified.

### Date and time value {#date_time_value}

A date and time value is 7 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | | Year, where 0 represents 1900 |
| 1 | 1 | | Month, starting with 1 |
| 2 | 1 | | Day of month, starting with 1 |
| 3 | 1 | | Hours |
| 4 | 1 | | Minutes |
| 5 | 1 | | Seconds |
| 6 | 1 | | UTC offset in number of 15 minute intervals, where negative values represent West of UTC |

> Note that if all values are zero, the date and time are not specified.

## Volume descriptor set

The volume descriptor set consists of:

* one or more volume descriptor, which must contain a primary volume descriptor
* [volume descriptor set terminator](#volume_descriptor_set_terminator)

### Volume descriptor

A volume descriptor is 2048 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | 1 | Format version |
| 7 | 2041 | | Data |

#### Type indicators {#volume_descriptor_type_indicators}

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | | [Boot record](#boot_record) |
| 1 | | [Primary volume descriptor](#primary_volume_descriptor) |
| 2 | | [Secondary (supplemental or enhanced) volume descriptor](#secondary_volume_descriptor) |
| 3 | | [Volume partition descriptor](#volume_partition_descriptor) |
| | | |
| 255 | | [Volume descriptor set terminator](#volume_descriptor_set_terminator) |

Per ECMA-119 values 4 to 254 are reserved.

### Boot record {#boot_record}

A boot record (volume descriptor) is 2048 bytes in size and consists of:

<!-- rumdl-disable MD033 MD056 -->

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 0 | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | 1 | Format version |
| <td colspan="4">*Boot record*</td> |
| 7 | 32 | | Boot system identifier, string using the [a-character set](#a_character_set) |
| 39 | 32 | | Boot identifier, string using the [a-character set](#a_character_set) |
| 71 | 1977 | | Boot system data |

<!-- rumdl-enable MD033 MD056 -->

### Primary volume descriptor {#primary_volume_descriptor}

A primary volume descriptor (PVD) is 2048 bytes in size and consists of:

<!-- rumdl-disable MD033 MD056 -->

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 1 | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | 1 | Format version |
| <td colspan="4">*Primary volume descriptor*</td> |
| 7 | 1 | 0 | Unknown (unused) |
| 8 | 32 | | System identifier, string using the [a-character set](#a_character_set) |
| 40 | 32 | | Volume identifier, string using the [d-character set](#d_character_set) |
| 72 | 8 | 0 | Unknown (unused) |
| 80 | 4 | | Volume size, in little-endian |
| 84 | 4 | | Volume size, in big-endian |
| 88 | 32 | 0 | Unknown (unused) |
| 120 | 2 | | Number of volumes in set, in little-endian |
| 122 | 2 | | Number of volumes in set, in big-endian |
| 124 | 2 | | Volume set index (or volume sequence number), in little-endian |
| 126 | 2 | | Volume set index (or volume sequence number), in big-endian |
| 128 | 2 | | (Logical) block size, in little-endian |
| 130 | 2 | | (Logical) block size, in big-endian |
| 132 | 4 | | Path table size, in little-endian |
| 136 | 4 | | Path table size, in big-endian |
| 140 | 4 | | Little-endian (type L (LSB)) path table start sector |
| 144 | 4 | | Little-endian (type L (LSB)) optional path table start sector, contains 0 if not set |
| 148 | 4 | | Big-endian (type M (MSB)) path table start sector |
| 152 | 4 | | Big-endian (type M (MSB)) optional path table start sector, contains 0 if not set |
| 156 | 34 | | Root [directory record](#directory_record) |
| 190 | 128 | | Volume set identifier, string using the [d-character set](#d_character_set) |
| 318 | 128 | | Publisher identifier, string using the [a-character set](#a_character_set) |
| 446 | 128 | | Data preparer identifier, string using the [a-character set](#a_character_set) |
| 574 | 128 | | Application identifier, string using the [a-character set](#a_character_set) |
| 702 | 37 | | Copyright file name (or identifier), which contains a string using the [d-character set](#d_character_set) with separator 1 and 2 |
| 739 | 37 | | Abstract file name (or identifier), which contains a string using the [d-character set](#d_character_set) with separator 1 and 2 |
| 776 | 37 | | Bibliographic file name (or identifier), which contains a string using the [d-character set](#d_character_set) with separator 1 and 2 |
| 813 | 17 | | Volume creation [time](#date_time_string) |
| 830 | 17 | | Volume (last) modification [time](#date_time_string) |
| 847 | 17 | | Volume expiration [time](#date_time_string) |
| 864 | 17 | | Volume effective [time](#date_time_string) |
| 881 | 1 | 1 | File structure version |
| 882 | 1 | 0 | Unknown (reserved) |
| 883 | 512 | | Unknown (application use) |
| 1395 | 653 | 0 | Unknown (reserved) |

<!-- rumdl-enable MD033 MD056 -->

> Note that the optional path tables can be used for multiple purposes.

### Secondary volume descriptor {#secondary_volume_descriptor}

TODO: describe differences Supplementary Volume Descriptors (SVD) and Enhanced Volume Descriptors
(EVD)

TODO: describe non-standard a1 and d1 character sets

A secondary volume descriptor is 2048 bytes in size and consists of:

<!-- rumdl-disable MD033 MD056 -->

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 2 | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | | Format version, where 1 represents a supplementary volume descriptor and 2 a enhanced volume descriptor |
| <td colspan="4">*Secondary volume descriptor*</td> |
| 7 | 1 | | [Volume flags](#volume_flags) |
| 8 | 32 | | System identifier, string using the [a-character set](#a_character_set) |
| 40 | 32 | | Volume identifier, string using the [d-character set](#d_character_set) |
| 72 | 8 | 0 | Unknown (unused) |
| 80 | 4 | | Volume size, in little-endian |
| 84 | 4 | | Volume size, in big-endian |
| 88 | 32 | 0 | Unknown (unused) |
| 120 | 2 | | Number of volumes in set, in little-endian |
| 122 | 2 | | Number of volumes in set, in big-endian |
| 124 | 2 | | Volume set index (or volume sequence number), in little-endian |
| 126 | 2 | | Volume set index (or volume sequence number), in big-endian |
| 128 | 2 | | (Logical) block size, in little-endian |
| 130 | 2 | | (Logical) block size, in big-endian |
| 132 | 4 | | Path table size, in little-endian |
| 136 | 4 | | Path table size, in big-endian |
| 140 | 4 | | Little-endian (type L (LSB)) path table start sector |
| 144 | 4 | | Little-endian (type L (LSB)) optional path table start sector, contains 0 if not set |
| 148 | 4 | | Big-endian (type M (MSB)) path table start sector |
| 152 | 4 | | Big-endian (type M (MSB)) optional path table start sector, contains 0 if not set |
| 156 | 34 | | Root [directory record](#directory_record) |
| 190 | 128 | | Volume set identifier, string using the [d-character set](#d_character_set) |
| 318 | 128 | | Publisher identifier, string using the [a-character set](#a_character_set) |
| 446 | 128 | | Data preparer identifier, string using the [a-character set](#a_character_set) |
| 574 | 128 | | Application identifier, string using the [a-character set](#a_character_set) |
| 702 | 37 | | Copyright file name (or identifier), which contains a string using the [a-character set](#a_character_set) with separator 1 and 2 |
| 739 | 37 | | Abstract file name (or identifier), which contains a string using the [a-character set](#a_character_set) with separator 1 and 2 |
| 776 | 37 | | Bibliographic file name (or identifier), which contains a string using the [a-character set](#a_character_set) with separator 1 and 2 |
| 813 | 17 | | Volume creation [time](#date_time_value) |
| 830 | 17 | | Volume (last) modification [time](#date_time_value) |
| 847 | 17 | | Volume expiration [time](#date_time_value) |
| 864 | 17 | | Volume effective [time](#date_time_value) |
| 881 | 1 | | File structure version |
| 882 | 1 | 0 | Unknown (reserved) |
| 883 | 512 | | Unknown (application use) |
| 1395 | 653 | 0 | Unknown (reserved) |

<!-- rumdl-enable MD033 MD056 -->

#### Volume flags {#volume_flags}

| Value | Identifier | Description |
| --- | --- | --- |
| 0x01 | | Uses non-standard character set |

### Volume partition descriptor {#volume_partition_descriptor}

A volume partition descriptor is 2048 bytes in size and consists of:

<!-- rumdl-disable MD033 MD056 -->

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 3 | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | 1 | Format version |
| <td colspan="4">*Volume partition descriptor*</td> |
| 7 | 1 | 0 | Unknown (unused) |
| 8 | 32 | | System identifier, string using the [a-character set](#a_character_set) |
| 40 | 32 | | Volume identifier, string using the [d-character set](#d_character_set) |
| 72 | 4 | | Volume partition offset, in little-endian |
| 76 | 4 | | Volume partition offset, in big-endian |
| 80 | 4 | | Volume partition size, in little-endian |
| 84 | 4 | | Volume partition size, in big-endian |
| 88 | 1960 | | Unknown (system use) |

<!-- rumdl-enable MD033 MD056 -->

### Volume descriptor set terminator {#volume_descriptor_set_terminator}

A volume descriptor set terminator is 2048 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 255 | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | 1 | Format version |
| 7 | 2041 | 0 | Unknown (reserved) |

## Path table

The path table consists of:

* one or more path table records

### Path table record

The path table record is of variable size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | | Name (or directory (file) identifier) size |
| 1 | 1 | | Extended attribute record size |
| 2 | 4 | | Directory start sector |
| 6 | 2 | | Parent directory number |
| 8 | ... | | Name or (Directory (file) identifier) |
| ... | ... | | Unknown (padding) |

> Note that the root directory is identified by a name (file identifier) consisting of a 0-byte
> value.

## Directories and files

### Directory record {#directory_record}

The directory record is of variable size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | | Record size |
| 1 | 1 | | Extended attribute record size |
| 2 | 4 | | Data start sector, in little-endian |
| 6 | 4 | | Data start sector, in big-endian |
| 10 | 4 | | Data size, in little-endian |
| 14 | 4 | | Data size, in big-endian |
| 18 | 7 | | [Recording time](#date_time_value) |
| 25 | 1 | | [File flags](#file_flags) |
| 26 | 1 | | File unit size |
| 27 | 1 | | Interleave gap size |
| 28 | 2 | | Volume set index (or volume sequence number), in little-endian |
| 30 | 2 | | Volume set index (or volume sequence number), in big-endian |
| 32 | 1 | | Name (or file identifier) size |
| 33 | ... | | Name (or file identifier) |
| ... | ... | | Unknown (alignment padding) |
| ... | ... | | Unknown (system use) |

#### File flags {#file_flags}

| Value | Identifier | Description |
| --- | --- | --- |
| 0x00000001 | | Is hidden (or existence) |
| 0x00000002 | | Is directory |
| 0x00000004 | | Is associated file |
| 0x00000008 | | Has associated extended attribute record |
| 0x00000010 | | Has owner/group identifier (or protection) |
| 0x00000020 | | Unknown (reserved) |
| 0x00000040 | | Unknown (reserved) |
| 0x00000080 | | Continuation (or next-extent) flag, contains 0 if last directory record |

### Extended attribute record

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | | Owner identifier, in little-endian |
| 2 | 2 | | Owner identifier, in big-endian |
| 4 | 2 | | Group identifier, in little-endian |
| 6 | 2 | | Group identifier, in big-endian |
| 8 | 2 | | Permissions |
| 10 | 17 | | Creation [time](#date_time_string) |
| 27 | 17 | | (Last) modification [time](#date_time_string) |
| 44 | 17 | | Expiration [time](#date_time_string) |
| 61 | 17 | | Effective [time](#date_time_string) |
| 78 | 1 | | [Record format](#record_format) |
| 79 | 1 | | Record attributes |
| 80 | 4 | | Record size |
| 84 | 32 | | System identifier |
| 116 | 64 | | Unknown |
| 180 | 1 | | Extended attribute record (format) version |
| 181 | 1 | | Escape sequences size |
| 182 | 64 | | Unknown (reserved) |
| 246 | 4 | | Application use size |
| 250 | ... | | Application use |
| ... | ... | | Escape sequences |

#### Permissions {#permissions}

| Value | Identifier | Description |
| --- | --- | --- |
| 0x0001 | | System owner read |
| 0x0002 | | Should be set to 1 |
| 0x0004 | | System owner execute |
| 0x0008 | | Should be set to 1 |
| 0x0010 | | Owner read |
| 0x0020 | | Should be set to 1 |
| 0x0040 | | Owner execute |
| 0x0080 | | Should be set to 1 |
| 0x0100 | | Group read |
| 0x0200 | | Should be set to 1 |
| 0x0400 | | Group execute |
| 0x0800 | | Should be set to 1 |
| 0x1000 | | Group read |
| 0x2000 | | Should be set to 1 |
| 0x4000 | | Group execute |
| 0x8000 | | Should be set to 1 |

#### Record format {#record_format}

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | | Structure of the information in the file is not specified |
| 1 | | A sequence of fixed-length records, where the record size is specified by the record size field |
| 2 | | A sequence of variable-length records where the record control word (or RCW) is recorded least significant byte first |
| 3 | | A sequence of variable-length records where the record control word (or RCW) is recorded most significant byte first |
| | | |
| 4 - 127 | | Unknown (reserved) |
| 128 - 255 | | Reserved for system use |

#### Record attributes {#record_attributes}

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | | Each record is preceded by a line feed character and followed by a carriage return character |
| 1 | | The first byte of a record is interpreted for vertical spacing |
| 2 | | The record contains the necessary control information |
| | | |
| 3 - 255 | | Unknown (reserved) |

> Note that if the [record format](#record_format) is 0, the record attributes are ignored in
> interchange.

## References

* [ECMA-119: Volume and file structure of CD-ROM for information interchange](https://ecma-international.org/wp-content/uploads/ECMA-119_6th_edition_december_2025.pdf)
