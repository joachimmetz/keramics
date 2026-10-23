# ZIP archive format

The ZIP archive format is a widely used archive format that stores one or more files, either
compressed or stored (uncompressed), together with the metadata needed to describe and extract
them. It was originally created by PKWARE, whose `PKZIP` and `PKUnZIP` programs produced and
consumed the format, and the records are identified by a signature that begins with the two byte
"PK" marker, an abbreviation of the author's name.

The format is used on its own, commonly with the file extension ".zip", and is also used as a
container by a large number of application formats, including Java ".jar" and ".war" archives and
the Office Open XML and OpenDocument file formats, such as ".docx", ".xlsx", ".pptx", ".odt",
".ods". Programs reading or writing ZIP archive files should rely on the internal record signatures
to identify files in the format, rather than on the file extension.

## Overview

A ZIP archive file is a sequence of records, in which each record is identified by a 4 byte
signature. The signature values begin with the two byte constant marker 0x4b50 (the characters
"PK"), so the records of a ZIP archive file can be located by looking for this marker.

A ZIP archive file consists of:

* zero or more file records, each of which consists of:
  * a local file header
  * optionally, an encryption header
  * file data
  * optionally, a data descriptor
* optionally, an archive decryption header, see [strong encryption](#strong_encryption)
* optionally, an archive extra data record, see [strong encryption](#strong_encryption)
* a central directory, consisting of:
  * one or more central directory headers, see [central directory header](#central_directory_header)
  * optionally, a cryptographic signature, see [cryptographic signature](#cryptographic_signature)
* optionally, a ZIP64 end of central directory record, see
  [ZIP64 end of central directory record](#zip64_eocd_record)
* optionally, a ZIP64 end of central directory locator, see
  [ZIP64 end of central directory locator](#zip64_eocd_locator)
* an end of central directory record, see [end of central directory record](#eocd_record)

A ZIP archive file must contain an end of central directory record, and at most one. A ZIP archive
file consisting of only an end of central directory record is an empty ZIP archive file. The
records of the central directory may be in a different order than the order in which the files
appear in the file. All multi-byte numeric values are stored in little-endian byte order, unless
explicitly stated otherwise for a specific data element.

> Note that compression and encryption are applied to the file data and the central directory,
> in the case of strong encryption, only. The local file headers, the encryption headers and the
> end of central directory record are not compressed.

### Characteristics

| Characteristics | Description |
| --- | --- |
| Byte order | little-endian |
| Date and time values | |
| Character strings | |

## Local file header

Every file stored in a ZIP archive file is preceded by a local file header for that file. The local
file header is 30 bytes plus the size of the variable length file name and extra field, and consists
of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x03\x04" | Signature |
| 4 | 2 | | Version needed to extract, see [version needed to extract](#version_needed_to_extract) |
| 6 | 2 | | [General purpose bit flag](#general_purpose_bit_flag) |
| 8 | 2 | | [Compression method](#compression_method) |
| 10 | 2 | | Last modification file time, in DOS time format |
| 12 | 2 | | Last modification file date, in DOS date format |
| 14 | 4 | | CRC-32 of the uncompressed file data, see [CRC-32](#crc_32) |
| 18 | 4 | | Compressed file size |
| 22 | 4 | | Uncompressed file size |
| 26 | 2 | | File name length, which is the size in bytes of the file name |
| 28 | 2 | | Extra field length, which is the size in bytes of the extra field |
| 30 | ... | | File name, see [file name](#file_name) |
| ... | ... | | Extra field, see [extra field](#extra_field) |

The file name and the extra field follow the fixed 30 byte prefix of the header, their sizes in
bytes are given by the file name length and the extra field length, respectively. The string
fields are not null terminated, since their length is given explicitly.

> Note that where bit 3 of the general purpose bit flag is set, the CRC-32, compressed size and
> uncompressed size fields are set to zero in the local file header. The correct values are then
> put in the data descriptor immediately following the compressed data, see
> [data descriptor](#data_descriptor), and in the central directory.

## Encryption header

Where a file is encrypted, an encryption header for the file is placed after the local file
header and before the file data. The size of the encryption header depends on the encryption
method used. For traditional PKWARE encryption the encryption header is 12 bytes, which
consist of the random data used to further initialize the encryption keys.

> Note that if an encryption header is present, the compressed file size of the file includes the
> bytes of the encryption header.

## File data

The file data is the compressed or stored (uncompressed) data of the file. The file data
immediately follows the local file header (and the encryption header, if the file is
encrypted). Zero byte files, directory entries and other entries that contain no data do not
include file data.

## Data descriptor {#data_descriptor}

Where bit 3 of the general purpose bit flag is set, the file data is followed by a data
descriptor. The data descriptor is used to facilitate streaming of a ZIP archive file, when it was
not possible to seek in the output file (for example, when writing to standard output or to a
non-seekable device), and it exists only if bit 3 is set. It consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x07\x08" | Signature |
| 4 | 4 | | CRC-32 of the uncompressed file data |
| 8 | 4 | | Compressed file size |
| 12 | 4 | | Uncompressed file size |

### Original format data descriptor

The signature is not specified in the original format, but the value 0x08074b50 (the characters
"PK\x07\x08") has commonly been adopted as the signature value for the data descriptor, and is
included here. When the signature is present the fields follow the signature; when it is absent the
fields directly follow the last byte of the compressed data.

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | CRC-32 of the uncompressed file data |
| 4 | 4 | | Compressed file size |
| 8 | 4 | | Uncompressed file size |

### ZIP64 data descriptor

For archives in the ZIP64 format, where the data descriptor is present, the compressed and
uncompressed sizes are stored as 8 byte values instead of 4 byte values.

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x07\x08" | Signature |
| 4 | 4 | | CRC-32 of the uncompressed file data |
| 8 | 8 | | Compressed file size |
| 16 | 8 | | Uncompressed file size |

## Central directory header {#central_directory_header}

Each local file header is accompanied by a corresponding central directory header in the central
directory section of the file. The central directory is at the end of the file, and provides the
index of the entries in the archive. A central directory header is 46 bytes plus the sizes of the
variable length file name, extra field and file comment, and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x01\x02" | Signature |
| 4 | 2 | | Version made by, see [version made by](#version_made_by) |
| 6 | 2 | | Version needed to extract, see [version needed to extract](#version_needed_to_extract) |
| 8 | 2 | | [General purpose bit flag](#general_purpose_bit_flag) |
| 10 | 2 | | [Compression method](#compression_method) |
| 12 | 2 | | Last modification file time, in DOS time format |
| 14 | 2 | | Last modification file date, in DOS date format |
| 16 | 4 | | CRC-32 of the uncompressed file data |
| 20 | 4 | | Compressed file size |
| 24 | 4 | | Uncompressed file size |
| 28 | 2 | | File name length |
| 30 | 2 | | Extra field length |
| 32 | 2 | | File comment length, which is the size in bytes of the file comment |
| 34 | 2 | | Disk number start, which is the number of the disk on which the file begins |
| 36 | 2 | | Internal file attributes |
| 38 | 4 | | External file attributes, which are host system dependent |
| 42 | 4 | | Relative offset of the local file header from the start of the first disk |
| 46 | ... | | File name |
| ... | ... | | Extra field, see [extra field](#extra_field) |
| ... | ... | | File comment |

The relative offset of the local file header is the offset from the start of the first disk on
which this file appears to where the local file header is found. It is the field a reader would
normally use to locate the local file header and the file data of an entry.

> Note that the entries in the central directory may not be in the same order that the files
> appear in the file. A reader should not assume the local file headers are located in the same
> order as the central directory headers.

## Cryptographic signature {#cryptographic_signature}

Optionally, a cryptographic (or digital) signature for the archive follows the central directory
headers. It consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x05\x05" | Signature |
| 4 | 2 | | Size of the cryptographic signature data |
| 6 | ... | | Cryptographic signature data |

With the central directory encryption feature, the central directory structure may be stored both
compressed and encrypted. When the central directory structure is encrypted, the digital
signature record is neither compressed nor encrypted.

## ZIP64 end of central directory record {#zip64_eocd_record}

For archives in the ZIP64 format, a ZIP64 end of central directory record is present. It is used
where one of the fields of the ordinary end of central directory record is too small to hold the
required data, and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x06\x06" | Signature |
| 4 | 8 | | Size of the ZIP64 end of central directory record, which does not include the leading 12 bytes |
| 12 | 2 | | Version made by |
| 14 | 2 | | Version needed to extract |
| 16 | 4 | | Number of this disk |
| 20 | 4 | | Number of the disk with the start of the central directory |
| 24 | 8 | | Total number of entries in the central directory on this disk |
| 32 | 8 | | Total number of entries in the central directory |
| 40 | 8 | | Size of the central directory |
| 48 | 8 | | Offset of the start of the central directory with respect to the starting disk number |
| 56 | ... | | ZIP64 extensible data sector (variable size) |

When a field in the end of central directory record is set to its maximum value (0xffff or
0xffffffff), the corresponding value is stored in the ZIP64 end of central directory record
instead. Fields of the ZIP64 extensible data sector are identified by a header ID (2 bytes)
followed by a data size (4 bytes).

## ZIP64 end of central directory locator {#zip64_eocd_locator}

The ZIP64 end of central directory locator record is present when a ZIP64 end of central
directory record is present. It indicates the location of the ZIP64 end of central directory
record, and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x06\x07" | Signature (0x07064b50), which identifies the start of the ZIP64 end of central directory locator |
| 4 | 4 | | Number of the disk with the start of the ZIP64 end of central directory record |
| 8 | 8 | | Relative offset of the ZIP64 end of central directory record |
| 16 | 4 | | Total number of disks |

The ZIP64 end of central directory locator record and the end of central directory record must
reside on the same disk when splitting or spanning an archive.

## End of central directory record {#eocd_record}

Every ZIP archive file ends with an end of central directory record, which points back to the
central directory and contains summary information about the archive. It consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "PK\x05\x06" | Signature (0x06054b50), which identifies the start of the end of central directory record |
| 4 | 2 | | Number of this disk |
| 6 | 2 | | Number of the disk with the start of the central directory |
| 8 | 2 | | Total number of entries in the central directory on this disk |
| 10 | 2 | | Total number of entries in the central directory |
| 12 | 4 | | Size of the central directory |
| 16 | 4 | | Offset of the start of the central directory with respect to the starting disk number |
| 20 | 2 | | ZIP archive file comment length, which is the size in bytes of the ZIP archive file comment |
| 22 | ... | | ZIP archive file comment |

Because the end of central directory record is at the end of the file, a reader normally locates
it by searching for its signature from the end of the file, and then uses the size and offset
fields to find and read the central directory.

> Note that the ZIP archive file comment is stored unsecured. No encryption or data authentication
> is applied to it, so confidential information should not be stored in the comment.

## General purpose bit flag {#general_purpose_bit_flag}

The general purpose bit flag is a 2 byte bit field that describes how the file is stored. The
defined bits are:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0.0 | 1 bit | 0x0001 | Encryption bit, if set the file is encrypted |
| 0.1 | 1 bit | 0x0002 | Defined only for compression method 6 (Imploding) and 8 (Deflate), see below |
| 0.2 | 1 bit | 0x0004 | Defined only for compression method 6 (Imploding) and 8 (Deflate), see below |
| 0.3 | 1 bit | 0x0008 | If set, the CRC-32, compressed size and uncompressed size are stored in the data descriptor, and set to zero in the local file header |
| 0.4 | 1 bit | 0x0010 | Reserved for use with compression method 8 (Deflate), for enhanced deflating |
| 0.5 | 1 bit | 0x0020 | If set, the file is compressed patched data |
| 0.6 | 1 bit | 0x0040 | Strong encryption, if set the version needed to extract must be at least 50 and bit 0 must also be set, see [strong encryption](#strong_encryption) |
| 0.11 | 1 bit | 0x0800 | Language encoding flag (EFS), if set the file name and comment fields are encoded using UTF-8 |
| 0.13 | 1 bit | 0x2000 | Set when encrypting the central directory to indicate that selected values in the local file header are masked, see [strong encryption](#strong_encryption) |

The remaining bits (0.7, 0.8, 0.9, 0.10, 0.12, 0.14 and 0.15) are unused, or are reserved by
PKWARE for enhanced compression, alternative streams, or other use.

The meaning of bits 1 and 2 depends on the compression method used. For compression method 6
(Imploding), bit 1 indicates whether an 8K (set) or 4K (clear) sliding dictionary was used, and
bit 2 indicates whether 3 (set) or 2 (clear) Shannon-Fano trees were used. For compression method
8 and 9 (Deflate), bits 1 and 2 indicate the compression option used:

| Bit 2 | Bit 1 | Description |
| --- | --- | --- |
| 0 | 0 | Normal compression option |
| 0 | 1 | Maximum compression option |
| 1 | 0 | Fast compression option |
| 1 | 1 | Super fast compression option |

For compression method 14 (LZMA), bit 1 indicates whether an end-of-stream (EOS) marker is used
to mark the end of the compressed data stream. Bits 1 and 2 are undefined for any other
compression method.

## Compression method {#compression_method}

The compression method is a 2 byte value that identifies the compression algorithm used on the
file data. The defined compression methods are:

| Value | Description |
| --- | --- |
| 0 | The file is stored (no compression) |
| 1 | The file is shrunk |
| 2 | The file is reduced with compression factor 1 |
| 3 | The file is reduced with compression factor 2 |
| 4 | The file is reduced with compression factor 3 |
| 5 | The file is reduced with compression factor 4 |
| 6 | The file is imploded |
| 7 | Reserved for the tokenizing compression algorithm |
| 8 | The file is deflated |
| 9 | Enhanced deflating, using Deflate64 |
| 10 | PKWARE data compression library imploding (old IBM TERSE) |
| 11 | Reserved by PKWARE |
| 12 | The file is compressed using the BZIP2 algorithm |
| 13 | Reserved by PKWARE |
| 14 | The file is compressed using LZMA |
| 15 | Reserved by PKWARE |
| 16 | The file is compressed using IBM z/OS CMPSC compression |
| 17 | Reserved by PKWARE |
| 18 | The file is compressed using IBM TERSE |
| 19 | The file is compressed using IBM LZ77 z architecture |
| 20 | Deprecated, use method 93 (Zstandard) |
| 93 | The file is compressed using Zstandard (zstd) |
| 94 | MP3 compression |
| 95 | The file is compressed using XZ |
| 96 | JPEG variant |
| 97 | WavPack compressed data |
| 98 | PPMd version I, revision 1 |
| 99 | AE-x encryption marker, see [strong encryption](#strong_encryption) |

The compression method 8 (Deflate) is the method used by default by most ZIP compatible
application programs. Deflate is a variation of LZ77 combined with Huffman coding, and is the
same algorithm family described in [Zlib compressed data](zlib.md). The stream stored in a ZIP
file is the raw Deflate stream, without the zlib header and check value.

> Note that the compression methods 1 through 6 (shrink, reduce and implode) are legacy
> algorithms and are no longer recommended for use when compressing files.

## Version made by {#version_made_by}

The version made by is a 2 byte value. The upper byte indicates the host operating system
compatibility of the file attribute information, and is used to determine the line record format
for text files and the mapping of the external file attributes. The lower byte indicates the
version of the ZIP specification supported by the software that encoded the file, encoded such
that the value divided by 10 is the major version number and the value modulo 10 is the minor
version number.

The upper byte of the version made by identifies the host system on which the external file
attributes are compatible. The defined values are:

| Value | Host system |
| --- | --- |
| 0 | MS-DOS and OS/2 (FAT / VFAT / FAT32 file systems) |
| 1 | Amiga |
| 2 | OpenVMS |
| 3 | UNIX |
| 4 | VM/CMS |
| 5 | Atari ST |
| 6 | OS/2 High Performance File System (HPFS) |
| 7 | Macintosh |
| 8 | Z-System |
| 9 | CP/M |
| 10 | Windows NTFS |
| 11 | MVS (OS/390 - z/OS) |
| 12 | VSE |
| 13 | Acorn Risc |
| 14 | VFAT |
| 15 | Alternate MVS |
| 16 | BeOS |
| 17 | Tandem |
| 18 | OS/400 |
| 19 | OS X (Darwin) |
| 20 through 255 | Unused |

## Version needed to extract {#version_needed_to_extract}

The version needed to extract is a 2 byte value indicating the minimum supported ZIP
specification version needed to extract the file. The value is encoded such that the value
divided by 10 is the major version number and the value modulo 10 is the minor version number, so
for example a value of 45 encodes version 4.5. When multiple features are applied to a file the
minimum version is set to the highest of the feature versions in use.

The current minimum feature versions are:

| Version | Feature |
| --- | --- |
| 1.0 | Default value |
| 1.1 | File is a volume label |
| 2.0 | File is a folder (directory) |
| 2.0 | File is compressed using Deflate |
| 2.0 | File is encrypted using traditional PKWARE encryption |
| 2.1 | File is compressed using Deflate64 |
| 2.5 | File is compressed using PKWARE DCL imploding |
| 2.7 | File is a patch data set |
| 4.5 | File uses ZIP64 format extensions |
| 4.6 | File is compressed using BZIP2 |
| 5.0 | File is encrypted using DES, 3DES, original RC2, or RC4 |
| 5.1 | File is encrypted using AES |
| 5.1 | File is encrypted using corrected RC2 |
| 5.2 | File is encrypted using corrected RC2-64 |
| 6.1 | File is encrypted using non-OAEP key wrapping |
| 6.2 | Central directory encryption |
| 6.3 | File is compressed using LZMA, PPMd, or encrypted using Blowfish or Twofish |

## CRC-32 {#crc_32}

The CRC-32 is a 4 byte cyclic redundancy check of the uncompressed file data, which provides
data integrity for the file. The CRC algorithm uses the "magic number" 0xdebb20e3, with the CRC
register pre-conditioned with all ones (a starting value of 0xffffffff), and the value
post-conditioned by taking the one's complement of the CRC residual. The value is stored in
little-endian order.

> Note that data integrity for each file must be provided using CRC-32. Where bit 3 of the
> general purpose bit flag is set, the CRC-32 field is set to zero in the local file header and
> the correct value is put in the data descriptor and in the central directory.

## File name {#file_name}

The file name is the name of the file, with an optional relative path. The path stored must not
contain a drive or device letter, or a leading slash, and all slashes used are forward slashes
("/"), as opposed to back slashes, for compatibility with Amiga and UNIX file systems.

> Note that where general purpose bit flag 11 (language encoding flag) is set, the file name and
> file comment are encoded using UTF-8. Where it is clear, the file name and comment should
> conform to the original ZIP character encoding (IBM Code Page 437).

## Extra field {#extra_field}

The extra field is a variable length field used for storage expansion. Where additional
information needs to be stored within a ZIP archive file for particular application or platform
needs, it is stored in the extra field. Programs supporting earlier versions of the specification
can safely skip over an extra field that they do not understand, and continue to the next header.
The extra field is 0 length in version 1.0 of the format.

Extra data is stored as a sequence of blocks, where each block consists of a 2 byte header ID
followed by a 2 byte data size, followed by the data block. The header ID identifies the type of
data in the following block, and the data size identifies the number of bytes in that block.
Header IDs 0 through 31 are reserved for use by PKWARE, and the remaining IDs can be used by
third party vendors.

### ZIP64 extended information extra field (tag 0x0001)

Where one of the size or offset fields in the local or central directory record is too small to
hold the required data (it is set to 0xFFFF or 0xFFFFFFFF), a ZIP64 extended information extra
field is created to hold the full value. The order of the fields is fixed, but a field only
appears if the corresponding record field is set to its maximum value. It consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | 0x0001 | Tag (header ID) for the ZIP64 extended information extra field |
| 2 | 2 | | Size of this extra block |
| 4 | 8 | | Original (uncompressed) file size |
| 12 | 8 | | Compressed file size |
| 20 | 8 | | Offset of the local file header record |
| 28 | 4 | | Number of the disk on which the file starts |

This entry in the local file header must include both the original (uncompressed) and the
compressed file size fields.

## Extra field header IDs

The current header ID mappings defined by PKWARE, and the common third party mappings, are:

| Header ID | Description |
| --- | --- |
| 0x0001 | ZIP64 extended information extra field |
| 0x0007 | AV info |
| 0x0008 | Reserved for extended language encoding data (PFS) |
| 0x0009 | OS/2 |
| 0x000a | NTFS |
| 0x000c | OpenVMS |
| 0x000d | UNIX |
| 0x000f | Patch descriptor |
| 0x0014 | PKCS#7 store for X.509 certificates |
| 0x0015 | X.509 certificate ID and signature for an individual file |
| 0x0016 | X.509 certificate ID for the central directory |
| 0x0017 | Strong encryption header |
| 0x0020 | Reserved for timestamp record |
| 0x7855 | Info-ZIP UNIX (new) |
| 0x6375 | Info-ZIP Unicode comment extra field |
| 0x7075 | Info-ZIP Unicode path extra field |
| 0x5455 | Extended timestamp |
| 0x9901 | AE-x encryption structure, see [strong encryption](#strong_encryption) |

## Encryption {#encryption}

Data encryption may be used to protect files within a ZIP archive file. The keying methods supported
include passwords and public / private keys, either individually or in combination. Encryption
may be applied to individual files. There are three families of encryption in use:

* traditional PKWARE encryption, which is indicated by encryption bit 0 and is considered weak
  by modern standards
* strong encryption (PKWARE's strong encryption specification), indicated by encryption bit 6
* AES encryption (the AE-x scheme), indicated by the compression method 99 marker

### Traditional PKWARE encryption

Traditional PKWARE encryption encrypts the compressed data stream with a stream cipher based on
three 32-bit keys initialized from the password. Each encrypted file has an additional 12 bytes
stored at the start of the data area (the encryption header) defining the encryption header for
that file. The encryption header is originally set to random values and is itself encrypted, and
the last 1 or 2 bytes of the decrypted header hold the high-order byte (or word) of the file
CRC, which can be used to test whether a supplied password is correct.

> Note that traditional PKWARE encryption is considered weak by today's standards. Its use is
> recommended only for situations with low security needs, or for compatibility with older
> applications.

### Strong encryption {#strong_encryption}

Strong encryption (introduced in version 5.x of the specification) supports encryption of each
file using a password or an X.509v3 digital certificate. Strong encryption is indicated by
setting encryption bit 6 in the general purpose bit flag, in which case bit 0 must also be set,
and the version needed to extract must be at least 50. When the central directory structure is
itself encrypted, an archive decryption header and an archive extra data record (signature
0x08064b50, the characters "PK\x06\x08")
precede the central directory, and the version needed to extract must be at least 62. Portions of
the strong encryption technology are covered by PKWARE patents.

### AES encryption (AE-x)

AES encryption (the AE-x scheme) is an alternate password-based encryption method that uses AES
in CTR (counter) mode with an HMAC-SHA1 authentication code, and defines key sizes of 128 or
256 bits (decrypting 192 bit keys is not restricted). It is identified as follows:

* the standard ZIP encryption bit (bit 0 of the general purpose bit flag) is set to indicate the
  file is encrypted
* the compression method field is set to 99 to indicate the file is encrypted with this method
* the actual compression method of the file is stored in an extra field structure identified by
  the header ID 0x9901
* the key size is selected by the version of the AE-x structure: AE-1 uses a 128 bit key and AE-2
  uses a 256 bit key

The AE-x encryption header is stored immediately before the encrypted file data, and after it the
encrypted data is followed by the 10 byte HMAC-SHA1 authentication code. The layout is:

| Offset | Size | Description |
| --- | --- | --- |
| 0 | 2 | Version of the AE-x structure, 1 for AE-1 (128 bit key) and 2 for AE-2 (256 bit key) |
| 2 | 2 | Vendor ID ("AE", 0x4541) |
| 4 | 2 | Vendor version, 0x0001 for AE-1 and 0x0002 for AE-2 |
| 6 | 2 | Actual compression method, the compression method used on the file before encryption |
| 8 | 16 | Key salt, 16 random bytes |
| 24 | 16 or 15 | New password verification value, 16 bytes for a 128 bit key and 15 bytes for a 256 bit key |
| 40 or 39 | 16 or 15 | New salt value, 16 bytes for a 128 bit key and 15 bytes for a 256 bit key |
| ... | ... | Encrypted file data, followed by a 10 byte HMAC-SHA1 authentication code |

For AE-1 the version 1 structure stores the file CRC value in the CRC-32 field. For AE-2 the
version 2 structure stores a value of 0 in the CRC-32 field.

> Note that when using AES encryption the version needed to extract must be at least 51 (5.1).

## History

The ZIP format dates back to the early 1990s, when PKWARE released `PKZIP` and `PKUnZIP` for the
MS-DOS platform. In these earliest versions the format supported compression methods 0 through 6
(stored, shrink, reduce 1 through 4, and implode), and the local file header, central directory
header and end of central directory records already used the "PK\x03\x04", "PK\x01\x02" and
"PK\x05\x06" signatures. Later versions of the specification added the Deflate compression
method (method 8, which became the default), Deflate64 (method 9), BZIP2 (method 12) and LZMA
(method 14), the ZIP64 format extensions for entries larger than 4 GiB, and the strong and AES
encryption schemes described above.

> Note that the modern specification, as published by PKWARE in APPNOTE version 6.3.9, is the
> authoritative definition of the format. The earlier APPNOTE version 4.5 and third party
> (reverse engineering) documents describe the same records, and are useful for understanding the
> historical and forensic structure of the format.

## Splitting and spanning

A ZIP archive file may be split into user defined segments, or spanned across multiple volumes (for
example disk media). Where a file is split, the individual segments use extensions such as ".z01",
".z02" and ".zip" for the final segment, and the disk number fields in the records identify which
segment an entry begins on. Spanning across removable media is analogous, and the disk number
start of an entry indicates the disk on which the entry begins.

## References

<!-- rumdl-disable MD013 -->

* [PKWARE APPNOTE 6.3.9 - .ZIP File Format Specification](https://pkwaredownloads.blob.core.windows.net/pkware-general/Documentation/APPNOTE-6.3.9.TXT)
* [PKWARE APPNOTE 4.5 - .ZIP File Format Specification (historical revision)](https://github.com/hahahahacker2009/9k-geoff/blob/53c7235fde0b36078fc0c60e747a5f8d32806435/sys/src/cmd/gzip/zip45.txt)
* [FileFormat.Info - The ZIP Archive File Format (corion.htm, historical description)](https://www.fileformat.info/format/zip/corion.htm)
* [WinZip - AES (AE-x) encryption for ZIP archive files](http://www.winzip.com/aes_info.htm)

<!-- rumdl-enable MD013 -->
