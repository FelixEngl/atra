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

use crate::link_state::LinkStateDBError;
use crate::queue::QueueError;
use crate::web_graph::LinkNetError;
use thiserror::Error;
use svm::error::SvmCreationError;
use text_processing::tf_idf::Idf;
use crate::blacklist::{InMemoryBlacklistManagerInitialisationError, PolyBlackList};
use crate::database::OpenDBError;
use crate::io::errors::ErrorWithPath;

/// Error messages when the context fails somehow.
#[derive(Debug, Error)]
pub enum LinkHandlingError {
    #[error(transparent)]
    LinkState(#[from] LinkStateDBError),
    #[error(transparent)]
    UrlQueue(#[from] QueueError),
    #[error(transparent)]
    LinkNetError(#[from] LinkNetError),
}



#[derive(Debug, Error)]
pub enum LocalContextInitError {
    #[error(transparent)] IoError(#[from] std::io::Error),
    #[error(transparent)] IoWithPathError(#[from] ErrorWithPath),
    #[error(transparent)] SerdeError(#[from] serde_json::Error),
    #[error(transparent)] OpenDBError(#[from] OpenDBError),
    #[error(transparent)] RocksDBError(#[from] rocksdb::Error),
    #[error(transparent)] QueueFileError(#[from] queue_file::Error),
    #[error(transparent)] BlackListError(#[from] InMemoryBlacklistManagerInitialisationError<PolyBlackList>),
    #[error(transparent)] SvmError(#[from] SvmCreationError<Idf>),
    #[error(transparent)] LinkNetError(#[from] LinkNetError),
}
