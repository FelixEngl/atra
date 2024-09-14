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
use crate::queue::url::element::UrlQueueElement;
use crate::queue::EnqueueCalled;
use crate::url::UrlWithDepth;
use tokio::sync::broadcast::Receiver;

pub mod element;
pub mod queue;
pub mod result;

/// A traif for an url queue
pub trait UrlQueue {
    /// Enqueues an [url] at distance 0
    async fn enqueue_seed(&self, url: &str) -> Result<(), QueueError> {
        self.enqueue(UrlQueueElement::new(
            true,
            0,
            false,
            UrlWithDepth::from_seed(url)?,
        ))
        .await
    }

    /// Enqueues all [urls] at distance 0
    async fn enqueue_seeds(
        &self,
        urls: impl IntoIterator<Item = impl AsRef<str>> + Clone,
    ) -> Result<(), QueueError> {
        self.enqueue_all(
            urls.into_iter()
                .map(|s| {
                    UrlWithDepth::from_seed(s.as_ref())
                        .map(|value| UrlQueueElement::new(true, 0, false, value))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .await
    }

    async fn enqueue(&self, entry: UrlQueueElement<UrlWithDepth>) -> Result<(), QueueError>;

    #[cfg(test)]
    async fn enqueue_borrowed<'a>(
        &self,
        entry: UrlQueueElement<&'a UrlWithDepth>,
    ) -> Result<(), QueueError>;

    async fn enqueue_all(
        &self,
        entries: impl IntoIterator<Item = UrlQueueElement<UrlWithDepth>>,
    ) -> Result<(), QueueError>;

    async fn dequeue(&self) -> Result<Option<UrlQueueElement<UrlWithDepth>>, QueueError>;

    #[cfg(test)]
    async fn dequeue_n(&self, n: usize) -> Result<Vec<UrlQueueElement<UrlWithDepth>>, QueueError>;

    /// Number of elements in the queue
    async fn len(&self) -> usize;

    /// Returns true if the queue is empty.
    async fn is_empty(&self) -> bool;

    /// Broadcasts if enqueue is called
    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
}
