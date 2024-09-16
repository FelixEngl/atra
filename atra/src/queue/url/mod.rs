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

use std::fmt::Debug;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::Deref;
use serde::de::DeserializeOwned;
use serde::Serialize;
use smallvec::SmallVec;
use crate::queue::errors::{QueueError, RawQueueError};
use crate::queue::url::element::UrlQueueElement;
use crate::queue::EnqueueCalled;
use crate::url::UrlWithDepth;
use tokio::sync::watch::Receiver;

pub mod element;
pub mod queue;
pub mod result;
mod refs;

pub use refs::*;

pub trait SupportsForcedQueueElement<T> where T: Serialize + DeserializeOwned + 'static {
    fn force_enqueue(&self, entry: UrlQueueElement<T>) -> Result<(), QueueError>;
}

/// A traif for an url queue
pub trait UrlQueue<T> where T: Serialize + DeserializeOwned + Sized + 'static {
    async fn enqueue(&self, entry: UrlQueueElement<T>) -> Result<(), QueueError>;

    #[cfg(test)]
    async fn enqueue_borrowed<'a>(
        &self,
        entry: UrlQueueElement<&'a T>,
    ) -> Result<(), QueueError>;

    async fn enqueue_all(
        &self,
        entries: impl IntoIterator<Item = UrlQueueElement<T>>,
    ) -> Result<(), QueueError>;

    async fn dequeue<'a>(&'a self) -> Result<Option<UrlQueueElementRef<'a, T>>, QueueError>;

    #[cfg(test)]
    async fn dequeue_n<'a>(&'a self, n: usize) -> Result<Vec<UrlQueueElementRef<'a, T>>, QueueError>;

    /// Number of elements in the queue
    async fn len(&self) -> usize;

    /// Returns true if the queue is empty.
    async fn is_empty(&self) -> bool;

    fn has_floating_urls(&self) -> bool;

    fn floating_url_count(&self) -> usize;

    /// Broadcasts if enqueue is called
    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
}

pub trait SupportsSeeding {

    /// Enqueues an [url] at distance 0
    async fn enqueue_seed(&self, target: &str) -> Result<(), QueueError>;

    /// Enqueues all [urls] at distance 0
    async fn enqueue_seeds(
        &self,
        urls: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<(), QueueError>;
}

impl<T> SupportsSeeding for T where T: UrlQueue<UrlWithDepth> {
    async fn enqueue_seed(&self, target: &str) -> Result<(), QueueError>  {
        self.enqueue(UrlQueueElement::new(
            true,
            0,
            false,
            UrlWithDepth::from_seed(target)?
        )).await
    }

    async fn enqueue_seeds(&self, urls: impl IntoIterator<Item=impl AsRef<str>>) -> Result<(), QueueError>  {
        self.enqueue_all(
            urls.into_iter()
                .map(|s| {
                    UrlWithDepth::from_seed(s.as_ref())
                        .map(|value| UrlQueueElement::new(true, 0, false, value))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ).await
    }
}
