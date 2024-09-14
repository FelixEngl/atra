// Copyright 2024 Felix Engl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// A pointer to the start of an entry in a warc [file]
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WarcSkipPointer {
    /// Offset from the start of the file to the start of the WARC-Header
    position: u64,
    /// The number of octets in the whole body
    body_octet_count: u64,
    /// The size of the warc header in bytes
    warc_header_offset: u32,
}

impl WarcSkipPointer {
    pub fn new(position: u64, warc_header_offset: u32, body_octet_count: u64) -> Self {
        Self {
            position,
            body_octet_count,
            warc_header_offset,
        }
    }

    pub fn position(&self) -> u64 {
        self.position
    }

    pub fn body_octet_count(&self) -> u64 {
        self.body_octet_count
    }

    pub fn warc_header_offset(&self) -> u32 {
        self.warc_header_offset
    }
}

/// A skip pointer with additional informations
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WarcSkipPointerWithPath {
    path: Utf8PathBuf,
    skip_pointer: WarcSkipPointer,
}

#[allow(dead_code)]
impl WarcSkipPointerWithPath {
    /// The file with the associated WARC entry
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// The underlying pointer
    pub fn pointer(&self) -> &WarcSkipPointer {
        &self.skip_pointer
    }

    delegate::delegate! {
        to self.skip_pointer {
            pub fn position(&self) -> u64;
            pub fn warc_header_offset(&self) -> u32;
            pub fn body_octet_count(&self) -> u64;
        }
    }

    pub fn new(path: Utf8PathBuf, skip_pointer: WarcSkipPointer) -> Self {
        Self { path, skip_pointer }
    }

    pub fn create(
        path: Utf8PathBuf,
        position: u64,
        warc_header_offset: u32,
        body_octet_count: u64,
    ) -> Self {
        Self::new(
            path,
            WarcSkipPointer::new(position, warc_header_offset, body_octet_count),
        )
    }
}
