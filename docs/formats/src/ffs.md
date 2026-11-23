# Fast File System (FFS) format

The Berkeley Fast File System (FFS) is the on-disk storage layout and allocation format underlying
the [Unix File System (UFS)](ufs.md) on BSD-derived and related Unix-like operating systems,
including FreeBSD, NetBSD, OpenBSD, Solaris, and historically macOS (as Apple UFS).

FFS was introduced in 4.2BSD to replace the original Unix File System by organizing storage into
cylinder groups, larger blocks subdivided into fragments, and redundant superblocks. In BSD kernel
implementations, FFS defines the physical on-disk layout (superblock, cylinder groups, blocks,
fragments, and allocation bitmaps), while the upper [UFS](ufs.md) layer defines the logical file
system structures ([inodes](ufs.md#inode), [directory entries](ufs.md#directory_entries),
[symbolic links](ufs.md#symbolic_links), and [extended attributes](ufs.md#extended_attributes)).

There are multiple versions of FFS:

| Version | Remarks |
| --- | --- |
| FFS1 (UFS1) | 32-bit fragment addresses, 32-bit timestamps, and 128-byte [UFS1 inodes](ufs.md#ufs1_inode), originating from 4.2BSD and 4.4BSD |
| FFS2 (UFS2) | 64-bit fragment addresses, 64-bit timestamps with birth time, 256-byte [UFS2 inodes](ufs.md#ufs2_inode), lazy inode initialization, and native extended attributes, introduced in FreeBSD 5.0 |
| FFS2EA (UFS2EA) | NetBSD variant of FFS2 with extended attributes enabled via a distinct superblock signature |

## Overview

A Fast File System (FFS) volume consists of:

* boot area (reserved space at the start of the volume for boot code and disk labels)
* primary [superblock](#superblock)
* one or more [cylinder groups](#cylinder_group)

### Characteristics

| Characteristics | Description |
| --- | --- |
| Byte order | little-endian or big-endian |
| Date and time values | number of seconds since January 1, 1970 00:00:00 UTC (POSIX epoch); 32-bit seconds in FFS1, 64-bit signed seconds in FFS2 |
| Character strings | NUL-terminated ASCII strings for volume name and last mount point path |

### Terminology

| Term | Description |
| --- | --- |
| Block | Primary unit of allocation, a power of 2 of at least 4096 bytes (commonly 8192, 16384, or 32768 bytes) |
| Fragment | Sub-block unit of allocation used to reduce internal fragmentation for small files and file tails; a block is divided into 1, 2, 4, or 8 fragments (commonly 512, 1024, 2048, or 4096 bytes) |
| Cylinder group | Independent allocation region containing a backup superblock, cylinder group header, allocation bitmaps, an inode table, and data blocks |

### Blocks and fragments {#blocks_and_fragments}

On-disk addresses in FFS (including block pointers stored in [UFS inodes](ufs.md#inode) and indirect
blocks) are expressed in terms of fragments (file system blocks), not full blocks:

```python
byte_offset = fragment_number * fragment_size
fragments_per_block = block_size // fragment_size
```

A full block spans fs_frag consecutive fragments (1, 2, 4, or 8) aligned on a fs_frag boundary. The
superblock stores precomputed shift values to convert between bytes, fragments, blocks, and 512-byte
disk sectors:

```python
block_size = 1 << fs_bshift
fragment_size = 1 << fs_fshift
fragments_per_block = 1 << fs_fragshift
sector_number = fragment_number << fs_fsbtodb
```

## Superblock {#superblock}

The primary superblock is located at a fixed byte offset relative to the start of the volume,
depending on the FFS version and platform:

| Offset | Identifier | Description |
| --- | --- | --- |
| 0 (0x000000) | SBLOCK_FLOPPY | Floppy disk or small media without a boot area |
| 8192 (0x002000) | SBLOCK_UFS1 | Standard FFS1 (UFS1) primary superblock location |
| 65536 (0x010000) | SBLOCK_UFS2 | Standard FFS2 (UFS2) primary superblock location |
| 262144 (0x040000) | SBLOCK_PIGGY | Alternative FFS2 superblock location when a larger boot area is needed |

> Note that readers typically probe the superblock locations in order (65536, 8192, 0, 262144) and
> verify that the superblock signature matches a known magic value and, for FFS2, that fs_sblockloc
> matches the probed offset.

Backup copies of the superblock are stored in each cylinder group at fragment offset fs_sblkno from
the start of the cylinder group.

The superblock (struct fs) is 1376 bytes in size (padded on disk to fs_sbsize bytes, up to 8192
bytes) and consists of:

<!-- rumdl-disable MD033 MD056 -->

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Unknown (historic linked-list field, fs_firstfield) |
| 4 | 4 | | Unknown (unused, fs_unused_1) |
| 8 | 4 | | Superblock fragment offset within a cylinder group (fs_sblkno) |
| 12 | 4 | | Cylinder group header fragment offset within a cylinder group (fs_cblkno) |
| 16 | 4 | | Inode table fragment offset within a cylinder group (fs_iblkno) |
| 20 | 4 | | First data fragment offset within a cylinder group (fs_dblkno) |
| 24 | 4 | | Legacy cylinder group offset stagger (fs_old_cgoffset, FFS1 only) |
| 28 | 4 | | Legacy cylinder group stagger bitmask (fs_old_cgmask, FFS1 only) |
| 32 | 4 | | Legacy last written time (fs_old_time, FFS1), seconds since January 1, 1970 00:00:00 UTC |
| 36 | 4 | | Legacy total number of fragments (fs_old_size, FFS1) |
| 40 | 4 | | Legacy number of data fragments (fs_old_dsize, FFS1) |
| 44 | 4 | | Number of cylinder groups (fs_ncg) |
| 48 | 4 | | Block size in bytes (fs_bsize) |
| 52 | 4 | | Fragment size in bytes (fs_fsize) |
| 56 | 4 | | Number of fragments per block (fs_frag) |
| 60 | 4 | | Minimum percentage of free blocks reserved for the superuser (fs_minfree) |
| 64 | 4 | | Legacy rotational delay (fs_old_rotdelay) |
| 68 | 4 | | Legacy disk revolutions per second (fs_old_rps) |
| 72 | 4 | | Block offset bitmask (fs_bmask, equal to `-(fs_bsize)`) |
| 76 | 4 | | Fragment offset bitmask (fs_fmask, equal to `-(fs_fsize)`) |
| 80 | 4 | | Block size log2 shift (fs_bshift) |
| 84 | 4 | | Fragment size log2 shift (fs_fshift) |
| 88 | 4 | | Maximum number of contiguous blocks (fs_maxcontig) |
| 92 | 4 | | Maximum number of blocks per cylinder group (fs_maxbpg) |
| 96 | 4 | | Fragments per block log2 shift (fs_fragshift) |
| 100 | 4 | | Fragment-to-sector shift (fs_fsbtodb) |
| 104 | 4 | | Actual size of the superblock in bytes (fs_sbsize) |
| 108 | 8 | | Unknown (spare, fs_spare1) |
| 116 | 4 | | Number of block pointers per indirect block (fs_nindir) |
| 120 | 4 | | Number of inodes per block (fs_inopb) |
| 124 | 4 | | Legacy number of sectors per fragment (fs_old_nspf) |
| 128 | 4 | | [Optimization preference](#optimization_preference) (fs_optim) |
| 132 | 4 | | Legacy number of sectors per track including spares (fs_old_npsect) |
| 136 | 4 | | Legacy hardware sector interleave (fs_old_interleave) |
| 140 | 4 | | Legacy sector 0 skew per track (fs_old_trackskew) |
| 144 | 8 | | File system identifier (fs_id, two 32-bit integers) |
| 152 | 4 | | Legacy cylinder group summary area fragment address (fs_old_csaddr, FFS1) |
| 156 | 4 | | Cylinder group summary area size in bytes (fs_cssize) |
| 160 | 4 | | Cylinder group header size in bytes (fs_cgsize) |
| 164 | 4 | | Unknown (spare, fs_spare2) |
| 168 | 4 | | Legacy number of sectors per track (fs_old_nsect) |
| 172 | 4 | | Legacy number of sectors per cylinder (fs_old_spc) |
| 176 | 4 | | Legacy number of cylinders (fs_old_ncyl) |
| 180 | 4 | | Legacy number of cylinders per cylinder group (fs_old_cpg) |
| 184 | 4 | | Number of inodes per cylinder group (fs_ipg) |
| 188 | 4 | | Number of fragments per cylinder group (fs_fpg) |
| 192 | 16 | | Legacy [32-bit cylinder group summary totals](#cylinder_group_summary_32bit) (fs_old_cstotal, FFS1) |
| 208 | 1 | | Superblock modified flag (fs_fmod, runtime only) |
| 209 | 1 | | File system clean flag (fs_clean, 1 if cleanly unmounted) |
| 210 | 1 | | Mounted read-only flag (fs_ronly, runtime only) |
| 211 | 1 | | Legacy 8-bit [file system flags](#file_system_flags) (fs_old_flags) |
| 212 | 468 | | Last mount point path (fs_fsmnt), NUL-terminated ASCII string |
| 680 | 32 | | Volume name (fs_volname), NUL-terminated ASCII string |
| 712 | 8 | | System-wide identifier (fs_swuid) |
| 720 | 4 | | Unknown (padding, fs_pad) |
| 724 | 4 | | Last searched cylinder group rotor (fs_cgrotor) |
| 728 | 128 | | Runtime pointer padding (fs_ocsp, fs_contigdirs, fs_csp, fs_maxcluster, fs_active) |
| 856 | 4 | | Legacy cylinders per cycle in position table (fs_old_cpc) |
| 860 | 4 | | Maximum blocking factor (fs_maxbsize) |
| <td colspan="4">*FFS2 extensions (reserved in legacy FFS1)*</td> |
| 864 | 8 | | Number of unreferenced inodes (fs_unrefs) |
| 872 | 8 | | Size of underlying storage provider in fragments (fs_providersize) |
| 880 | 8 | | Size of area reserved for metadata (fs_metaspace) |
| 888 | 8 | | Saved maximum file size (fs_save_maxfilesize) |
| 896 | 96 | | Unknown (spare, 12 x 8 bytes, fs_sparecon64) |
| 992 | 8 | | Actual byte offset where superblock was read (fs_sblockactualloc, runtime) |
| 1000 | 8 | | Expected byte offset of the primary superblock (fs_sblockloc) |
| 1008 | 64 | | [64-bit cylinder group summary totals](#cylinder_group_summary_64bit) (fs_cstotal) |
| 1072 | 8 | | Last written time (fs_time), 64-bit signed seconds since January 1, 1970 00:00:00 UTC |
| 1080 | 8 | | Total number of fragments in the file system (fs_size) |
| 1088 | 8 | | Number of data fragments in the file system (fs_dsize) |
| 1096 | 8 | | Cylinder group summary area fragment address (fs_csaddr) |
| 1104 | 8 | | Blocks pending release (fs_pendingblocks) |
| 1112 | 4 | | Inodes pending release (fs_pendinginodes) |
| 1116 | 20 x 4 | | Active snapshot inode numbers (fs_snapinum) |
| 1196 | 4 | | Expected average file size in bytes (fs_avgfilesize, typically 16384) |
| 1200 | 4 | | Expected average number of files per directory (fs_avgfpdir, typically 64) |
| 1204 | 4 | | Saved cylinder group size (fs_save_cgsize) |
| 1208 | 8 | | Last mount or consistency check time (fs_mtime), 64-bit signed seconds since POSIX epoch |
| 1216 | 4 | | Soft updates journal free space (fs_sujfree) |
| 1220 | 84 | | Unknown (spare, 21 x 4 bytes, fs_sparecon32) |
| 1304 | 4 | | Superblock CRC-32C check-hash (fs_ckhash) |
| 1308 | 4 | | [Metadata check-hash flags](#metadata_check_hash_flags) (fs_metackhash) |
| <td colspan="4">*Common*</td> |
| 1312 | 4 | | [File system flags](#file_system_flags) (fs_flags) |
| 1316 | 4 | | Size of cluster summary array (fs_contigsumsize) |
| 1320 | 4 | | Maximum length of an inline (fast) [symbolic link](ufs.md#symbolic_links) (fs_maxsymlinklen) |
| 1324 | 4 | | Legacy [inode format](#inode_format) (fs_old_inodefmt, FFS1) |
| 1328 | 8 | | Maximum representable file size in bytes (fs_maxfilesize) |
| 1336 | 8 | | Inverse block bitmask (fs_qbmask, equal to `fs_bsize - 1`) |
| 1344 | 8 | | Inverse fragment bitmask (fs_qfmask, equal to `fs_fsize - 1`) |
| 1352 | 4 | | Validation state (fs_state) |
| 1356 | 4 | | Legacy [rotational position table format](#postbl_format) (fs_old_postblformat) |
| 1360 | 4 | | Legacy number of rotational positions (fs_old_nrpos) |
| 1364 | 8 | | Unknown (spare, fs_spare5) |
| 1372 | 4 | | [Superblock signature](#superblock_signatures) (fs_magic) |

<!-- rumdl-enable MD033 MD056 -->

### Superblock signatures {#superblock_signatures}

The 32-bit signature at offset 1372 (0x55c) identifies both the FFS format version and the byte
order of the file system:

| Value | Little-endian bytes | Big-endian bytes | Identifier | Description |
| --- | --- | --- | --- | --- |
| 0x00011954 | "\x54\x19\x01\x00" | "\x00\x01\x19\x54" | FS_UFS1_MAGIC | FFS1 (UFS1) file system |
| 0x19540119 | "\x19\x01\x54\x19" | "\x19\x54\x01\x19" | FS_UFS2_MAGIC | FFS2 (UFS2) file system |
| 0x19012038 | "\x38\x20\x01\x19" | "\x19\x01\x20\x38" | FS_UFS2EA_MAGIC | NetBSD FFS2 (UFS2) with extended attributes |

### Optimization preference {#optimization_preference}

| Value | Identifier | Description |
| --- | --- | --- |
| 0 | FS_OPTTIME | Minimize allocation time |
| 1 | FS_OPTSPACE | Minimize disk fragmentation |

### Cylinder group summary (32-bit) {#cylinder_group_summary_32bit}

The 32-bit cylinder group summary (struct csum) is 16 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Number of directories (cs_ndir) |
| 4 | 4 | | Number of free blocks (cs_nbfree) |
| 8 | 4 | | Number of free inodes (cs_nifree) |
| 12 | 4 | | Number of free fragments (cs_nffree) |

### Cylinder group summary (64-bit) {#cylinder_group_summary_64bit}

The 64-bit cylinder group summary totals (struct csum_total) is 64 bytes in size and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 8 | | Total number of directories (cs_ndir) |
| 8 | 8 | | Total number of free blocks (cs_nbfree) |
| 16 | 8 | | Total number of free inodes (cs_nifree) |
| 24 | 8 | | Total number of free fragments (cs_nffree) |
| 32 | 4 x 8 | | Unknown (spare, cs_spare) |

### File system flags {#file_system_flags}

In FFS2 (and in FFS1 when FS_FLAGS_UPDATED is set in fs_old_flags), the 32-bit fs_flags field at
offset 1312 stores the active file system flags:

| Value | Identifier | Description |
| --- | --- | --- |
| 0x00000001 | FS_UNCLEAN | File system was not cleanly unmounted |
| 0x00000002 | FS_DOSOFTDEP | Soft Updates enabled |
| 0x00000004 | FS_NEEDSFSCK | Consistency check (fsck) required |
| 0x00000008 | FS_SUJ | Soft Updates Journaling (SU+J) enabled |
| 0x00000010 | FS_ACLS | POSIX.1e Access Control Lists (ACLs) enabled |
| 0x00000020 | FS_MULTILABEL | TrustedBSD Mandatory Access Control (MAC) multi-label enabled |
| 0x00000040 | FS_GJOURNAL | GEOM journaling enabled |
| 0x00000080 | FS_FLAGS_UPDATED | Flags have been migrated from 8-bit fs_old_flags to 32-bit fs_flags |
| 0x00000100 | FS_NFS4ACLS | NFSv4 Access Control Lists (ACLs) enabled |
| 0x00000200 | FS_METACKHASH | Metadata CRC-32C check-hashes enabled |
| 0x00000400 | FS_TRIM | Automatic TRIM / discard requests enabled |

### Metadata check-hash flags {#metadata_check_hash_flags}

When FS_METACKHASH is set in fs_flags, fs_metackhash indicates which metadata structures are
protected by CRC-32C (Castagnoli) check-hashes:

| Value | Identifier | Description |
| --- | --- | --- |
| 0x0001 | CK_CYLGRP | Cylinder group headers have check-hashes (cg_ckhash) |
| 0x0002 | CK_SUPERBLOCK | Superblock has a check-hash (fs_ckhash) |
| 0x0004 | CK_INODE | Inodes have check-hashes (di_ckhash) |
| 0x0008 | CK_INDIR | Indirect blocks have check-hashes |
| 0x0010 | CK_DIR | Directory blocks have check-hashes |

### Inode format {#inode_format}

| Value | Identifier | Description |
| --- | --- | --- |
| -1 (0xffffffff) | FS_42INODEFMT | Legacy 4.2BSD inode and directory format (without d_type in directory entries) |
| 2 | FS_44INODEFMT | 4.4BSD inode and directory format (with d_type in directory entries and inline symlinks) |

### Rotational position table format {#postbl_format}

| Value | Identifier | Description |
| --- | --- | --- |
| -1 (0xffffffff) | FS_42POSTBLFMT | Legacy 4.2BSD static rotational position table |
| 1 | FS_DYNAMICPOSTBLFMT | Dynamic rotational position table |

## Cylinder group {#cylinder_group}

The file system is divided into fs_ncg cylinder groups, each spanning fs_fpg fragments (the final
cylinder group may be smaller). Each cylinder group consists of:

* optional backup [superblock](#superblock) (at `cg_start + fs_sblkno`)
* cylinder group header and allocation bitmaps (at `cg_start + fs_cblkno`)
* inode table containing fs_ipg [UFS inodes](ufs.md#inode) (at `cg_start + fs_iblkno`)
* data blocks and fragments (starting at `cg_start + fs_dblkno`)

For cylinder group index cg_index (starting at 0), the base fragment addresses are calculated as:

```python
cg_base = cg_index * fs_fpg
if is_ffs2:
    cg_start = cg_base
else:
    cg_start = cg_base + fs_old_cgoffset * (cg_index & ~fs_old_cgmask)

backup_superblock_fragment = cg_start + fs_sblkno
cg_header_fragment = cg_start + fs_cblkno
inode_table_fragment = cg_start + fs_iblkno
first_data_fragment = cg_start + fs_dblkno
```

> Note that in cylinder group 0, cg_base is 0; the primary superblock is located at the fixed
> `SBLOCK_*` byte offset rather than necessarily overlapping the boot area.

### Inode table addressing {#inode_table_addressing}

Inodes are numbered starting at 0, with fs_ipg inodes per cylinder group. The byte offset of an
inode on disk is calculated as:

```python
cg_index = inode_number // fs_ipg
inode_index_in_cg = inode_number % fs_ipg
inode_size = 256 if is_ffs2 else 128
inode_offset = (inode_table_fragment(cg_index) * fs_fsize) + (inode_index_in_cg * inode_size)
```

> Note that in FFS2, inodes are initialized lazily as needed. Within a cylinder group, any inode
> whose local index `inode_index_in_cg >= cg_initediblk` has not yet been initialized on disk and
> should be treated as unallocated.

### Cylinder group header

The cylinder group header (struct cg) is 168 bytes in size (followed by variable-length allocation
bitmaps within fs_cgsize bytes) and consists of:

| Offset | Size | Value | Description |
| --- | --- | --- | --- |
| 0 | 4 | | Unknown (historic linked-list field, cg_firstfield) |
| 4 | 4 | 0x00090255 | Signature (cg_magic, CG_MAGIC) |
| 8 | 4 | | Legacy last written time (cg_old_time, FFS1), seconds since POSIX epoch |
| 12 | 4 | | Cylinder group index (cg_cgx) |
| 16 | 2 | | Legacy number of cylinders in this cylinder group (cg_old_ncyl) |
| 18 | 2 | | Legacy number of inode blocks in this cylinder group (cg_old_niblk, FFS1) |
| 20 | 4 | | Number of data fragments in this cylinder group (cg_ndblk) |
| 24 | 16 | | [32-bit cylinder group summary](#cylinder_group_summary_32bit) (cg_cs) |
| 40 | 4 | | Block allocation rotor (cg_rotor) |
| 44 | 4 | | Fragment allocation rotor (cg_frotor) |
| 48 | 4 | | Inode allocation rotor (cg_irotor) |
| 52 | 8 x 4 | | Counts of available fragments of each size 1 to `fs_frag - 1` (cg_frsum) |
| 84 | 4 | | Legacy byte offset to block totals per cylinder (cg_old_btotoff) |
| 88 | 4 | | Legacy byte offset to free block positions per cylinder (cg_old_boff) |
| 92 | 4 | | Byte offset to the used inode bitmap (cg_iusedoff) |
| 96 | 4 | | Byte offset to the free fragment bitmap (cg_freeoff) |
| 100 | 4 | | Byte offset to the next available space in the cylinder group block (cg_nextfreeoff) |
| 104 | 4 | | Byte offset to the free cluster summary array (cg_clustersumoff) |
| 108 | 4 | | Byte offset to the free cluster bitmap (cg_clusteroff) |
| 112 | 4 | | Number of clusters in this cylinder group (cg_nclusterblks) |
| 116 | 4 | | Number of inode blocks in this cylinder group (cg_niblk, FFS2) |
| 120 | 4 | | Number of initialized inodes in this cylinder group (cg_initediblk, FFS2) |
| 124 | 4 | | Number of unreferenced inodes in this cylinder group (cg_unrefs) |
| 128 | 4 | | Unknown (spare, cg_sparecon32) |
| 132 | 4 | | Cylinder group CRC-32C check-hash (cg_ckhash) |
| 136 | 8 | | Last written time (cg_time, FFS2), 64-bit signed seconds since POSIX epoch |
| 144 | 3 x 8 | | Unknown (spare, cg_sparecon64) |

### Allocation bitmaps

The cylinder group header is immediately followed by packed bitmaps whose byte offsets relative to
the start of the cylinder group header are recorded in struct cg:

* **Used inode bitmap** (at cg_iusedoff): 1 bit per inode in the cylinder group (fs_ipg bits), where
  a set bit (1) indicates an allocated inode and a cleared bit (0) indicates a free inode.
* **Free fragment bitmap** (at cg_freeoff): 1 bit per fragment in the cylinder group (cg_ndblk
  bits), where a set bit (1) indicates a free fragment and a cleared bit (0) indicates an allocated
  fragment.
* **Free cluster bitmap** (at cg_clusteroff, when `fs_contigsumsize > 0`): 1 bit per full block,
  indicating whether all fs_frag fragments in the block are free.

## Format edge cases and corruption scenarios

### Dual endianness

FFS file systems can be either little-endian (for example, x86 FreeBSD) or big-endian (for example,
SPARC Solaris or PowerPC Apple UFS). Readers must detect byte order by checking the superblock
signature at offset 1372 in both endiannesses before interpreting any multi-byte integers.

### Uninitialized FFS2 inodes

FFS2 initializes inode table blocks lazily and records the count of initialized inodes per cylinder
group in cg_initediblk. Reading inode slots at or beyond cg_initediblk can return stale or
uninitialized disk data rather than valid inodes.

### Operating system extensions

Although FreeBSD, NetBSD, OpenBSD, Solaris, and Apple UFS share the core FFS superblock and cylinder
group layout, operating systems have added divergent consistency and journaling extensions (such as
FreeBSD Soft Updates, Soft Updates Journaling, and snapshots; NetBSD Write Ahead Physical Block
Logging (WAPBL); and Solaris UFS logging).

## References

* [FreeBSD fs.h](https://github.com/freebsd/freebsd-src/blob/main/sys/ufs/ffs/fs.h), superblock
  and cylinder group definitions
* Marshall Kirk McKusick, William N. Joy, Samuel J. Leffler, and Robert S. Fabry, "A Fast File
  System for UNIX", ACM Transactions on Computer Systems, Vol. 2, No. 3, August 1984
* Marshall Kirk McKusick, "Enhancements to the Fast Filesystem to Support Multi-terabyte Storage
  Systems", Proceedings of BSDCon 2003
