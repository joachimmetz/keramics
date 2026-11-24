# zip

The zip module provides read-only support for the
[ZIP archive format](https://keramics.github.io/zip.html).

Supported features:

| Category | Feature(s) |
| --- | --- |
| Format versions | ZIP, ZIP64 |
| Compression | Stored (uncompressed), DEFLATE |
| | CRC-32 checksum verification |

Unsupported features:

| Category | Feature(s) |
| --- | --- |
| Compression | bzip2, LZMA, others |
| Encryption | Traditional PKWARE, WinZip AES |
| | Multi-disk (spanned) archives |
