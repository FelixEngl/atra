//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::io;
use thiserror::Error;
use crate::database::DatabaseError;
use crate::link_state::LinkStateDBError;
use crate::queue::QueueError;
use crate::web_graph::LinkNetError;

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

/// The errors occuring during crawling
#[derive(Debug, Error)]
pub enum WebsiteCrawlerError {
    #[error(transparent)]
    Fetcher(#[from] crate::client::Error),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    LinkState(#[from] LinkStateDBError),
    #[error(transparent)]
    LinkHandling(#[from] LinkHandlingError),
    #[error(transparent)]
    IOError(#[from] io::Error),
}