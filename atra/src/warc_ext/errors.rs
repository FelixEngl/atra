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

use crate::io::errors::ErrorWithPath;
use data_encoding::DecodeError;
use thiserror::Error;
use warc::field::{WarcFieldName, WarcFieldValue};
use warc::reader::WarcCursorReadError;
use warc::writer::WarcWriterError;

#[derive(Debug, Error)]
pub enum ReaderError {
    #[error(transparent)]
    IO(#[from] ErrorWithPath),
    #[error(transparent)]
    Encoding(#[from] DecodeError),
    #[error(transparent)]
    Warc(#[from] WarcCursorReadError),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("The field value is {1:?} but this is not a valid value for {0} in the header!!!")]
    IllegalFieldValue(WarcFieldName, WarcFieldValue),
}

#[derive(Debug, Error)]
pub enum WriterError {
    #[error(transparent)]
    Warc(#[from] WarcWriterError),
    #[error(transparent)]
    IO(#[from] ErrorWithPath),
}
