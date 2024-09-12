use thiserror::Error;
use crate::crawl::ErrorConsumer;
use crate::database::DatabaseError;
use crate::warc_ext::{ReaderError, WriterError};

#[derive(Debug, Error)]
pub enum WriteError<E> {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    WarcReaderError(#[from] ReaderError),
    #[error(transparent)]
    WarcWriterError(#[from] WriterError),
    #[error(transparent)]
    SlimError(E)
}
