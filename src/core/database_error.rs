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

use std::fmt::{Debug, Display, Formatter};
use data_encoding::{BASE64, DecodeError};
use rocksdb::{Error, ErrorKind};
use thiserror::Error;
use crate::core::io::errors::ErrorWithPath;
use crate::core::warc::WarcReadError;
use crate::warc::header::{WarcHeaderValueError};
use crate::warc::reader::WarcCursorReadError;
use crate::warc::writer::WarcWriterError;


/// Returns the reason why a database is failing
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Failed in {cf} with {source}.")]
    Damaged {
        cf: &'static str,
        #[source]
        source: Error
    },
    #[error("Tried to execute {action} in {cf} but resulted in {source}.")]
    FailureWithoutKey {
        cf: &'static str,
        action: DBActionType,
        #[source]
        source: Error
    },
    #[error("Tried to execute {action} in {cf} with {key} but resulted in {source}.")]
    Failure {
        cf: &'static str,
        action: DBActionType,
        key: String,
        entry: Option<Vec<u8>>,
        #[source]
        source: Error
    },
    #[error("Recoverable: Tried to execute {action} in {cf} with {key} but resulted in {source}.")]
    RecoverableFailure {
        cf: &'static str,
        action: DBActionType,
        key: String,
        entry: Option<Vec<u8>>,
        #[source]
        source: Error
    },
    #[error("NotFound: Tried to execute {action} in {cf} with {key} but resulted in {source}.")]
    NotFound {
        cf: &'static str,
        action: DBActionType,
        key: String,
        entry: Option<Vec<u8>>,
        #[source]
        source: Error
    },
    #[error("Unknown failure while executing {action} in {cf} with {key} but resulted in {source}.")]
    Unknown {
        cf: &'static str,
        action: DBActionType,
        key: String,
        entry: Option<Vec<u8>>,
        #[source]
        source: Error
    },
    #[error("The value for {key} in {cf} is not serializable!\n{value:?}")]
    NotSerializable {
        cf: &'static str,
        key: String,
        value: Box<dyn Debug + Send + Sync>,
        #[source]
        source: bincode::Error
    },
    #[error("The value for {key} in {cf} is not deserializable!\n{entry}")]
    NotDeSerializable {
        cf: &'static str,
        key: String,
        entry: LazyBase64Value<Vec<u8>>,
        #[source]
        source: bincode::Error
    },
    #[error(transparent)]
    WarcError(#[from] WarcCursorReadError),
    #[error(transparent)]
    IOErrorWithPath(#[from] ErrorWithPath),
    #[error(transparent)]
    IOError(#[from]std::io::Error),
    #[error(transparent)]
    WarcWriterError(#[from] WarcWriterError),
    #[error(transparent)]
    WarcHeaderValueError(#[from] WarcHeaderValueError),
    #[error(transparent)]
    Base64DecodeError(#[from] DecodeError),
    #[error(transparent)]
    WarcReadError(#[from] WarcReadError)
}


#[derive(Debug)]
#[repr(transparent)]
pub struct LazyBase64Value<V: AsRef<[u8]>>(pub V);

impl<V: AsRef<[u8]>> Display for LazyBase64Value<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", data_encoding::BASE64.encode(self.0.as_ref()))
    }
}

/// The action executed on the db
#[derive(Debug, strum::Display, Copy, Clone)]
pub enum DBActionType {
    Read,
    Write,
    // BulkWrite,
    Merge,
    Delete,
    // Iterate,
    Flush,
    // Custom(&'static str)
}

impl DatabaseError {
    pub fn from_serialisation<T: Debug + Send + Sync + 'static>(
        cf: &'static str,
        key: String,
        value: T,
        source: bincode::Error
    ) -> Self {
        Self::NotSerializable {
            cf,
            key,
            source,
            value: Box::new(value)
        }
    }

    pub fn from_deserialisation(
        cf: &'static str,
        key: String,
        entry: LazyBase64Value<Vec<u8>>,
        source: bincode::Error
    ) -> Self {
        Self::NotDeSerializable {
            cf,
            key,
            source,
            entry
        }
    }

    pub fn from<K: AsRef<[u8]>>(
        cf: &'static str,
        action: DBActionType,
        key: K,
        entry: Option<&[u8]>,
        source: Error,
    ) -> DatabaseError {
        match source.kind() {
            ErrorKind::Corruption | ErrorKind::IOError | ErrorKind::ColumnFamilyDropped | ErrorKind::ShutdownInProgress | ErrorKind::CompactionTooLarge => {
                DatabaseError::Damaged {
                    cf,
                    source
                }
            }
            ErrorKind::NotSupported | ErrorKind::InvalidArgument | ErrorKind::Incomplete | ErrorKind::Expired => {
                DatabaseError::Failure {
                    cf,
                    action,
                    key: data_encoding::BASE64.encode(key.as_ref()),
                    entry: entry.map(|value| value.to_vec()),
                    source
                }
            }
            ErrorKind::TryAgain | ErrorKind::MergeInProgress | ErrorKind::TimedOut | ErrorKind::Busy | ErrorKind::Aborted => {
                DatabaseError::RecoverableFailure {
                    cf,
                    action,
                    key: data_encoding::BASE64.encode(key.as_ref()),
                    entry: entry.map(|value| value.to_vec()),
                    source
                }
            }
            ErrorKind::NotFound => {
                DatabaseError::NotFound {
                    cf,
                    action,
                    key: data_encoding::BASE64.encode(key.as_ref()),
                    entry: entry.map(|value| value.to_vec()),
                    source
                }
            }
            ErrorKind::Unknown => {
                DatabaseError::Unknown {
                    cf,
                    action,
                    key: data_encoding::BASE64.encode(key.as_ref()),
                    entry: entry.map(|value| value.to_vec()),
                    source
                }
            }
        }
    }
}



/// Indicates a raw database error
pub trait RawDatabaseError {
    type ReturnValue;

    fn enrich_no_key(
        self,
        cf: &'static str,
        action: DBActionType
    ) -> Self::ReturnValue where Self: Sized;

    fn enrich<K: AsRef<[u8]>>(
        self,
        cf: &'static str,
        action: DBActionType,
        key: K,
        entry: Option<&[u8]>,
    ) -> Self::ReturnValue where Self: Sized;

    #[inline]
    fn enrich_with_entry<K: AsRef<[u8]>>(
        self,
        cf: &'static str,
        action: DBActionType,
        key: K,
        entry: &[u8],
    ) -> Self::ReturnValue where Self: Sized {
        self.enrich(
            cf,
            action,
            key,
            Some(entry)
        )
    }

    #[inline]
    fn enrich_without_entry<K: AsRef<[u8]>>(
        self,
        cf: &'static str,
        action: DBActionType,
        key: K,
    ) -> Self::ReturnValue where Self: Sized {
        self.enrich(
            cf,
            action,
            key,
            None
        )
    }
}

impl RawDatabaseError for Error{
    type ReturnValue = DatabaseError;

    fn enrich_no_key(self, cf: &'static str, action: DBActionType) -> Self::ReturnValue where Self: Sized {
        DatabaseError::FailureWithoutKey {
            cf,
            action,
            source: self
        }
    }

    #[inline]
    fn enrich<K: AsRef<[u8]>>(self, cf: &'static str, action: DBActionType, key: K, entry: Option<&[u8]>) -> Self::ReturnValue {
        DatabaseError::from(
            cf,
            action,
            key,
            entry,
            self
        )
    }
}

impl<T> RawDatabaseError for Result<T, Error>{
    type ReturnValue = Result<T, DatabaseError>;

    fn enrich_no_key(self, cf: &'static str, action: DBActionType) -> Self::ReturnValue where Self: Sized {
        match self {
            Ok(value) => {
                Ok(value)
            }
            Err(err) => {Err(err.enrich_no_key(cf, action))}
        }
    }

    fn enrich<K: AsRef<[u8]>>(self, cf: &'static str, action: DBActionType, key: K, entry: Option<&[u8]>) -> Self::ReturnValue {
        match self {
            Ok(value) => {
                Ok(value)
            }
            Err(err) => {Err(err.enrich(cf, action, key, entry))}
        }
    }
}

// todo: lazy
pub trait RawIOError {
    type ReturnValue;
    fn enrich_ser<V: Debug + Send + Sync + 'static, K: AsRef<[u8]>>(
        self,
        cf: &'static str,
        key: K,
        value: V,
    ) -> Self::ReturnValue;


    fn enrich_de<K: AsRef<[u8]>>(
        self,
        cf: &'static str,
        key: K,
        value: Vec<u8>,
    ) -> Self::ReturnValue;
}

impl RawIOError for bincode::Error {
    type ReturnValue = DatabaseError;

    #[inline]
    fn enrich_ser<V: Debug + Send + Sync + 'static, K: AsRef<[u8]>>(self, cf: &'static str, key: K, value: V) -> Self::ReturnValue {
        DatabaseError::from_serialisation(
            cf,
            data_encoding::BASE64.encode(key.as_ref()),
            value,
            self
        )
    }

    #[inline]
    fn enrich_de<K: AsRef<[u8]>>(self, cf: &'static str, key: K, value: Vec<u8>) -> Self::ReturnValue {
        DatabaseError::from_deserialisation(
            cf,
            BASE64.encode(key.as_ref()),
            LazyBase64Value(value),
            self
        )
    }
}

impl<T> RawIOError for Result<T, bincode::Error> {
    type ReturnValue = Result<T, DatabaseError>;

    #[inline]
    fn enrich_ser<V: Debug + Send + Sync + 'static, K: AsRef<[u8]>>(self, cf: &'static str, key: K, value: V) -> Self::ReturnValue {
        match self {
            Ok(value) => {
                Ok(value)
            }
            Err(err) => {Err(err.enrich_ser(cf, key, value))}
        }
    }

    #[inline]
    fn enrich_de<K: AsRef<[u8]>>(self, cf: &'static str, key: K, value: Vec<u8>) -> Self::ReturnValue {
        match self {
            Ok(value) => {
                Ok(value)
            }
            Err(err) => {Err(err.enrich_de(cf, key, value))}
        }
    }
}