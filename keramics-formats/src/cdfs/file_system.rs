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

use std::sync::Arc;

use super::volume::CdFsVolume;

/// CD file system (CDFS) file system.
pub struct CdFsFileSystem {
    /// Layers.
    volumes: Vec<Arc<CdFsVolume>>,
}

impl CdFsFileSystem {
    /// Creates a new file system.
    pub(super) fn new(volumes: &[Arc<CdFsVolume>]) -> Self {
        Self {
            volumes: volumes.to_vec(),
        }
    }
}
