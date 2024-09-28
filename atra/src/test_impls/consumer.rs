// Copyright 2024. Felix Engl
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

use crate::contexts::local::LinkHandlingError;
use crate::contexts::worker::CrawlWriteError;
use crate::crawl::ErrorConsumer;
use crate::database::DatabaseError;
use crate::link_state::{LinkStateDBError, LinkStateError};
use crate::queue::QueueError;
use crate::test_impls::FakeResponseError;
use thiserror::Error;

pub struct TestErrorConsumer;

impl TestErrorConsumer {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum TestGlobalError {
    #[error(transparent)]
    SlimCrawlError(#[from] DatabaseError),
    #[error(transparent)]
    LinkHandling(#[from] LinkHandlingError),
    #[error(transparent)]
    LinkState(#[from] LinkStateError),
    #[error(transparent)]
    LinkStateDatabase(#[from] LinkStateDBError),
    #[error(transparent)]
    CrawlWriteError(#[from] CrawlWriteError<DatabaseError>),
    #[error(transparent)]
    QueueError(#[from] QueueError),
    #[error(transparent)]
    ClientError(#[from] FakeResponseError),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    RequestError(#[from] reqwest::Error),
}

impl ErrorConsumer<TestGlobalError> for TestErrorConsumer {
    type Error = TestGlobalError;

    fn consume_init_error(&self, e: TestGlobalError) -> Result<(), Self::Error> {
        self.consume_crawl_error(e)
    }

    fn consume_crawl_error(&self, err: TestGlobalError) -> Result<(), Self::Error> {
        /// true = return OK
        fn handle_db_error(e: &DatabaseError) -> bool {
            match e {
                x @ DatabaseError::Damaged { .. } => {
                    log::error!("{x}");
                    false
                }
                x @ DatabaseError::FailureWithoutKey { .. } => {
                    log::error!("{x}");
                    false
                }
                x @ DatabaseError::Failure { .. } => {
                    log::error!("{x}");
                    false
                }
                x @ DatabaseError::RecoverableFailure { .. } => {
                    log::debug!("{x}");
                    true
                }
                x @ DatabaseError::NotFound { .. } => {
                    log::warn!("{x}");
                    true
                }
                x @ DatabaseError::Unknown { .. } => {
                    log::warn!("{x}");
                    true
                }
                x @ DatabaseError::NotSerializable { .. } => {
                    log::warn!("{x}");
                    false
                }
                x @ DatabaseError::NotDeSerializable { .. } => {
                    log::warn!("{x}");
                    true
                }
                DatabaseError::IOErrorWithPath(e) => {
                    log::error!("Got an IO error,try to recover: {e}");
                    true
                }
                DatabaseError::IOError(e) => {
                    log::error!("Got an IO error,try to recover: {e}");
                    true
                }
                DatabaseError::WarcWriterError(e) => {
                    log::error!("Failed to write the warc : {e}");
                    false
                }
                DatabaseError::WarcHeaderValueError(e) => {
                    log::error!("Illegal warc header used: {e}");
                    true
                }
                DatabaseError::Base64DecodeError(e) => {
                    log::warn!("Was not able to decode a base64 value: {e}");
                    true
                }
                DatabaseError::WarcCursorError(e) => {
                    log::warn!("Was not able to read from warc: {e}");
                    true
                }
                DatabaseError::WarcReadError(e) => {
                    log::warn!("Was not able to read from warc: {e}");
                    true
                }
            }
        }

        fn handle_linkstate(e: &LinkStateError) -> bool {
            log::error!("LinkState Error: {e}");
            false
        }

        /// true = return OK
        fn handle_link_state_db_error(e: &LinkStateDBError) -> bool {
            match e {
                LinkStateDBError::Database(db) => handle_db_error(db),
                LinkStateDBError::LinkStateError(state) => handle_linkstate(state),
            }
        }

        fn handle_url_queue_error(e: &QueueError) -> bool {
            match e {
                QueueError::QueueFileError(e) => {
                    log::error!("The queue file has some kind of error: {e}");
                    false
                }
                QueueError::EncodingError(e) => {
                    log::warn!("Had some kind of encoding error: {e}");
                    true
                }
                QueueError::UrlError(e) => {
                    log::warn!("The url was not valid: {e}");
                    true
                }
                QueueError::LockPoisoned => {
                    log::error!("The queue locks are poisoned!");
                    false
                }
            }
        }

        let result = match &err {
            TestGlobalError::SlimCrawlError(e) => handle_db_error(e),
            TestGlobalError::LinkHandling(e) => match e {
                LinkHandlingError::LinkState(e) => handle_link_state_db_error(e),
                LinkHandlingError::UrlQueue(e) => handle_url_queue_error(e),
                LinkHandlingError::LinkNetError(e) => {
                    log::error!("The webgraph had a non recoverable falure: {e}");
                    false
                }
            },
            TestGlobalError::LinkState(e) => handle_linkstate(e),
            TestGlobalError::LinkStateDatabase(e) => handle_link_state_db_error(e),
            TestGlobalError::CrawlWriteError(e) => match e {
                CrawlWriteError::Database(e) => handle_db_error(e),
                CrawlWriteError::WarcReaderError(e) => {
                    log::error!("Failed to read from a warc file: {e}");
                    true
                }
                CrawlWriteError::WarcWriterError(e) => {
                    log::error!("Failed to write to a warc file: {e}");
                    false
                }
                CrawlWriteError::SlimError(e) => handle_db_error(e),
                CrawlWriteError::TempFilesCanNotBeStoredError => {
                    true
                }
            },
            TestGlobalError::QueueError(e) => handle_url_queue_error(e),
            TestGlobalError::ClientError(e) => {
                log::debug!("Client error: {e}");
                true
            }
            TestGlobalError::IOError(e) => {
                log::debug!("Client error: {e}");
                true
            }
            TestGlobalError::RequestError(err) => {
                log::debug!("{err}");
                true
            }
        };

        if result {
            Ok(())
        } else {
            Err(err)
        }
    }

    #[inline]
    fn consume_poll_error(&self, e: TestGlobalError) -> Result<(), Self::Error> {
        self.consume_crawl_error(e)
    }
}
