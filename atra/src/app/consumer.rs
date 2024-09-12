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

use thiserror::Error;
use crate::client::ClientError;
use crate::contexts::local::LinkHandlingError;
use crate::contexts::worker::CrawlWriteError;
use crate::crawl::ErrorConsumer;
use crate::database::DatabaseError;
use crate::link_state::{LinkStateDBError, LinkStateError};
use crate::queue::QueueError;

pub struct GlobalErrorConsumer;

impl GlobalErrorConsumer {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum GlobalError {
    #[error(transparent)] SlimCrawlError(#[from] DatabaseError),
    #[error(transparent)] LinkHandling(#[from] LinkHandlingError),
    #[error(transparent)] LinkState(#[from] LinkStateError),
    #[error(transparent)] LinkStateDatabase(#[from] LinkStateDBError),
    #[error(transparent)] CrawlWriteError(#[from] CrawlWriteError<DatabaseError>),
    #[error(transparent)] QueueError(#[from] QueueError),
    #[error(transparent)] ClientError(#[from] ClientError),
    #[error(transparent)] IOError(#[from] std::io::Error),
}

impl ErrorConsumer<GlobalError> for GlobalErrorConsumer {
    type Error = GlobalError;

    fn consume_crawl_error(&self, e: GlobalError) -> Result<(), Self::Error> {
        todo!()
    }

    fn consume_poll_error(&self, e: GlobalError) -> Result<(), Self::Error> {
        todo!()
    }
}