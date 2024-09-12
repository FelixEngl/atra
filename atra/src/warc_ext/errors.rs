use crate::io::errors::ErrorWithPath;
use data_encoding::DecodeError;
use thiserror::Error;
use warc::writer::WarcWriterError;

#[derive(Debug, Error)]
pub enum ReaderError {
    #[error(transparent)]
    IO(#[from] ErrorWithPath),
    #[error(transparent)]
    Encoding(#[from] DecodeError),
}

#[derive(Debug, Error)]
pub enum WriterError {
    #[error(transparent)]
    Warc(#[from] WarcWriterError),
    #[error(transparent)]
    IO(#[from] ErrorWithPath),
}
