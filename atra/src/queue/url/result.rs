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

use crate::queue::raw::errors::QueueError;
use crate::queue::url::UrlQueueElement;
use crate::url::guard::GuardianError;
use crate::url::UrlWithDepth;
use std::error::Error;
use std::fmt::Debug;
use thiserror::Error;

/// The result of the GuardedSeedUrlProvider extraction.
/// Helps to interpret what happened
pub enum UrlQueuePollResult<T, E: Error> {
    Ok(T),
    Abort(AbortCause),
    Err(QueueExtractionError<E>),
}

/// The abort cause for something. Can be used as error, but it can also be used for simple fallthrough.
#[derive(Debug, Error)]
pub enum AbortCause {
    #[error("The number of misses was higher than the maximum. Try again later.")]
    TooManyMisses,
    #[error("No valid domain for crawl found.")]
    OutOfPullRetries,
    #[error("The queue is empty.")]
    QueueIsEmpty,
    #[error("The element does not have a host.")]
    NoHost(UrlQueueElement<UrlWithDepth>),
    #[error("Shutdown")]
    Shutdown,
}

/// All possible errors that can happen when retrieving a provider
#[derive(Debug, Error)]
pub enum QueueExtractionError<E: Error> {
    #[error(transparent)]
    HostManager(#[from] GuardianError),
    #[error(transparent)]
    LinkState(E),
    #[error(transparent)]
    QueueError(#[from] QueueError),
}
