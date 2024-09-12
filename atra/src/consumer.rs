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