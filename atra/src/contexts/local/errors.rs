use std::io;
use thiserror::Error;
use crate::database_error::DatabaseError;
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
    LinkNetError(#[from] LinkNetError)
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