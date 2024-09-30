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
use camino::Utf8PathBuf;
use data_encoding::BASE64;
use itertools::{Either, Itertools, Position};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIs};
use warc::field::WarcFieldName::ExternalBinFile;
use crate::data::RawVecData;
use crate::io::errors::{ErrorWithPath, ToErrorWithPath};
use crate::io::file_owner::FileOwner;
use crate::warc_ext::skip_pointer::WarcSkipPointerWithPath;
use crate::warc_ext::{read_body, ReaderError};
use crate::warc_ext::read::read_meta;

/// The kind of the single warc instruction.
#[derive(Serialize, Deserialize, Display, Copy, Clone, Debug, Eq, PartialEq, EnumIs, Default)]
#[repr(u8)]
pub enum WarcSkipInstructionKind {
    #[default]
    Normal = 0,
    Base64 = 1,
    ExternalFileHint = 2,
    NoData = 3,
}

/// An instruction for skipping in a warc file.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum WarcSkipInstruction {
    Single {
        /// The associated skip ponter
        pointer: WarcSkipPointerWithPath,
        /// The number of octets in the body for the header signature
        header_signature_octet_count: u32,
        /// The kind of the single.
        kind: WarcSkipInstructionKind,
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
        kind: WarcSkipInstructionKind,
    ) -> Self {
        Self::Single {
            pointer,
            header_signature_octet_count,
            kind,
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

    pub fn is_external_hint(&self) -> bool {
        match self {
            WarcSkipInstruction::Single {
                kind, ..
            } => {
                kind.is_external_file_hint()
            }
            _ => false
        }
    }

    /// Reads this in the context of [file_owner].
    pub async fn read_in_context(
        &self,
        file_owner: Option<&impl FileOwner>,
    ) -> Result<RawVecData, ReaderError> {
        match self {
            value @ WarcSkipInstruction::Single { pointer, .. } => {
                if let Some(file_owner) = file_owner {
                    file_owner.wait_until_free_path(pointer.path()).await?;
                }
                value.read()
            }
            value @ WarcSkipInstruction::Multiple { pointers, .. } => {
                if let Some(file_owner) = file_owner {
                    for value in pointers {
                        file_owner.wait_until_free_path(value.path()).await?;
                    }
                }
                value.read()
            }
        }
    }

    /// Reads this from the pointer.
    pub fn read(&self) -> Result<RawVecData, ReaderError> {
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
                kind,
            } => {
                let result = match kind {
                    WarcSkipInstructionKind::Normal => {
                        read_impl(pointer, *header_signature_octet_count)?.into()
                    }
                    WarcSkipInstructionKind::Base64 => {
                        match read_impl(pointer, *header_signature_octet_count)? {
                            None => {
                                RawVecData::None
                            }
                            Some(value) => {
                                RawVecData::from_vec(BASE64.decode(&value)?)
                            }
                        }
                    }
                    WarcSkipInstructionKind::ExternalFileHint => {
                        let mut file = File::options()
                            .read(true)
                            .open(pointer.path())
                            .to_error_with_path(pointer.path())?;

                        let header = read_meta(&mut file, pointer.pointer())?;

                        match header {
                            None => {
                                RawVecData::None
                            }
                            Some(header) => {
                                match header.get_external_bin_file() {
                                    None => {
                                        RawVecData::None
                                    }
                                    Some(value) => {
                                        match value {
                                            Ok(field_value) => {
                                                match field_value.clone().into_inner() {
                                                    Either::Left(s) => {
                                                        RawVecData::from_external(Utf8PathBuf::from(s))
                                                    }
                                                    Either::Right(v) => {
                                                        RawVecData::from_external(Utf8PathBuf::from(String::from_utf8(v)?))
                                                    }
                                                }
                                            }
                                            Err(anything) => {
                                                return Err(ReaderError::IllegalFieldValue(ExternalBinFile, anything.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    WarcSkipInstructionKind::NoData => {
                        RawVecData::None
                    }
                };

                Ok(result)
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
                    Ok(RawVecData::None)
                } else {
                    let collected_data = if *is_base64 {
                        BASE64.decode(&collected_data)?
                    } else {
                        collected_data
                    };
                    Ok(RawVecData::from_vec(collected_data))
                }
            }
        }
    }
}
