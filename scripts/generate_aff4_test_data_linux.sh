#!/usr/bin/env bash
#
# Script to generate Keramics AFF4 test files on Linux.
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

assert_availability_binary aff4imager

set -e

mkdir -p test_data/aff4

BASE_IMAGE_FILE="test_data/ext/ext2.raw"

if [ -f "${BASE_IMAGE_FILE}" ]
then
    # Create an AFF4 volume with an ext2 file system.
    # Note that the output path is passed as an absolute path, because the AFF4 imager establishes
    # an internal virtual routing map, and therefor not reliably handles relative output paths in
    # sub directories.
    aff4imager -t --relative -i "${BASE_IMAGE_FILE}" -o "${PWD}/test_data/aff4/ext2.aff4"

    # Create an AFF4 volume with two images, an ext2 and an ext3 file system.
    aff4imager -t --relative -i "${BASE_IMAGE_FILE}" -i test_data/ext/ext3.raw -o "${PWD}/test_data/aff4/ext2_multi.aff4"

    # Create a split AFF4 container, with chunked (BEVY) based image storage, from an
    # incompressible 12 MiB source.
    # Note that keramics does not support split AFF4 containers, nor BEVY based image storage.
    SPLIT_SOURCE_IMAGE_FILE="/tmp/keramics_aff4_split.raw"
    dd if=/dev/urandom of="${SPLIT_SOURCE_IMAGE_FILE}" bs=1M count=12 2> /dev/null
    aff4imager -t --relative -i "${SPLIT_SOURCE_IMAGE_FILE}" -o "${PWD}/test_data/aff4/split.aff4" -s 4M
    rm -f "${SPLIT_SOURCE_IMAGE_FILE}"
fi

exit ${EXIT_SUCCESS}
