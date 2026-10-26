# Advanced Forensic Format (AFF)

The Advanced Forensic Format (AFF) is used to store data and associated metadata mainly for digital
forensic purposes. It supports:

* storage of both data and associated metadata in one or more files
* data storage, compressed or uncompressed, in pages (or blocks)
* optional integrity verification, with hashes, such as MD5, SHA1 and SHA256, for as storage media
  image and individual pages
* optional encryption
* optional digital signatures, for chain-of-custody and long-term file integrity

The format described in this document is the single file ("raw") on-disk format, for storing a
single storage media image in a single file, commonly with the file extension ".aff".

> Note the AFF format is not to be confused with the [AFF4](aff4.md) format.

AFF comes in multiple format versions:

* 1.0, initial version which supports zlib, PKCS #7 cryptographic signatures
* 2.0, encryption support was added
* 3.0, bzip2 and LZMA compression was support added, introduction of different variants, such as
  single-file (.aff), segmented image directory splits (.afd), and independent raw
  bitstream-metadata split pairings (.afm), changes in metadata taxonomy.

> Note that bzip2 compression was never implemented in the AFFLIBv3 reference implementation.

## Overview

An AFF file consists of:

* a file header
* zero or more image data segments, referred to as pages.
* optionally, a hash per page (segment), see [page hashes](#page_hashes)
* optionally, metadata. Free form name-value pairs stored as segments, see [metadata](#metadata)
* optionally, an encryption key segment, see [encryption](#encryption)
* optionally, signature segments, see [signatures](#signatures)

AFF is defined in two layers. The disk-representation layer defines how a disk image and its
associated metadata are represented, as named segments with a flag and a data payload. The
data-storage layer specifies how those segments are stored in an actual file. This document
describes the disk-representation layer and the binary format of the data-storage layer.

> Note the AFF specification also defines an XML format for the data-storage layer, which stores
> the same segments as an XML document. The binary format is smaller and faster than the XML
> format. The XML encoding can be produced automatically from an AFF binary file using AFFLIB's
> "afxml" tool.

### Characteristics

| Characteristics | Description |
| --- | --- |
| Byte order | big-endian |
| Date and time values | TBD |
| Character strings | TBD |

The default page size is 16 MiB (16777216 bytes).

## File header

An AFF file starts with an 8 byte file header, which contains:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 8 | "AFF10\r\n\000" | Signature |

A file that contains this signature at offset 0 is identified as an AFF file. An empty file with
the extension ".aff" is also considered to be an AFF file.

## Segment

The remainder of the file consists of segments. Segments are arranged back-to-back, without any
padding between them. A segment consists of:

* a segment header
* a segment name
* optionally, segment data
* a segment tail

> Note that the order of the segments within an AFF file is not specified. In particular the first
> segment of the file does not need to be page 0. Readers need to use the name of a segment to
> locate a specific value within the file.

### Segment header

The segment header is 16 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "AFF\x00" | Signature, which identifies the start of a segment |
| 4 | 4 | | Name length, which contains the number of bytes of the segment name |
| 8 | 4 | | Data length, which contains the number of bytes of the segment data |
| 12 | 4 | | Flag, which contains an argument associated with the segment name |

The segment name follows the segment header. The size of the segment name in bytes is contained
in the name length. The segment data, if present, follows the segment name. The size of the
segment data in bytes is contained in the data length.

All multi-byte numeric values are stored in network byte order (big-endian).

### Segment name

The size of a segment name is limited to 64 bytes. The segment names used by the AFF format are:

* a page number, such as "page123", where 123 is the zero-based page number in decimal notation,
  see [pages](#pages)
* a page hash name, such as "page123_md5", see [page hashes](#page_hashes)
* a metadata name, such as "imagesize" or "badflag", see [metadata](#metadata)
* an encryption key name, such as "affkey_aes256", see [encryption](#encryption)
* a signature name, such as "cert-sha256", see [signatures](#signatures)
* a bill of materials name, such as "affbom1", see [bill of materials](#bill_of_materials)
* "dir", which identifies an AFF directory (AFD) file, see [related formats](#related-formats)
* an empty name (string), which identifies an [ignored segment](#ignored_segment)

> Note a segment name may refer to a segment that is stored encrypted (with the suffix
> "/aes256"), see [encryption](#encryption), or signed, with the suffix "/sha256", see
> [signatures](#signatures).

### Segment data

The size of the segment data in bytes is contained in the data length of the segment header. The
segment data is one of:

* a page of image data, at most the page size in bytes, see [pages](#pages)
* a hash value of a page or of the whole image, see [page hashes](#page_hashes) and
  [image hashes](#image_hashes)
* an encoded 64-bit value, see [64-bit values](#64_bit_values)
* a metadata value, which is a free form string, see [metadata](#metadata)
* an encryption key value, see [encryption](#encryption)
* a digital signature, see [signatures](#signatures)

### Segment tail

The segment tail is 8 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | "ATT\x00" | Signature, which identifies the end of a segment |
| 4 | 4 | | Segment size, which contains the total size of the segment |

The segment size contains the total size of the segment, being the segment header (16 bytes)
plus the segment name, the segment data and the segment tail (8 bytes).

> Note the segment size is used to verify the integrity of a segment: if the segment size does
> not equal the computed total size of the segment the file is considered corrupt. The segment
> size in the tail, together with the name and data lengths in the header, allows a reader to
> skip the data of a segment without reading it, enabling rapid seeking through the file in both
> directions.

### Ignored segment {#ignored_segment}

A segment with a empty segment name is called an ignored segment, which are used to:

* invalidate (or delete) the data of a previously written segment. The data of the previously
  written segment is overwritten with zeros, and a new value (if any) is then written elsewhere in
  the file
* fill space in the file. In particular, where a new segment is written into a previously
  invalidated, and larger, region the remaining space is filled with an ignored segment

## 64-bit values {#64_bit_values}

A 64-bit value is stored as two 32-bit values, as the lower 32 bits first followed by the upper
32 bits. Each of the two 32-bit values is stored in big-endian order. The flag of a segment
containing a 64-bit value is set to 0x0002.

| Offset | Size | Description |
| --- | --- | --- |
| 0 | 4 | Lower 32 bits, stored in big-endian order |
| 4 | 4 | Higher 32 bits, stored in big-endian order |

E.g. the 64-bit value 0x0000000100000004 is stored as the byte sequence 04 00 00 00 01 00 00 00.

The known segment names that contain a 64-bit value are:

* "imagesize", the size of the image, see [metadata](#metadata)
* "badsectors", the number of bad sectors, see [metadata](#metadata)
* "devicesectors", the number of sectors of the device, see [metadata](#metadata)
* "blanksectors", the number of blank (all zero 0x00) sectors, see [metadata](#metadata)
* "cylinders", the number of cylinders of the C:H:S geometry of the device
* "heads", the number of heads of the C:H:S geometry of the device
* "sectors_per_track", the number of sectors per track of the C:H:S geometry of the device

## Pages {#pages}

The image data is stored in pages. A page is identified by a segment name of the form "pageN",
where N is the zero-based page number in decimal notation, e.g. "page0" or "page123".

* The page number of a page is the image offset divided by the page size (zero-based). The
  offset of a sector within its page is the image offset minus the page number multiplied by
  the page size.
* The data of a page is at most the page size in number of bytes. A page that is smaller than the
  page size in number of bytes is padded, see [bad flag](#bad_flag).
* Pages do not need to be stored sequentially.

The flag in the segment header of a page indicates how the page data has been stored, see
[page compression](#page_compression).

### Page compression {#page_compression}

The flag in the segment header of a page consists of the following bits:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0.0 | 1 bit | 0x0001 | Page compressed flag, which indicates that the page data is stored compressed |
| 0.1 | 1 bit | 0x0002 | Page compressed at maximum level flag, which indicates that the page data was compressed at the maximum compression level |
| 0.4 | 4 bits | 0x00f0 | The compression algorithm used on the page data |

The compression algorithm is one of:

| Value | Identifier | Description |
| --- | --- | --- |
| 0x0000 | AF_PAGE_COMP_ALG_ZLIB | [zlib compressed data](zlib.md) |
| 0x0010 | AF_PAGE_COMP_ALG_BZIP | Reserved for bzip2 compressed data, but never implemented |
| 0x0020 | AF_PAGE_COMP_ALG_LZMA | [LZMA compressed data](#lzma) |
| 0x0030 | AF_PAGE_COMP_ALG_ZERO | [Zero page](#zero_page) |

Where a page cannot be compressed, or the compressed data is not smaller than the page data, the
page is stored uncompressed, and the flag is set to the value 0x0000.

### Zero page {#zero_page}

A zero page is a page that consists entirely of 0 byte values.

A zero page is stored as a single 32-bit value, in big-endian order, which contains the number of
consecutive zero bytes that the page represents. The flag of a zero page has the page compressed
flag set, the zero compression algorithm set, and the page compressed at maximum level flag set,
i.e. the flag is 0x0033.

### LZMA {#lzma}

An LZMA compressed page is stored as an LZMA compressed stream, without an end marker. The
stream is preceded by a five byte properties block and an eight byte uncompressed size,
consisting of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 1 | 0x5d | Properties, calculated as (pb*5 + lp)*9 + lc, where the position bits (pb) is 2, the literal position bits (lp) is 0, and the literal context bits (lc) is 3 |
| 1 | 4 | 0x01000000 | The dictionary size, which is 16777216 bytes (2 to the power of 24), stored in little-endian order |
| 5 | 8 | | Uncompressed size, the size in bytes of the uncompressed page, stored in little-endian order |
| 13 | ... | | LZMA compressed data |

## Page hashes {#page_hashes}

For each page of the image, a hash of the (uncompressed) page data can be stored in an optional,
dedicated segment:

| Segment name | Size in bytes | Description |
| --- | --- | --- |
| "pageN_md5" | 16 | MD5 hash of the page data |
| "pageN_sha1" | 20 | SHA1 hash of the page data |
| "pageN_sha256" | 32 | SHA256 hash of the page data |

> Note the hash is over the uncompressed page data, including any padding (zero fill and bad flag
> pattern) applied to the page.

TODO: determine the behavior of encrypted pages.

## Image hashes {#image_hashes}

A hash of the entire image (the linear bitstream of all the pages) can be stored in the following
metadata segments:

| Segment name | Size in bytes | Description |
| --- | --- | --- |
| "md5" | 16 | MD5 hash of the image |
| "sha1" | 20 | SHA1 hash of the image |
| "sha256" | 32 | SHA256 hash of the image |

## Bad flag {#bad_flag}

A bad flag is a pattern that is used to mark bad sectors, and to fill the remainder of partially
written pages.

* The bad flag is stored in the "badflag" metadata segment, and is as long as one sector.
* The bad flag used by AFFLIBv3 is a sector sized region of random bytes, where the first 10 bytes
  are overwritten with the ASCII string "BAD SECTOR".

Where a page contains fewer than the page size in number of bytes, the remainder of the sector
in which the last data byte is located is zero filled, and the remainder of the page (in whole
sectors) is filled with the bad flag.

> Note that a page that contains the bad flag pattern in a sector can therefore be used to detect
> bad sectors, and to distinguish them from real data sectors.

## Metadata {#metadata}

Metadata is stored as name-value pairs. Each name-value pair is stored as a segment, where the
segment name is the metadata name and the segment data is the metadata value, unless the value is
a 64-bit value, see [64-bit values](#64_bit_values).

The metadata value is a free form string unless the segment is known to contain another encoding.
The value is stored either in the flag of the segment header, or in the segment data of the
segment.

The known metadata segments are:

| Segment name | Value | Description |
| --- | --- | --- |
| "acquisition_commandline" | string | The command line used to create the image |
| "acquisition_date" | string | Acquisition date and time in the format YYYY-MM-DD HH:MM:SS TZT |
| "acquisition_device" | string | Device used to do the imaging |
| "acquisition_dmesg" | string | The output of the "dmesg" command at the time of acquisition |
| "acquisition_iso_country" | string | ISO country code of the acquisition |
| "acquisition_macaddr" | string | MAC addresses of the acquisition system. A list of lines, each line is a MAC address, e.g. "00:03:93:14:c5:04" |
| "acquisition_notes" | string | Notes made while imaging |
| "acquisition_seconds" | flag | Acquisition time in seconds |
| "acquisition_tecnician" | string | Name of the technician that did the imaging |
| "aff_file_type" | string | The type of the file. One of "AFF", "AFM" or "AFD" |
| "afflib_version" | string | Version of the library that created the file |
| "badflag" | data | The pattern used to mark a bad sector, see [bad flag](#bad_flag) |
| "badsectors" | 64-bit value | The number of bad sectors |
| "batch_item_name" | string | The name of the item the image belongs to in the batch |
| "batch_name" | string | The name of a batch the image belongs to |
| "blanksectors" | 64-bit value | The number of blank (all zero 0x00) sectors |
| "case_num" | string | Case number |
| "creator" | string | Name of the program that created the AFF file |
| "cylinders" | 64-bit value | Cylinders of the C:H:S geometry |
| "dco_present" | flag | Whether a device capacity offset (DCO) is present. 0 or 1 |
| "device_capabilities" | string | Human readable string of the capabilities of the source device |
| "device_firmware" | string | The firmware version of the source device |
| "device_manufacturer" | string | The manufacturer of the source device |
| "device_model" | string | The model of the source device |
| "devicesectors" | 64-bit value | The number of sectors of the device |
| "device_sn" | string | The serial number of the source device |
| "device_source" | string | The source of the device |
| "heads" | 64-bit value | Heads of the C:H:S geometry |
| "hpa_present" | flag | Whether a host protected area (HPA) is present. 0 or 1 |
| "image_gid" | data | A 128-bit unique identifier of the image |
| "imagesize" | 64-bit value | The size of the image in bytes, which is the number of the last mapped byte in the image plus 1 |
| "lbasize" | 64-bit value | The LBA size of the device, in bytes |
| "location_in_computer" | string | Where the source device was located in the computer |
| "md5" | data | MD5 hash of the whole image, see [image hashes](#image_hashes) |
| "pagesize" | flag | The page size in bytes. When not present 16 MiB (16777216 bytes) is the default |
| "sectorsize" | flag | The sector size in bytes. When not present 512 bytes is the default |
| "sectors_per_track" | 64-bit value | Sectors per track of the C:H:S geometry |
| "sha1" | data | SHA1 hash of the whole image, see [image hashes](#image_hashes) |
| "sha256" | data | SHA256 hash of the whole image, see [image hashes](#image_hashes) |

> Note the metadata names "segN" and "segsize" are deprecated names for the page number "pageN" and
> the pagesize, respectively. The metadata names "imaging_commandline", "imaging_date",
> "imaging_device" and "imaging_notes", used by the AFF 1.0 specification, are deprecated names
> for "acquisition_commandline", "acquisition_date", "acquisition_device" and "acquisition_notes",
> respectively.

## Encryption {#encryption}

The AFF format allows segments to be stored encrypted. The data of an encrypted segment is stored
in a segment whose name is the segment name with the suffix "/aes256" appended, e.g. the encrypted
form of the segment "page1" is stored in the segment "page1/aes256".

Encryption is applied to the segment data only. The segment header, name and tail are not encrypted.

To read the clear text data of an encrypted segment, where D is the size of the segment data in
bytes:

* read the D bytes of the segment data
* decrypt the first D - (D modulo 16) bytes with AES-256 in CBC mode
* remove the padding, where the padding is the last 16 - (D modulo 16) bytes of the decrypted
  data, if D modulo 16 is not 0

The size of the clear text data is D - 16, if D modulo 16 is not 0, otherwise the size of the
clear text data is D.

To write an encrypted segment, where C is the size of the clear text data in bytes:

* the clear text is padded with `pad` bytes, where `extra` is C modulo 16 and `pad` is
  (16 - extra) modulo 16, and each of the `pad` bytes has the value `pad + extra`, which is 16
  where padding is present (there is no padding if C is a multiple of 16)
* the C + pad bytes of clear text are encrypted with AES-256 in CBC mode
* the segment data of the encrypted segment is the ciphertext, whose size in bytes is
  C + pad + extra
* the initialization vector (IV) of the AES-256 cipher is the 16 bytes of the segment name (the
  name without the "/aes256" suffix), zero extended or truncated to 16 bytes

The encryption key is a 256-bit AES key, that is stored in the file as follows:

### affkey_aes256

The "affkey_aes256" segment contains an encryption key that is bound to a passphrase. The segment
data is 52 bytes and consists of:

| Offset | Size | Description |
| --- | --- | --- |
| 0 | 4 | Version, which is 1, stored in big-endian order |
| 4 | 32 | The 256-bit AES encryption key, encrypted with the SHA-256 digest of the passphrase, as two independent 16 byte blocks (ECB mode) |
| 36 | 16 | 16 zero bytes, encrypted with the SHA-256 digest of the passphrase, as an independent 16 byte block (ECB mode). Used to verify that the passphrase is correct |

The SHA-256 digest of the passphrase is used as the AES-256 key for the above encryptions. A
reader decrypts the two key blocks and the zero block with the SHA-256 digest of the passphrase,
and the passphrase is correct only if the zero block decrypts to all zero (0x00) bytes.

> Note versions of AFFLIB before 3.1.6 may have written the key segment with a larger size, due
> to a structure packing bug. Readers should be prepared to accept the key and zero blocks at
> offsets 4 and 36 of the segment data, regardless of the total segment size.

### affkey_evpN

The "affkey_evpN" segments contain an encryption key that is sealed using one or more
certificates (public keys). The value N is the number of the certificate, starting at 0, with
one "affkey_evpN" segment per certificate. The segment data consists of:

| Offset | Size | Description |
| --- | --- | --- |
| 0 | 4 | Version, which is 1 for the current version, stored in big-endian order |
| 4 | 4 | The size in bytes of the encrypted encryption key, stored in big-endian order |
| 8 | 4 | The size in bytes of the encrypted affkey, stored in big-endian order |
| 12 | 16 | Initialization vector, which is 16 random bytes |
| 28 | ... | The encryption key, encrypted with the public key of the certificate N |
| ... | ... | The 256-bit AES encryption key (affkey), encrypted with AES-256 in CBC mode using the encryption key and the initialization vector |

To recover the 256-bit AES encryption key, a reader decrypts the encrypted key with the private
key matching the certificate N, and then decrypts the encrypted affkey with AES-256 in CBC mode,
using the recovered encryption key and the initialization vector.

## Signatures {#signatures}

The AFF format allows segments to be digitally signed. A segment that is signed is referred to as
the signed segment. The signature of the segment is stored in a separate segment, where the name
of the signature segment is the name of the signed segment with the suffix "/sha256" appended.

* The flag of the signature segment is set to the signature mode, see below
* The data of the signature segment is a digital signature, produced with the private key of the
  signer
* The signature is calculated over the SHA-256 digest of the concatenation of: the name of the
  signed segment (including a zero byte at the end), the flag of the signed segment (as a 32-bit
  value in big-endian order), and the data of the signed segment
* The certificate (public key) of the signer is stored in the "cert-sha256" segment, as a PEM
  encoded X.509 certificate

> Note the AFF 1.0 specification used PKCS #7 signatures. In particular, the signature of a page
> hash segment was stored in a dedicated segment, such as "pageN_md5_sig" or "pageN_sha1_sig",
> where N is the page number.

### Signature modes

The flag of the signature segment is set to the following signature modes:

| Value | Description |
| --- | --- |
| 0x0000 | The signature is for the name, flag and data of the signed segment as they are stored in the file |
| 0x0001 | The signature is for the name, a value of the flag of 0, and the (uncompressed) data of the signed segment |
| 0xffff | The signature is invalid; the signed segment should be deleted or re-signed |

> Note that signature mode 0x0001 is used where the data of the signed segment is compressed in
> the file, and the signature is for the (uncompressed) data. Reading the signed segment returns
> the (uncompressed) data.

### Bill of materials {#bill_of_materials}

The "affbomN" segments contain a bill of materials (BOM) of the AFF file. N is a sequence
number, starting at 1, which identifies a link in the chain of custody of the file. Each time
the file is copied or its custody is transferred, a new BOM segment is appended. A BOM segment
contains an XML document:

```xml
<affbom version="1">
  <date type='ISO 8601'>YYYY-MM-DDTHH:MM:SS</date>
  <program>PROGRAM</program>
  <notes>TEXT</notes>
  <signingcertificate>
    PEM ENCODED X.509 CERTIFICATE
  </signingcertificate>
  <affsegments>
    <segmenthash segname="NAME" sigmode="0" alg='sha256'>
      BASE 64 ENCODED SHA-256 DIGEST
    </segmenthash>
  </affsegments>
</affbom>
BASE 64 ENCODED SIGNATURE
```

The elements of the document are:

* `<date>`: the time and date the BOM was created, in ISO 8601 format
* `<program>`: the name of the program that created the BOM
* `<notes>`: optional free form text describing the transfer of custody
* `<signingcertificate>`: the PEM encoded X.509 certificate used to sign the BOM
* `<affsegments>`: one `<segmenthash>` element per segment of the file, where the "segname"
  attribute is the name of the segment, the "sigmode" attribute is the value 0, and the data of
  the element is the base 64 encoded SHA-256 digest of the concatenation of: the name of the
  segment (including a zero byte at the end), the flag of the segment (as a 32-bit value in
  big-endian order), and the data of the segment
* the base 64 encoded signature over the XML document (up to, but not including, the
  signature), produced with the private key matching the signing certificate

> Note the BOM format is maintained by the AFFLIB tools. The AFF core library only defines the
> "affbomN" segment name.

## Related formats

The AFFLIB library also implements two other file formats, that are built on the same segment
concept used by the AFF format:

* **AFD** (AFF Directory), in which a disk image is stored in multiple AFF files, stored in a
  single directory. Each AFF file contains a set of pages of the image, and contains the "dir"
  segment, which should be the last segment of the file.
* **AFM**, in which a raw (dd) file is used to store the image data, and an AFF file (with the
  extension ".afm") is used to store the metadata of the image. The raw file can also be split
  into multiple raw files (split-raw), in which case each raw file contains a set of pages of
  the image.

## References

* [Advanced Forensic Format: an open, extensible format for disk imaging](https://cs.harvard.edu/malan/publications/aff.pdf),
  by S. Garfinkel, D. Malan, K. Dubec, C. Stevens and C. Pham
