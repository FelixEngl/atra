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

use crate::database::DatabaseError;
use crate::warc_ext::{ReaderError, WriterError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CrawlWriteError<E> {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    WarcReaderError(#[from] ReaderError),
    #[error(transparent)]
    WarcWriterError(#[from] WriterError),
    #[error(transparent)]
    SlimError(E),
    #[error("Tried to store a tempfile. this is not possible!")]
    TempFilesCanNotBeStoredError
}
