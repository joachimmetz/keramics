# aff

The aff module provides read-only support for the
[Advanced Forensic Format (AFF)](https://keramics.github.io/aff.html).

Supported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 1.0 |
| Format variants | AFF (.AFF) |
| Compression | zlib, LZMA, zero page |

Unsupported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 2.0 (encryption), 3.0 (bzip2, AFD, AFM) |
| Format variants | AFD (.AFD), AFM (.AFM) |
| Compression | bzip2 |
| | Encryption, digital signatures |
