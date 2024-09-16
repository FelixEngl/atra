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
use crate::queue::raw::implementation::RawAgingQueueFile;
use crate::queue::raw::RawAgingQueue;
use crate::queue::url::{PollWaiterFactory, UrlQueue, UrlQueueElement};
use crate::queue::EnqueueCalled;
use crate::url::UrlWithDepth;
use std::path::Path;
use tokio::sync::broadcast::Receiver;
use crate::queue::url::poll::{PollWaiter};

#[derive(Debug)]
pub struct UrlQueueWrapper<T: RawAgingQueue> {
    inner: T,
    factory: PollWaiterFactory
}

impl UrlQueueWrapper<RawAgingQueueFile> {
    /// Opens as a raw file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, queue_file::Error> {
        Ok(Self {
            inner: RawAgingQueueFile::open(path)?,
            factory: PollWaiterFactory::new()
        })
    }
}

impl<T: RawAgingQueue> UrlQueueWrapper<T> {
    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// An url queue provides a threadsafe way to get values.
impl<T: RawAgingQueue> UrlQueue for UrlQueueWrapper<T> {
    #[inline]
    async fn enqueue(&self, entry: UrlQueueElement<UrlWithDepth>) -> Result<(), QueueError> {
        unsafe { self.inner.enqueue_any(entry).await }
    }

    #[cfg(test)]
    async fn enqueue_borrowed<'a>(
        &self,
        entry: UrlQueueElement<&'a UrlWithDepth>,
    ) -> Result<(), QueueError> {
        unsafe { self.inner.enqueue_any(entry).await }
    }

    #[inline]
    async fn enqueue_all(
        &self,
        entries: impl IntoIterator<Item = UrlQueueElement<UrlWithDepth>>,
    ) -> Result<(), QueueError> {
        unsafe { self.inner.enqueue_any_all(entries).await }
    }

    #[inline]
    async fn dequeue(&self) -> Result<Option<UrlQueueElement<UrlWithDepth>>, QueueError> {
        unsafe { self.inner.dequeue_any().await }
    }

    #[cfg(test)]
    async fn dequeue_n(&self, n: usize) -> Result<Vec<UrlQueueElement<UrlWithDepth>>, QueueError> {
        unsafe { self.inner.dequeue_any_n(n).await }
    }

    /// Number of elements in the queue
    #[inline]
    async fn len(&self) -> usize {
        self.inner.len().await
    }

    /// Returns true if the queue is empty.
    #[inline]
    async fn is_empty(&self) -> bool {
        self.inner.is_empty().await
    }

    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled> {
        self.inner.subscribe_to_change()
    }

    fn start_polling(&self) -> PollWaiter {
        self.factory.create()
    }
}

impl<T: RawAgingQueue> From<T> for UrlQueueWrapper<T> {
    fn from(value: T) -> Self {
        Self{inner: value, factory: PollWaiterFactory::new()}
    }
}

#[cfg(test)]
mod test {
    use crate::queue::url::element::UrlQueueElement;
    use crate::queue::url::queue::{UrlQueue, UrlQueueWrapper};
    use crate::url::UrlWithDepth;
    use scopeguard::defer;


    pub async fn test_queue1(q: impl UrlQueue) {
        q.enqueue_seed("https://www.test1.de").await.unwrap();
        q.enqueue_seed("https://www.test2.de").await.unwrap();
        q.enqueue_seed("https://www.test3.de").await.unwrap();
        assert_eq!(3, q.len().await);
        assert_eq!(
            "https://www.test1.de/",
            q.dequeue().await.unwrap().unwrap().as_ref().as_str()
        );
        assert_eq!(
            "https://www.test2.de/",
            q.dequeue().await.unwrap().unwrap().as_ref().as_str()
        );
        assert_eq!(
            "https://www.test3.de/",
            q.dequeue().await.unwrap().unwrap().as_ref().as_str()
        );
    }

    pub async fn test_queue2(q: impl UrlQueue) {
        q.enqueue_all([
            UrlQueueElement::new(
                true,
                0,
                false,
                UrlWithDepth::from_seed("https://www.test1.de").unwrap(),
            ),
            UrlQueueElement::new(
                true,
                0,
                false,
                UrlWithDepth::from_seed("https://www.test2.de").unwrap(),
            ),
            UrlQueueElement::new(
                true,
                0,
                false,
                UrlWithDepth::from_seed("https://www.test3.de").unwrap(),
            ),
        ])
            .await
            .unwrap();
        let values = q.dequeue_n(3).await.unwrap();
        assert_eq!("https://www.test1.de/", values[0].as_ref().as_str());
        assert_eq!("https://www.test2.de/", values[1].as_ref().as_str());
        assert_eq!("https://www.test3.de/", values[2].as_ref().as_str());
        q.enqueue(
            UrlQueueElement::new(
                true,
                0,
                false,
                UrlWithDepth::from_seed("https://www.test4.de").unwrap(),
            )
        ).await.unwrap();

        q.enqueue(
            UrlQueueElement::new(
                true,
                0,
                false,
                UrlWithDepth::from_seed("https://www.test5.de").unwrap(),
            )
        ).await.unwrap();

        assert_eq!("https://www.test4.de/", q.dequeue().await.unwrap().unwrap().as_ref().as_str());
        assert_eq!("https://www.test5.de/", q.dequeue().await.unwrap().unwrap().as_ref().as_str());

        q.enqueue(
            UrlQueueElement::new(
                true,
                0,
                false,
                UrlWithDepth::from_seed("https://www.test6.de").unwrap(),
            )
        ).await.unwrap();

        assert_eq!("https://www.test6.de/", q.dequeue().await.unwrap().unwrap().as_ref().as_str());
    }

    #[tokio::test]
    async fn can_initialize() {
        defer! {
            let _ = std::fs::remove_file("test0.q");
        }
        let _ = std::fs::remove_file("test0.q");
        test_queue1(UrlQueueWrapper::open("test0.q").unwrap()).await
    }

    #[tokio::test]
    async fn can_initialize_many() {
        defer! {
            let _ = std::fs::remove_file("test1.q");
        }
        let _ = std::fs::remove_file("test1.q");
        test_queue2(UrlQueueWrapper::open("test1.q").unwrap()).await
    }

    #[tokio::test]
    async fn test_impl_behaves_similar() {
        test_queue1(crate::test_impls::TestUrlQueue::default()).await;
        test_queue2(crate::test_impls::TestUrlQueue::default()).await;
    }
}
