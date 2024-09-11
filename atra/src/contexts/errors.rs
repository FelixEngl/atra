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

/// An error thrown when the recovery fails
#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("Failed to recover {0}")]
    LinkStateDB(#[from] LinkStateDBError),
    #[error(transparent)]
    UrlQueue(#[from] QueueError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
}
