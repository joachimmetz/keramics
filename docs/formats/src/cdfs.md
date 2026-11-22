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

* [Apple Hybrid CD extension](#apple_hybrid_cd_extension), which combines CDFS and [HFS](hfs.md)
* [Joliet](#joliet), which adds Unicode (UCS-2) character support
* [Rock Ridge](#rock_ridge), which adds support for POSIX permissions and overcomes file name
  restrictions
* [El Torito](#el_torito), which adds support for bootable CD-ROMs
* [UDF (Universal Disk Format)](#udf) (or ECMA-167)

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

### File name version suffix

In CDFS file names can have a version suffix:

* ".;\[1-9]\[0-9]\*" version suffix, intended for a file name without an extension, such as
  "README.;1"
* ";\[1-9]\[0-9]\*" version suffix, intended for a file name with an extension, such as
  "README.TXT;1"

Where the version can be 1 through 32767.

The CDFS format allows both "README.TXT;1", "README.TXT.;1", "README.TXT;2" and "RE;1DME.TXT;1" to
exist at the same time and treats them as separate file entries.

> Note that file names of directories do not have a version suffix.

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

> Note that extended attribute here does not refer to xattr type of extended attributes but an
> extension of the file (system) entry attributes.

The extended attribute record (XAR) is of variable size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | | Owner identifier, in little-endian |
| 2 | 2 | | Owner identifier, in big-endian |
| 4 | 2 | | Group identifier, in little-endian |
| 6 | 2 | | Group identifier, in big-endian |
| 8 | 2 | | [Permissions](#permissions) |
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

## Extensions

### Apple Hybrid CD extension {#apple_hybrid_cd_extension}

An Apple Hybrid CD combines an ISO 9660 file system and an [HFS](hfs.md) (or HFS+) file system on
the same optical disc so that macOS systems mount the HFS volume while other operating systems
mount the ISO 9660 volume.

The hybrid layout takes advantage of the unused 16-sector (32 KiB) ISO 9660 system area at the
beginning of the disc:

* Offset `0` (512 bytes) contains an [Apple Partition Map (APM)](apm.md) driver descriptor map
  (signature `"ER"`).
* Subsequent blocks inside the 32 KiB system area contain [APM](apm.md) partition map entries
  (signature `"PM"`), including an `"Apple_HFS"` partition entry that points to the [HFS](hfs.md)
  (or HFS+) volume.
* Offset `32768` (sector 16) begins the standard ISO 9660 volume descriptor set as usual.

In a shared hybrid disc, both the ISO 9660 directory records and the HFS/HFS+ catalog extents
reference the same file data sectors on disc. In a non-shared hybrid disc, each file system stores
its file data in separate regions.

#### Apple ISO 9660 extensions

In addition to hybrid APM/HFS discs, Apple Technical Note FL 36 defines System Use entries stored
at the end of ISO 9660 [directory records](#directory_record) to preserve classic Mac OS Finder
metadata on ISO 9660 volumes:

<!-- typos:disable -->

* The legacy `"BA"` (`0x42, 0x41`) entry precedes SUSP and consists of the 2-byte signature `"BA"`,
  a 1-byte system use identifier (`0x01` for ProDOS, `0x02` for HFS, `0x06` for HFS with icon and
  comment support), followed by HFS Finder information (4-byte file type, 4-byte file creator, and
  2-byte Finder flags).

<!-- typos:enable -->
* The SUSP-compatible `"AA"` (`0x41, 0x41`) entry conforms to the
  [SUSP entry header](#susp_entry_header):

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | "AA" | Signature |
| 2 | 1 | | Entry size |
| 3 | 1 | 2 | Entry version |
| 4 | 1 | | System use identifier (`0x02` for HFS) |
| 5 | 4 | | HFS file type (files only) |
| 9 | 4 | | HFS file creator (files only) |
| 13 | 2 | | HFS Finder flags (at offset 5 for directories) |

> Note that when Apple ISO 9660 extensions are used without an HFS volume, Mac OS resource forks
> are stored as associated ISO 9660 files whose directory records have [file flag](#file_flags)
> `0x04` (is associated file) set.

### Joliet {#joliet}

Joliet is a Microsoft extension to ISO 9660 that provides Unicode file names and relaxes ISO 9660
naming and directory-depth restrictions while remaining backward-compatible with standard ISO 9660
readers.

A Joliet volume adds a [Secondary (Supplementary) Volume Descriptor](#secondary_volume_descriptor)
(type indicator `2`, format version `1`) that references a separate directory hierarchy and path
tables whose strings are encoded in big-endian UCS-2 (UTF-16BE).

#### Joliet escape sequences

A Joliet supplementary volume descriptor is identified by its 32-byte
[escape sequences](#escape_sequences) field at offset 88 (unused trailing bytes are set to `0x00`):

| Value | Identifier | Description |
| --- | --- | --- |
| "\x25\x2f\x40" or "%/@" | UCS-2 Level 1 | Basic Multilingual Plane without combining characters |
| "\x25\x2f\x43" or "%/C" | UCS-2 Level 2 | Basic Multilingual Plane with limited combining characters |
| "\x25\x2f\x45" or "%/E" | UCS-2 Level 3 | Full UCS-2 Basic Multilingual Plane |

#### Joliet naming and layout differences

Compared to the Primary Volume Descriptor hierarchy, a Joliet hierarchy has the following
differences:

* Volume descriptor string fields (such as system identifier, volume identifier, volume set
  identifier, publisher, data preparer, application, copyright, abstract, and bibliographic file
  identifiers) are encoded in big-endian UCS-2 and padded with `0x0020` (for odd-sized 37-byte
  fields, the last byte is `0x00`).
* File and directory names in directory records and path table records are encoded in big-endian
  UCS-2. The Joliet specification allows names up to 64 UCS-2 characters (128 bytes), though some
  authoring tools emit up to 103 or 110 characters within the 255-byte directory record limit.
* All UCS-2 characters are permitted except control characters (`0x0000` to `0x001f`) and `"*"`,
  `"/"`, `":"`, `";"`, `"?"`, and `"\\"`. Lowercase characters, spaces, and multiple dots are
  allowed.
* File names retain the `";1"` version suffix encoded in big-endian UCS-2 (`0x00, 0x3b, 0x00,
  0x31`), while the trailing dot before the version suffix is optional when no extension is
  present.
* The root directory identifier in path tables and directory records remains the single byte `0x00`
  (and the parent directory identifier remains `0x01`).
* Directory hierarchy depth is not limited to 8 levels.

### Rock Ridge {#rock_ridge}

Rock Ridge adds POSIX file system semantics to ISO 9660 volumes through two layered IEEE
specifications:

* **System Use Sharing Protocol (SUSP)** (IEEE P1281), which defines a structured record format
  inside the System Use area at the end of ISO 9660 [directory records](#directory_record) (after
  the file identifier and optional 1-byte alignment padding).
* **Rock Ridge Interchange Protocol (RRIP)** (IEEE P1282), which defines POSIX metadata records on
  top of SUSP.

#### SUSP entry header {#susp_entry_header}

Every SUSP and RRIP entry begins with a 4-byte header:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | | Signature (2-byte ASCII identifier) |
| 2 | 1 | | Entry size, in bytes (including this header) |
| 3 | 1 | 1 | Entry version |
| 4 | ... | | Entry data |

#### SUSP entries

| Value | Identifier | Description |
| --- | --- | --- |
| "CE" | Continuation Area | References a continuation block (logical block location, offset within block, and length in both-byte order) when entries exceed the directory record size |
| "PD" | Padding Field | Unused alignment or reserved padding bytes |
| "SP" | System Use Sharing Protocol Indicator | Stored in the `"."` entry of the root directory; contains check bytes `0xbe, 0xef` and a 1-byte `LEN_SKP` offset skipped before SUSP entries in each directory record |
| "ST" | System Use Sharing Protocol Terminator | Terminates the sequence of System Use entries in a directory record or continuation area |
| "ER" | Extensions Reference | Declares an active extension (for example `"RRIP_1991A"`, `"IEEE_P1282"`, or `"IEEE_1282"`) with identifier, descriptor, and source strings |
| "ES" | Extension Selector | Selects which registered extension subsequent entries belong to (SUSP 1.12) |

#### RRIP entries

<!-- typos:disable -->

| Value | Identifier | Description |
| --- | --- | --- |
| "PX" | POSIX File Attributes | File mode (`st_mode`), link count (`st_nlink`), user identifier (`st_uid`), group identifier (`st_gid`), and optional file serial number (`st_ino`, RRIP 1.12) in both-byte order |
| "PN" | POSIX Device Number | High and low 32-bit device numbers (`dev_t_high`, `dev_t_low`) in both-byte order for block and character special files |
| "SL" | Symbolic Link | Symbolic link target encoded as a sequence of component records (`flags`, `length`, `content`), supporting continuation (`0x01`), current directory (`0x02`), parent directory (`0x04`), and root (`0x08`) |
| "NM" | Alternate Name | POSIX file name supporting mixed case and arbitrary byte sequences, prefixed by a 1-byte flags field (`0x01` continue, `0x02` current, `0x04` parent) |
| "CL" | Child Link | Logical block location (both-byte order) of a relocated directory moved out of a hierarchy deeper than 8 levels |
| "PL" | Parent Link | Logical block location (both-byte order) of the original parent directory of a relocated directory |
| "RE" | Relocated Directory | Marks a placeholder directory (commonly `RR_MOVED`) that stores relocated subdirectories so RRIP-aware readers hide it |
| "TF" | Time Stamp(s) | Stores one or more timestamps selected by a 1-byte flags bitmask |
| "SF" | Sparse File | Virtual file size and table depth for sparse file data |
| "RR" | Rock Ridge Indicator | Deprecated RRIP 1.09 1-byte bitmask indicating which RRIP entries appear in the directory record |

<!-- typos:enable -->

#### POSIX file attributes ("PX") entry

A `"PX"` entry is 36 bytes (RRIP 1.09) or 44 bytes (RRIP 1.12) in size:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | "PX" | Signature |
| 2 | 1 | 36 or 44 | Entry size |
| 3 | 1 | 1 | Entry version |
| 4 | 8 | | POSIX file mode (`st_mode`), in both-byte order |
| 12 | 8 | | POSIX file links (`st_nlink`), in both-byte order |
| 20 | 8 | | POSIX user identifier (`st_uid`), in both-byte order |
| 28 | 8 | | POSIX group identifier (`st_gid`), in both-byte order |
| 36 | 8 | | POSIX file serial number (`st_ino`), in both-byte order (RRIP 1.12 only) |

#### Time stamps ("TF") entry flags

The `"TF"` entry stores a 1-byte flags field at offset 4, followed by one timestamp per set flag bit
in ascending bit order:

| Value | Identifier | Description |
| --- | --- | --- |
| 0x01 | CREATION | Creation time |
| 0x02 | MODIFY | (Last) modification time |
| 0x04 | ACCESS | (Last) access time |
| 0x08 | ATTRIBUTES | (Last) attribute change time |
| 0x10 | BACKUP | (Last) backup time |
| 0x20 | EXPIRATION | Expiration time |
| 0x40 | EFFECTIVE | Effective time |
| 0x80 | LONG_FORM | Uses 17-byte [date and time string](#date_time_string) format when set, or 7-byte [date and time value](#date_time_value) format when clear |

### El Torito {#el_torito}

El Torito is an extension to ISO 9660 that enables optical discs to boot x86 BIOS, PowerPC, Mac,
and UEFI systems.

#### Boot record volume descriptor

An El Torito disc contains a [Boot Record](#boot_record) volume descriptor (type indicator `0`,
signature `"CD001"`, format version `1`) with the following fields:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 0 | [Type indicator](#volume_descriptor_type_indicators) |
| 1 | 5 | "CD001" | Signature |
| 6 | 1 | 1 | Format version |
| 7 | 32 | "EL TORITO SPECIFICATION" | Boot system identifier, padded with 0-byte values |
| 39 | 32 | 0 | Unused (boot identifier) |
| 71 | 4 | | Boot catalog absolute logical sector number (2048-byte sector), in little-endian |
| 75 | 1973 | 0 | Unused |

#### Boot catalog

The boot catalog begins at the sector referenced by the boot record and consists of a sequence of
32-byte entries: a Validation Entry, an Initial/Default Entry, and optional Section Header, Section
Entry, and Section Entry Extension records (for multi-platform or UEFI boot images).

##### Validation entry

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 0x01 | Header identifier |
| 1 | 1 | | [Platform identifier](#el_torito_platform_identifiers) |
| 2 | 2 | 0 | Reserved |
| 4 | 24 | | Identifier string (manufacturer or developer) |
| 28 | 2 | | Checksum word, in little-endian (sum of all sixteen 16-bit little-endian words in the entry must equal `0x0000`) |
| 30 | 2 | "\x55\xaa" | Key bytes |

##### Initial/Default entry

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | | Boot indicator (`0x88` = bootable, `0x00` = not bootable) |
| 1 | 1 | | [Boot media type](#el_torito_boot_media_types) |
| 2 | 2 | | Load segment, in little-endian (`0x0000` uses default segment `0x07c0`) |
| 4 | 1 | | System type (copy of the partition type byte from the boot image's MBR partition table) |
| 5 | 1 | 0 | Unused |
| 6 | 2 | | Sector count (number of 512-byte virtual sectors to load), in little-endian |
| 8 | 4 | | Load RBA (starting 2048-byte logical sector of the boot image), in little-endian |
| 12 | 20 | 0 | Unused |

##### Section header entry

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | | Header indicator (`0x90` = more section headers follow, `0x91` = final section header) |
| 1 | 1 | | [Platform identifier](#el_torito_platform_identifiers) |
| 2 | 2 | | Number of section entries following this header, in little-endian |
| 4 | 28 | | Section identifier string |

Each following Section Entry uses the same layout as the Initial/Default Entry for offsets 0
through 11, with a 1-byte selection criteria type at offset 12 and 19 bytes of vendor-unique
selection criteria at offsets 13 through 31. Additional selection criteria can follow in 32-byte
Section Entry Extensions (header indicator `0x44`).

##### Platform identifiers {#el_torito_platform_identifiers}

| Value | Identifier | Description |
| --- | --- | --- |
| 0x00 | | 80x86 |
| 0x01 | | PowerPC |
| 0x02 | | Mac |
| 0xef | | EFI (UEFI) |

##### Boot media types {#el_torito_boot_media_types}

| Value | Identifier | Description |
| --- | --- | --- |
| 0x00 | | No emulation |
| 0x01 | | 1.2 MiB floppy disk emulation |
| 0x02 | | 1.44 MiB floppy disk emulation |
| 0x03 | | 2.88 MiB floppy disk emulation |
| 0x04 | | Hard disk emulation |

### UDF (Universal Disk Format) {#udf}

Universal Disk Format (UDF) is a file system specification maintained by the Optical Storage
Technology Association (OSTA) as a profile of ISO/IEC 13346 (ECMA-167), with revisions 1.02, 1.50,
2.00, 2.01, 2.50, and 2.60. UDF replaces ISO 9660 for DVD, Blu-ray, and rewritable optical media,
removing the 4 GiB file size limit and adding Unicode names, sparse and inline files, metadata
partitions, and packet-written update support.

#### ISO 9660 / UDF Bridge and Volume Recognition Sequence

Optical discs frequently contain both ISO 9660 and UDF structures on the same medium (an ISO 9660 /
UDF Bridge disc). After the 32 KiB system area (sectors 0 to 15) and the ISO 9660 volume descriptor
set (ending with the [volume descriptor set terminator](#volume_descriptor_set_terminator)), the
ECMA-167 Volume Recognition Sequence (VRS) appears as a contiguous series of 2048-byte descriptors
using the same header layout (`type`, 5-byte `signature`, `version`):

| Value | Identifier | Description |
| --- | --- | --- |
| "BEA01" | Beginning Extended Area Descriptor | Marks the start of the Volume Recognition Sequence |
| "NSR02" | Volume Structure Descriptor (ECMA-167 2nd edition) | Identifies UDF 1.02 or 1.50 |
| "NSR03" | Volume Structure Descriptor (ECMA-167 3rd edition) | Identifies UDF 2.00, 2.01, 2.50, or 2.60 |
| "BOOT2" | Boot Descriptor | Optional ECMA-167 boot descriptor |
| "TEA01" | Terminating Extended Area Descriptor | Marks the end of the Volume Recognition Sequence |

#### Descriptor tag

All UDF volume, partition, and file system descriptors are little-endian and begin with a 16-byte
descriptor tag:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | | [Tag identifier](#udf_tag_identifiers) |
| 2 | 2 | | Descriptor version (2 for ECMA-167 2nd edition, 3 for 3rd edition) |
| 4 | 1 | | Tag checksum (8-bit sum of bytes 0..3 and 5..15) |
| 5 | 1 | 0 | Reserved |
| 6 | 2 | | Tag serial number |
| 8 | 2 | | Descriptor CRC (CRC-ITU-T / CCITT of descriptor body) |
| 10 | 2 | | Descriptor CRC length |
| 12 | 4 | | Tag location (logical sector or logical block number of this descriptor) |

#### Core UDF descriptor structures {#udf_tag_identifiers}

A UDF reader locates the Anchor Volume Descriptor Pointer (AVDP) at logical sector 256 (with
backup copies at the last sector `N` and/or `N - 256`), follows it to the Main (or Reserve) Volume
Descriptor Sequence (VDS), resolves the Logical Volume Descriptor and Partition Descriptor(s), and
then reads the File Set Descriptor to locate the root directory Information Control Block (ICB):

| Value | Identifier | Description |
| --- | --- | --- |
| 1 | Primary Volume Descriptor | Volume identifier, volume sequence number, and recording timestamp |
| 2 | Anchor Volume Descriptor Pointer (AVDP) | Points to the Main and Reserve Volume Descriptor Sequences |
| 4 | Implementation Use Volume Descriptor | Implementation-specific volume metadata (`*UDF LV Info`) |
| 5 | Partition Descriptor | Partition starting sector, length, partition number, and access type |
| 6 | Logical Volume Descriptor | Logical block size, logical volume identifier, partition maps (Type 1 physical, Type 2 virtual/sparable/metadata), and File Set Descriptor location |
| 7 | Unallocated Space Descriptor | Extents of unallocated space |
| 8 | Terminating Descriptor | Marks the end of a Volume Descriptor Sequence |
| 9 | Logical Volume Integrity Descriptor | Integrity type (open or closed), free space, file and directory counts, and minimum/maximum UDF revision |
| 256 | File Set Descriptor | Located inside the partition; points to the root directory ICB |
| 257 | File Identifier Descriptor (FID) | Directory entry storing file characteristics flags (hidden, directory, deleted, parent), file identifier, and target ICB |
| 261 | File Entry | File metadata (timestamps, permissions, UID/GID, link count, size, allocation descriptors or inline data) in UDF 1.02 and 1.50 |
| 266 | Extended File Entry | Extends File Entry with creation time, object size, and stream directory ICB for named streams and extended attributes in UDF 2.00 and later |

> Note that UDF stores character strings in OSTA Compressed Unicode (CS0), where the first byte is
> a compression identifier: `0x08` indicates 8-bit ISO-8859-1 characters and `0x10` indicates
> 16-bit big-endian UCS-2 characters.

## References

* [ECMA-119: Volume and file structure of CD-ROM for information interchange](https://ecma-international.org/wp-content/uploads/ECMA-119_6th_edition_december_2025.pdf)
* [Apple Technical Note FL 36: Apple Extensions to ISO 9660](https://developer.apple.com/library/archive/documentation/mac/Files/Files-302.html)
* [Joliet (file system)](https://en.wikipedia.org/wiki/Joliet_(file_system)), by Wikipedia
* [IEEE P1281: System Use Sharing Protocol (SUSP) Version 1.12](https://web.archive.org/web/20170404043400/http://www.ymi.com/ymi/sites/default/files/pdf/System%20Use%20Sharing%20Protocol.pdf)
* [IEEE P1282: Rock Ridge Interchange Protocol (RRIP) Version 1.12](https://web.archive.org/web/20170404043401/http://www.ymi.com/ymi/sites/default/files/pdf/Rock%20Ridge.pdf)
* ["El Torito" Bootable CD-ROM Format Specification Version 1.0](https://pdos.csail.mit.edu/6.828/2018/readings/boot-cdrom.pdf)
* [ECMA-167: Volume and File Structure for Write-Once and Rewritable Media using Non-Sequential Recording](https://ecma-international.org/publications-and-standards/standards/ecma-167/)
* [OSTA Universal Disk Format (UDF) Specification Revision 2.60](https://www.osta.org/specs/pdf/udf260.pdf)
