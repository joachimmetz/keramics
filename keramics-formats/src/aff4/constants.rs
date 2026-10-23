/* Copyright 2024-2026 Joachim Metz <joachim.metz@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License. You may
 * obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations
 * under the License.
 */

/// AFF4 file name of the version container.
pub(super) const AFF4_VERSION_FILE_NAME: &str = "version.txt";

/// AFF4 file name of the container description.
pub(super) const AFF4_CONTAINER_DESCRIPTION_FILE_NAME: &str = "container.description";

/// AFF4 file name of the information container.
pub(super) const AFF4_INFORMATION_FILE_NAME: &str = "information.turtle";

/// Default AFF4 bytes per sector.
pub(super) const AFF4_DEFAULT_BYTES_PER_SECTOR: u32 = 512;

/// AFF4 major version this implementation supports.
pub(super) const AFF4_SUPPORTED_MAJOR_VERSION: u16 = 1;
