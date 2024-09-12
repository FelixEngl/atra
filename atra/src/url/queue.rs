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

pub(crate) mod element;

use std::path::Path;
use tokio::sync::broadcast::Receiver;
pub use element::UrlQueueElementBase;
use crate::queue::{QueueError};
use crate::queue::RawAgingQueueFile;
use crate::queue::RawAgingQueue;
use crate::url::UrlWithDepth;

/// The element in an url queue
pub type UrlQueueElement = UrlQueueElementBase<UrlWithDepth>;
pub type UrlQueueElementWeak<'a> = UrlQueueElementBase<&'a UrlWithDepth>;




/// A traif for an url queue
pub trait UrlQueue {
    /// Enqueues an [url] at distance 0
    async fn enqueue_seed(&self, url: &str) -> Result<(), QueueError> {
        self.enqueue(UrlQueueElementWeak::new(true, 0, false, &UrlWithDepth::from_seed(url)?)).await
    }

    /// Enqueues all [urls] at distance 0
    async fn enqueue_seeds(&self, urls: impl IntoIterator<Item = impl AsRef<str>> + Clone) -> Result<(), QueueError> {
        self.enqueue_all(
            urls.into_iter()
                .map(|s| UrlWithDepth::from_seed(s.as_ref()).map(|value| UrlQueueElement::new(true, 0, false, value)))
                .collect::<Result<Vec<_>, _>>()?
        ).await
    }

    async fn enqueue<'a>(&self, entry: UrlQueueElementWeak<'a>) -> Result<(), QueueError>;

    async fn enqueue_all<E: Into<UrlQueueElement>>(&self, entries: impl IntoIterator<Item=E> + Clone) -> Result<(), QueueError>;

    async fn dequeue(&self) -> Result<Option<UrlQueueElement>, QueueError>;

    async fn dequeue_n(&self, n: usize) -> Result<Vec<UrlQueueElement>, QueueError>;


    /// Number of elements in the queue
    async fn len(&self) -> usize;

    /// Returns true if the queue is empty.
    async fn is_empty(&self) -> bool;

    /// Broadcasts if enqueue is called
    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
}

#[derive(Debug, Copy, Clone)]
pub struct EnqueueCalled;

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct UrlQueueWrapper<T: RawAgingQueue>(T);

impl UrlQueueWrapper<RawAgingQueueFile> {

    /// Opens as a raw file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, queue_file::Error> {
        Ok(Self(RawAgingQueueFile::open(path)?))
    }
}

impl<T: RawAgingQueue> UrlQueueWrapper<T> {
    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        self.0
    }
}


/// An url queue provides a threadsafe way to get values.
impl<T: RawAgingQueue> UrlQueue for  UrlQueueWrapper<T> {

    #[inline] async fn enqueue<'a>(&self, entry: UrlQueueElementWeak<'a>) -> Result<(), QueueError> {
        unsafe { self.0.enqueue_any(entry).await }
    }

    #[inline] async fn enqueue_all<E: Into<UrlQueueElement>>(&self, entries: impl IntoIterator<Item=E> + Clone) -> Result<(), QueueError> {
        unsafe { self.0.enqueue_any_all(entries).await }
    }

    #[inline] async fn dequeue(&self) -> Result<Option<UrlQueueElement>, QueueError> {
        unsafe { self.0.dequeue_any().await }
    }

    #[inline] async fn dequeue_n(&self, n: usize) -> Result<Vec<UrlQueueElement>, QueueError> {
        unsafe { self.0.dequeue_any_n(n).await }
    }

    /// Number of elements in the queue
    #[inline] async fn len(&self) -> usize {
         self.0.len().await
    }

    /// Returns true if the queue is empty.
    #[inline] async fn is_empty(&self) -> bool  {
        self.0.is_empty().await
    }

    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled> {
        self.0.subscribe_to_change()
    }
}

impl<T: RawAgingQueue> From<T> for UrlQueueWrapper<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod test {
    use scopeguard::defer;
    use crate::url::queue::{UrlQueue, UrlQueueWrapper};
    use crate::url::UrlWithDepth;

    #[tokio::test]
    async fn can_initialize(){
        defer! {
            let _ = std::fs::remove_file("test0.q");
        }
        let init = UrlQueueWrapper::open("test0.q").unwrap();
        init.enqueue_seed("https://www.test1.de").await.unwrap();
        init.enqueue_seed("https://www.test2.de").await.unwrap();
        init.enqueue_seed("https://www.test3.de").await.unwrap();
        assert_eq!(3, init.len().await);
        assert_eq!("test1", init.dequeue().await.unwrap().unwrap().as_ref().as_str());
        assert_eq!("test2", init.dequeue().await.unwrap().unwrap().as_ref().as_str());
        assert_eq!("test3", init.dequeue().await.unwrap().unwrap().as_ref().as_str());
    }

    #[tokio::test]
    async fn can_initialize_many(){
        defer! {
            let _ = std::fs::remove_file("test1.q");
        }
        let init = UrlQueueWrapper::open("test1.q").unwrap();
        init.enqueue_all([
            (true, 0, false, UrlWithDepth::from_seed("https://www.test1.de").unwrap()),
            (true, 0, false, UrlWithDepth::from_seed("https://www.test2.de").unwrap()),
            (true, 0, false, UrlWithDepth::from_seed("https://www.test3.de").unwrap())]).await.unwrap();
        let values = init.dequeue_n(3).await.unwrap();
        assert_eq!("https://www.test1.de", values[0].as_ref().as_str());
        assert_eq!("https://www.test2.de", values[1].as_ref().as_str());
        assert_eq!("https://www.test3.de", values[2].as_ref().as_str());
    }
}