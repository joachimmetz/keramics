# Unix File System (UFS) format

The Unix File System (UFS) is the logical file system layer used by BSD-derived and related
Unix-like operating systems (such as FreeBSD, NetBSD, OpenBSD, Solaris, and historically Apple UFS)
on top of an underlying on-disk storage layout such as the [Fast File System (FFS)](ffs.md).

In BSD implementations, UFS defines the on-disk file system metadata structures (inodes, block
addressing, directory entries, symbolic links, and extended attributes), while [FFS](ffs.md)
defines the volume superblock, cylinder groups, blocks, fragments, and allocation bitmaps.

There are two primary UFS on-disk metadata formats:

| Version | Remarks |
| --- | --- |
| UFS1 | 32-bit block addresses, 32-bit timestamps, and 128-byte inodes, used with FFS1 |
| UFS2 | 64-bit block addresses, 64-bit timestamps with birth time, 256-byte inodes, and native extended attributes, used with FFS2 |

## Overview

A Unix File System (UFS) builds its directory hierarchy and file storage using:

* an underlying volume layout (typically [FFS](ffs.md), which provides the
  [superblock](ffs.md#superblock), [blocks and fragments](ffs.md#blocks_and_fragments), and
  [cylinder groups](ffs.md#cylinder_group))
* fixed-size [inodes](#inode) storing file metadata and block pointers
* [directory entries](#directory_entries) mapping file names to inode numbers
* [symbolic links](#symbolic_links) stored either inline in the inode or in data blocks
* optional [extended attributes](#extended_attributes)

### Characteristics

| Characteristics | Description |
| --- | --- |
| Byte order | little-endian or big-endian, inherited from the underlying [FFS](ffs.md) volume |
| Date and time values | number of seconds since January 1, 1970 00:00:00 UTC (POSIX epoch) with a 32-bit nanosecond fraction (32-bit seconds in UFS1, 64-bit signed seconds in UFS2) |
| Character strings | UTF-8 or narrow character strings (ASCII or system-defined codepage) |

### Terminology

| Term | Description |
| --- | --- |
| Inode | On-disk metadata structure describing a file system entry such as a regular file, directory, symbolic link, or device node |
| Whiteout | Special directory entry used by BSD union mounts to mask an entry from a lower file system layer |

## Inode {#inode}

Inodes are located within the cylinder group inode tables using the
[FFS inode table addressing](ffs.md#inode_table_addressing) calculation.

### UFS1 inode {#ufs1_inode}

The UFS1 on-disk inode (struct ufs1_dinode) is 128 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | | [File mode](#file_mode) (di_mode) |
| 2 | 2 | | Number of hard links (di_nlink) |
| 4 | 4 | | Legacy 16-bit owner/group identifiers (oldids) or Soft Updates Journaling next unlinked inode (di_freelink) |
| 8 | 8 | | File data size in bytes (di_size) |
| 16 | 4 | | Last access time (di_atime), seconds since January 1, 1970 00:00:00 UTC |
| 20 | 4 | | Last access time fraction in nanoseconds (di_atimensec) |
| 24 | 4 | | Last content modification time (di_mtime), seconds since January 1, 1970 00:00:00 UTC |
| 28 | 4 | | Last content modification time fraction in nanoseconds (di_mtimensec) |
| 32 | 4 | | Last inode status change time (di_ctime), seconds since January 1, 1970 00:00:00 UTC |
| 36 | 4 | | Last inode status change time fraction in nanoseconds (di_ctimensec) |
| 40 | 12 x 4 | | Direct fragment addresses (di_db) |
| 88 | 3 x 4 | | Single, double, and triple indirect fragment addresses (di_ib) |
| 100 | 4 | | [Inode flags](#inode_flags) (di_flags) |
| 104 | 4 | | Number of 512-byte blocks allocated to the file (di_blocks) |
| 108 | 4 | | Inode generation number (di_gen) |
| 112 | 4 | | Owner (user) identifier (di_uid) |
| 116 | 4 | | Group identifier (di_gid) |
| 120 | 8 | | Modification revision counter for NFSv4 (di_modrev, or spare) |

### UFS2 inode {#ufs2_inode}

The UFS2 on-disk inode (struct ufs2_dinode) is 256 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 2 | | [File mode](#file_mode) (di_mode) |
| 2 | 2 | | Number of hard links (di_nlink) |
| 4 | 4 | | Owner (user) identifier (di_uid) |
| 8 | 4 | | Group identifier (di_gid) |
| 12 | 4 | | Optimal block size hint (di_blksize) |
| 16 | 8 | | File data size in bytes (di_size) |
| 24 | 8 | | Number of 512-byte blocks allocated to the file (di_blocks) |
| 32 | 8 | | Last access time (di_atime), 64-bit signed seconds since January 1, 1970 00:00:00 UTC |
| 40 | 8 | | Last content modification time (di_mtime), 64-bit signed seconds since January 1, 1970 00:00:00 UTC |
| 48 | 8 | | Last inode status change time (di_ctime), 64-bit signed seconds since January 1, 1970 00:00:00 UTC |
| 56 | 8 | | Creation (birth) time (di_birthtime), 64-bit signed seconds since January 1, 1970 00:00:00 UTC |
| 64 | 4 | | Last content modification time fraction in nanoseconds (di_mtimensec) |
| 68 | 4 | | Last access time fraction in nanoseconds (di_atimensec) |
| 72 | 4 | | Last inode status change time fraction in nanoseconds (di_ctimensec) |
| 76 | 4 | | Creation (birth) time fraction in nanoseconds (di_birthnsec) |
| 80 | 4 | | Inode generation number (di_gen) |
| 84 | 4 | | Kernel-internal flags (di_kernflags) |
| 88 | 4 | | [Inode flags](#inode_flags) (di_flags) |
| 92 | 4 | | [Extended attributes](#extended_attributes) size in bytes (di_extsize) |
| 96 | 2 x 8 | | [Extended attributes](#extended_attributes) fragment addresses (di_extb) |
| 112 | 12 x 8 | | Direct fragment addresses (di_db) |
| 208 | 3 x 8 | | Single, double, and triple indirect fragment addresses (di_ib) |
| 232 | 8 | | Modification revision counter for NFSv4 (di_modrev) |
| 240 | 4 | | Soft Updates Journaling next unlinked inode (di_freelink) |
| 244 | 4 | | Inode CRC-32C check-hash (di_ckhash) |
| 248 | 2 x 4 | | Unknown (spare, di_spare) |

> Note that for character and block special device inodes, the device number is stored in the first
> direct block entry (`di_db[0]`).

### File mode {#file_mode}

<!-- rumdl-disable MD033 MD056 -->

| Value | Identifier | Description |
| --- | --- | --- |
| <td colspan="3">*Access other, Bitmask: 0x0007 (S_IRWXO)*</td> |
| 0x0001 | S_IXOTH | Execute/search permission for other |
| 0x0002 | S_IWOTH | Write permission for other |
| 0x0004 | S_IROTH | Read permission for other |
| <td colspan="3">*Access group, Bitmask: 0x0038 (S_IRWXG)*</td> |
| 0x0008 | S_IXGRP | Execute/search permission for group |
| 0x0010 | S_IWGRP | Write permission for group |
| 0x0020 | S_IRGRP | Read permission for group |
| <td colspan="3">*Access owner (or user), Bitmask: 0x01c0 (S_IRWXU)*</td> |
| 0x0040 | S_IXUSR | Execute/search permission for owner |
| 0x0080 | S_IWUSR | Write permission for owner |
| 0x0100 | S_IRUSR | Read permission for owner |
| <td colspan="3">*Special mode bits*</td> |
| 0x0200 | S_ISVTX | Sticky bit |
| 0x0400 | S_ISGID | Set group identifier (GID) on execution |
| 0x0800 | S_ISUID | Set owner (or user) identifier (UID) on execution |
| <td colspan="3">*Type of file, Bitmask: 0xf000 (IFMT)*</td> |
| 0x1000 | IFIFO | Named pipe (FIFO) |
| 0x2000 | IFCHR | Character device |
| 0x4000 | IFDIR | Directory |
| 0x6000 | IFBLK | Block device |
| 0x8000 | IFREG | Regular file |
| 0xa000 | IFLNK | Symbolic link |
| 0xc000 | IFSOCK | Socket |
| 0xe000 | IFWHT | Whiteout |

<!-- rumdl-enable MD033 MD056 -->

### Inode flags {#inode_flags}

<!-- rumdl-disable MD033 MD056 -->

| Value | Identifier | Description |
| --- | --- | --- |
| <td colspan="3">*User-settable flags*</td> |
| 0x00000001 | UF_NODUMP | Do not dump (back up) the file |
| 0x00000002 | UF_IMMUTABLE | File may not be changed |
| 0x00000004 | UF_APPEND | Writes to the file may only append |
| 0x00000008 | UF_OPAQUE | Directory is opaque when viewed through a union stack |
| 0x00000010 | UF_NOUNLINK | File may not be renamed or deleted |
| <td colspan="3">*Superuser-settable flags*</td> |
| 0x00010000 | SF_ARCHIVED | File is archived |
| 0x00020000 | SF_IMMUTABLE | System-wide immutable file |
| 0x00040000 | SF_APPEND | System-wide append-only file |
| 0x00100000 | SF_NOUNLINK | System-wide undeletable file |
| 0x00200000 | SF_SNAPSHOT | Inode is a file system snapshot |

<!-- rumdl-enable MD033 MD056 -->

### Reserved inode numbers

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | | Unused (indicates a deleted or unallocated directory entry) |
| 1 | UFS_WINO | Whiteout inode |
| 2 | UFS_ROOTINO | Root directory inode |

### Block addressing and indirect blocks

File and directory data blocks are addressed through a fixed hierarchy of fragment addresses stored
in the inode (where block size fs_bsize and fragment count fs_frag are defined by the
[FFS superblock](ffs.md#superblock)):

* **Direct pointers (`di_db[0..12]`)**: 12 direct fragment addresses (UFS_NDADDR = 12). Each entry
  addresses one full block of fs_bsize bytes, except that for a file whose total size does not
  exceed `12 * fs_bsize`, the last direct pointer may address a partial block consisting of 1 to
  `fs_frag - 1` fragments (a tail fragment).
* **Single indirect pointer (`di_ib[0]`)**: addresses a full block containing fs_nindir fragment
  addresses (`fs_bsize / 4` 32-bit addresses in UFS1, or `fs_bsize / 8` 64-bit addresses in UFS2).
* **Double indirect pointer (`di_ib[1]`)**: addresses a block of fragment addresses pointing to
  single indirect blocks.
* **Triple indirect pointer (`di_ib[2]`)**: addresses a block of fragment addresses pointing to
  double indirect blocks.

A fragment address of 0 represents an unallocated (sparse) block or indirect subtree and reads as
0-byte values.

## Directory entries {#directory_entries}

Directory contents are stored in the data blocks of a directory inode and are subdivided into fixed
512-byte chunks (DIRBLKSIZ = 512). Individual directory entries never span across a 512-byte chunk
boundary; the final entry in each chunk extends its record length (d_reclen) to reach the end of the
chunk.

A directory contains entries for "." (self), ".." (parent), and its child file system entries. A
directory entry with an inode number (d_ino) of 0 represents a deleted or unused entry whose space
is skipped using d_reclen.

### Directory entry (4.4BSD / UFS1 and UFS2) {#directory_entry}

The modern directory entry (struct direct) is of variable size (padded to a 4-byte boundary) and
consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Inode number (d_ino), or 0 if unused |
| 4 | 2 | | Record length in bytes (d_reclen), a multiple of 4 |
| 6 | 1 | | [Directory entry file type](#directory_entry_file_types) (d_type) |
| 7 | 1 | | Name length in bytes (d_namlen), excluding the NUL terminator (at most 255) |
| 8 | ... | | Name (d_name), NUL-terminated character string padded to a 4-byte boundary |

> Note that even in UFS2, d_ino in directory entries is a 32-bit unsigned integer.

### Directory entry (4.2BSD / legacy UFS1)

On legacy 4.2BSD UFS1 file systems (where `fs_maxsymlinklen <= 0` and the
[FFS inode format](ffs.md#inode_format) is FS_42INODEFMT), directory entries omit the d_type field
and store a 16-bit name length instead:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Inode number (d_ino), or 0 if unused |
| 4 | 2 | | Record length in bytes (d_reclen), a multiple of 4 |
| 6 | 2 | | Name length in bytes (d_namlen), excluding the NUL terminator (at most 255) |
| 8 | ... | | Name (d_name), NUL-terminated character string padded to a 4-byte boundary |

### Directory entry file types {#directory_entry_file_types}

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | DT_UNKNOWN | Unknown file type |
| 1 | DT_FIFO | Named pipe (FIFO) |
| 2 | DT_CHR | Character device |
| 4 | DT_DIR | Directory |
| 6 | DT_BLK | Block device |
| 8 | DT_REG | Regular file |
| 10 | DT_LNK | Symbolic link |
| 12 | DT_SOCK | Socket |
| 14 | DT_WHT | Whiteout |

## Symbolic links {#symbolic_links}

UFS supports fast (inline) symbolic links when `fs_maxsymlinklen > 0` (typically 60 bytes in UFS1
and 120 bytes in UFS2, corresponding to the combined size of di_db and di_ib).

If di_blocks is 0 and di_size is less than fs_maxsymlinklen, the symbolic link target path is stored
directly inside the di_db and di_ib area of the inode (starting at offset 40 in UFS1 or offset 112
in UFS2). Otherwise, the target path is stored in allocated data blocks addressed normally through
di_db and di_ib.

## Extended attributes {#extended_attributes}

UFS2 natively supports extended attributes stored in up to 2 blocks (UFS_NXADDR = 2) referenced by
`di_extb[0]` and `di_extb[1]`, with the total byte length of all attribute records stored in
di_extsize.

> Note that UFS1 does not have native extended attribute pointers in the inode; FreeBSD UFS1
> optionally stores extended attributes in external backing files located under the ".attribute"
> directory at the root of the file system.

### Extended attribute entry

Within the di_extsize bytes addressed by di_extb, extended attributes are stored sequentially as
packed variable-length records aligned to 8-byte boundaries. Each record (struct extattr) consists
of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Total record length in bytes (ea_length), including header, name, content, and alignment padding |
| 4 | 1 | | [Extended attribute namespace](#extended_attribute_namespaces) (ea_namespace) |
| 5 | 1 | | Number of trailing content padding bytes (ea_contentpadlen, 0 to 7) |
| 6 | 1 | | Name length in bytes (ea_namelength), excluding padding |
| 7 | ... | | Name (ea_name), ASCII string (not necessarily NUL-terminated) followed by 1 to 8 zero-padding bytes to align the content to an 8-byte boundary |
| ... | ... | | Attribute value data, followed by ea_contentpadlen trailing zero-padding bytes |

The offsets and sizes of the name and value within an extended attribute record are calculated as:

```python
header_and_name_size = (7 + ea_namelength + 7) & ~7
content_offset = header_and_name_size
content_size = ea_length - header_and_name_size - ea_contentpadlen
```

### Extended attribute namespaces {#extended_attribute_namespaces}

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | EXTATTR_NAMESPACE_EMPTY | Empty / unused entry |
| 1 | EXTATTR_NAMESPACE_USER | User namespace ("user") |
| 2 | EXTATTR_NAMESPACE_SYSTEM | System namespace ("system", used for ACLs and MAC labels) |

## Format edge cases and corruption scenarios

### Whiteout directory entries

BSD union mounts create whiteout directory entries (d_type equal to DT_WHT and d_ino equal to
UFS_WINO = 1) to mask entries from lower file system layers. Inode 1 does not represent an actual
file on disk and should not be dereferenced as a regular inode.

## References

* [FreeBSD dinode.h](https://github.com/freebsd/freebsd-src/blob/main/sys/ufs/ufs/dinode.h),
  on-disk inode definitions
* [FreeBSD dir.h](https://github.com/freebsd/freebsd-src/blob/main/sys/ufs/ufs/dir.h), directory
  entry definitions
* [FreeBSD extattr.h](https://github.com/freebsd/freebsd-src/blob/main/sys/ufs/ufs/extattr.h),
  extended attribute definitions
