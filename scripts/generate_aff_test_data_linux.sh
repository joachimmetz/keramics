#!/usr/bin/env bash
#
# Script to generate Keramics AFF test files on Linux.
#
# Copyright 2024-2026 Joachim Metz <joachim.metz@gmail.com>
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License. You may
# obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
# WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
# License for the specific language governing permissions and limitations
# under the License.

source ./scripts/shared_linux.sh

assert_availability_binary affconvert

set -e

mkdir -p test_data/aff

BASE_IMAGE_FILE="test_data/ext/ext2.raw"

if [ -f "${BASE_IMAGE_FILE}" ]
then
    # Create a compressed AFF image with an ext2 file system.
    affconvert -z -y -o test_data/aff/ext2.AFF "${BASE_IMAGE_FILE}"

    # Create an uncompressed AFF image with an ext2 file system.
    affconvert -z -y -x -o test_data/aff/ext2_uncompressed.AFF "${BASE_IMAGE_FILE}"

    # Create an AFF image, with 1 MiB pages, with an ext2 file system.
    affconvert -z -y -s 1m -o test_data/aff/ext2_pagesize_1m.AFF "${BASE_IMAGE_FILE}"

    # Create an AFF image, with 1 MiB pages, containing a trailing zero page,
    # from a source consisting of the ext2 file system image followed by
    # a 1 MiB zero region.
    ZERO_PAGE_SOURCE_IMAGE_FILE="/tmp/keramics_aff_zero_page.raw"
    cat "${BASE_IMAGE_FILE}" /dev/zero | head -c $(( 5 * 1024 * 1024 )) > "${ZERO_PAGE_SOURCE_IMAGE_FILE}"
    affconvert -z -y -s 1m -o test_data/aff/ext2_zero_page.AFF "${ZERO_PAGE_SOURCE_IMAGE_FILE}"
    rm -f "${ZERO_PAGE_SOURCE_IMAGE_FILE}"

    # Create an AFF image, with LZMA compressed pages, with an ext2 file system.
    # Note that keramics does not support AFF images with LZMA compressed pages.
    affconvert -z -y -L -o test_data/aff/ext2_lzma.AFF "${BASE_IMAGE_FILE}"

    # Create an AFF directory (AFD) split, with 1 MiB pages, of the ext2 file system image.
    # Note that keramics does not support AFF directory (AFD) splits.
    affconvert -z -y -x -s 1m -M1m -o test_data/aff/ext2_split.afd "${BASE_IMAGE_FILE}"
fi

exit ${EXIT_SUCCESS}
