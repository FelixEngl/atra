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

use std::sync::{MutexGuard, TryLockError};
use crate::url::ParseError;
use thiserror::Error;

/// Error of an url queue file
#[derive(Debug, Error)]
pub enum QueueError {
    #[error(transparent)]
    QueueFileError(#[from] queue_file::Error),
    #[error(transparent)]
    EncodingError(#[from] bincode::Error),
    #[error(transparent)]
    UrlError(#[from] ParseError),
    #[error("Locks Poisoned")]
    LockPoisoned
}

impl<T> TryFrom<RawQueueError<T>> for QueueError {
    type Error = T;

    fn try_from(value: RawQueueError<T>) -> Result<Self, Self::Error> {
        match value {
            RawQueueError::QueueFileError(err) => {
                Ok(err.into())
            }
            RawQueueError::EncodingError(err) => {
                Ok(err.into())
            }
            RawQueueError::UrlError(err) => {
                Ok(err.into())
            }
            RawQueueError::Blocked(v) => {
                Err(v)
            }
            RawQueueError::LockPoisoned => {
                Ok(Self::LockPoisoned)
            }
        }
    }
}



/// Error of an url queue file
#[derive(Debug, Error)]
pub enum RawQueueError<T> {
    #[error(transparent)]
    QueueFileError(#[from] queue_file::Error),
    #[error(transparent)]
    EncodingError(#[from] bincode::Error),
    #[error(transparent)]
    UrlError(#[from] ParseError),
    #[error("The queue is blocked.")]
    Blocked(T),
    #[error("Poisoned")]
    LockPoisoned
}

impl<T> RawQueueError<T> {
    pub fn retry(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }
}