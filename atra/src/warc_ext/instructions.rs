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

use std::fs::File;

use data_encoding::BASE64;
use itertools::{Itertools, Position};
use serde::{Deserialize, Serialize};

use crate::io::errors::{ErrorWithPath, ToErrorWithPath};
use crate::io::file_owner::FileOwner;
use crate::warc_ext::skip_pointer::WarcSkipPointerWithPath;
use crate::warc_ext::{read_body, ReaderError};

/// An instruction for skipping in a warc file.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum WarcSkipInstruction {
    Single {
        /// The associated skip ponter
        pointer: WarcSkipPointerWithPath,
        /// The number of octets in the body for the header signature
        header_signature_octet_count: u32,
        /// Base64 marker
        is_base64: bool,
    },
    Multiple {
        /// All skip pointers, sorted in continuation order
        pointers: Vec<WarcSkipPointerWithPath>,
        /// The number of octets in the first pointer
        header_signature_octet_count: u32,
        /// Base64 marker
        is_base64: bool,
    },
}

impl WarcSkipInstruction {
    pub fn new_single(
        pointer: WarcSkipPointerWithPath,
        header_signature_octet_count: u32,
        is_base64: bool,
    ) -> Self {
        Self::Single {
            pointer,
            header_signature_octet_count,
            is_base64,
        }
    }

    pub fn new_multi(
        pointers: Vec<WarcSkipPointerWithPath>,
        header_signature_octet_count: u32,
        is_base64: bool,
    ) -> Self {
        Self::Multiple {
            pointers,
            header_signature_octet_count,
            is_base64,
        }
    }

    /// Reads this in the context of [file_owner].
    pub async fn read_in_context(
        &self,
        file_owner: &impl FileOwner,
    ) -> Result<Option<Vec<u8>>, ReaderError> {
        match self {
            value @ WarcSkipInstruction::Single { pointer, .. } => {
                file_owner.wait_until_free_path(pointer.path()).await?;
                value.read()
            }
            value @ WarcSkipInstruction::Multiple { pointers, .. } => {
                for value in pointers {
                    file_owner.wait_until_free_path(value.path()).await?;
                }
                value.read()
            }
        }
    }

    /// Reads this from the pointer.
    pub fn read(&self) -> Result<Option<Vec<u8>>, ReaderError> {
        fn read_impl(
            pointer: &WarcSkipPointerWithPath,
            header_signature_octet_count: u32,
        ) -> Result<Option<Vec<u8>>, ErrorWithPath> {
            let mut file = File::options()
                .read(true)
                .open(pointer.path())
                .to_error_with_path(pointer.path())?;
            return read_body(&mut file, pointer.pointer(), header_signature_octet_count)
                .to_error_with_path(pointer.path());
        }

        match self {
            WarcSkipInstruction::Single {
                pointer,
                header_signature_octet_count,
                is_base64,
            } => {
                let data = read_impl(pointer, *header_signature_octet_count)?;
                Ok(if *is_base64 {
                    if let Some(value) = data {
                        Some(BASE64.decode(&value)?)
                    } else {
                        None
                    }
                } else {
                    data
                })
            }
            WarcSkipInstruction::Multiple {
                pointers,
                header_signature_octet_count,
                is_base64,
            } => {
                let mut collected_data = Vec::new();
                for (pos, value) in pointers.iter().with_position() {
                    match pos {
                        Position::First | Position::Only => {
                            match read_impl(value, *header_signature_octet_count)? {
                                None => {}
                                Some(value) => collected_data.extend(value),
                            }
                        }
                        _ => match read_impl(value, 0)? {
                            None => {}
                            Some(value) => collected_data.extend(value),
                        },
                    }
                }
                if collected_data.is_empty() {
                    Ok(None)
                } else {
                    let collected_data = if *is_base64 {
                        BASE64.decode(&collected_data)?
                    } else {
                        collected_data
                    };
                    Ok(Some(collected_data))
                }
            }
        }
    }
}
