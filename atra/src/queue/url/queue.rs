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

use crate::queue::errors::{QueueError, RawQueueError};
use crate::queue::raw::implementation::RawAgingQueueFile;
use crate::queue::raw::RawAgingQueue;
use crate::queue::url::{
    SupportsForcedQueueElement, UrlQueue, UrlQueueElement, UrlQueueElementRef,
    UrlQueueElementRefCounter,
};
use crate::queue::{EnqueueCalled, RawSupportsForcedQueueElement};
use crate::url::UrlWithDepth;
use clap::builder::TypedValueParser;
use itertools::{Either, Itertools};
use nom::Parser;
use std::future::Future;
use std::ops::ControlFlow;
use std::path::Path;
use tokio::sync::watch::Receiver;
use tokio::task::yield_now;

#[derive(Debug)]
pub struct UrlQueueWrapper<T: RawAgingQueue> {
    inner: T,
    counter: UrlQueueElementRefCounter,
}

impl UrlQueueWrapper<RawAgingQueueFile> {
    /// Opens as a raw file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, queue_file::Error> {
        Ok(Self::new(RawAgingQueueFile::open(path)?))
    }
}

impl<T> UrlQueueWrapper<T>
where
    T: RawAgingQueue + RawSupportsForcedQueueElement,
{
    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        self.inner
    }

    #[inline]
    fn wrap(&self, result: UrlQueueElement<UrlWithDepth>) -> UrlQueueElementRef<UrlWithDepth> {
        let drop = self.counter.create_drop_notifyer();
        UrlQueueElementRef::new(result, self, drop)
    }

    fn convert_result<V, U>(
        result: Result<V, RawQueueError<U>>,
    ) -> ControlFlow<Result<V, QueueError>, U> {
        match result {
            Ok(value) => ControlFlow::Break(Ok(value)),
            Err(err) => match err.try_into() {
                Ok(err) => ControlFlow::Break(Err(err)),
                Err(v) => ControlFlow::Continue(v),
            },
        }
    }

    pub fn new(inner: T) -> Self {
        Self {
            inner,
            counter: UrlQueueElementRefCounter::new(),
        }
    }
}

impl<T> SupportsForcedQueueElement<UrlWithDepth> for UrlQueueWrapper<T>
where
    T: RawAgingQueue + RawSupportsForcedQueueElement,
{
    fn force_enqueue(&self, entry: UrlQueueElement<UrlWithDepth>) -> Result<(), QueueError> {
        unsafe { self.inner.force_enqueue(entry) }
    }
}

/// An url queue provides a threadsafe way to get values.
impl<T: RawAgingQueue> UrlQueue<UrlWithDepth> for UrlQueueWrapper<T> {
    #[inline]
    async fn enqueue(&self, mut entry: UrlQueueElement<UrlWithDepth>) -> Result<(), QueueError> {
        let mut entry = Either::Left(entry);
        loop {
            unsafe {
                match Self::convert_result(self.inner.enqueue_any(entry)) {
                    ControlFlow::Break(result) => return result,
                    ControlFlow::Continue(v) => {
                        entry = Either::Right(v);
                        yield_now().await
                    }
                }
            }
        }
    }

    #[cfg(test)]
    async fn enqueue_borrowed(
        &self,
        entry: UrlQueueElement<&UrlWithDepth>,
    ) -> Result<(), QueueError> {
        let mut entry = Either::Left(entry);
        loop {
            unsafe {
                match Self::convert_result(self.inner.enqueue_any(entry)) {
                    ControlFlow::Break(result) => return result,
                    ControlFlow::Continue(v) => {
                        entry = Either::Right(v);
                        yield_now().await
                    }
                }
            }
        }
    }

    #[inline]
    async fn enqueue_all(
        &self,
        entries: impl IntoIterator<Item = UrlQueueElement<UrlWithDepth>>,
    ) -> Result<(), QueueError> {
        let mut entries = Either::Left(entries);
        loop {
            unsafe {
                match Self::convert_result(self.inner.enqueue_any_all(entries)) {
                    ControlFlow::Break(result) => return result,
                    ControlFlow::Continue(v) => {
                        entries = Either::Right(v);
                        yield_now().await
                    }
                }
            }
        }
    }

    async fn dequeue<'a>(
        &'a self,
    ) -> Result<Option<UrlQueueElementRef<'a, UrlWithDepth>>, QueueError> {
        loop {
            match Self::convert_result(unsafe { self.inner.dequeue_any() }) {
                ControlFlow::Break(Ok(Some(value))) => return Ok(Some(self.wrap(value))),
                ControlFlow::Break(Ok(None)) => return Ok(None),
                ControlFlow::Break(Err(err)) => return Err(err),
                ControlFlow::Continue(_) => yield_now().await,
            }
        }
    }

    async fn dequeue_n<'a>(
        &'a self,
        n: usize,
    ) -> Result<Vec<UrlQueueElementRef<'a, UrlWithDepth>>, QueueError> {
        loop {
            match Self::convert_result(unsafe { self.inner.dequeue_any_n(n) }) {
                ControlFlow::Break(Ok(value)) => {
                    return Ok(value
                        .into_iter()
                        .map(|value| self.wrap(value))
                        .collect_vec())
                }
                ControlFlow::Break(Err(err)) => return Err(err),
                ControlFlow::Continue(_) => yield_now().await,
            }
        }
    }

    /// Number of elements in the queue
    #[inline]
    async fn len(&self) -> usize {
        loop {
            match Self::convert_result(self.inner.len_nonblocking()) {
                ControlFlow::Break(Ok(size)) => return size + self.counter.get_count(),
                _ => yield_now().await,
            }
        }
    }

    /// Returns true if the queue is empty.
    #[inline]
    async fn is_empty(&self) -> bool {
        loop {
            match Self::convert_result(self.inner.is_empty_nonblocking()) {
                ControlFlow::Break(Ok(empty)) => return empty && self.counter.get_count() == 0,
                ControlFlow::Break(Err(err)) => {
                    panic!("The queue had an error that is unrecoverable! {}", err)
                }
                _ => yield_now().await,
            }
        }
    }

    fn has_floating_urls(&self) -> bool {
        self.counter.awaits_drops()
    }

    fn floating_url_count(&self) -> usize {
        self.counter.get_count()
    }

    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled> {
        self.inner.subscribe_to_change()
    }
}

impl<T: RawAgingQueue> From<T> for UrlQueueWrapper<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod test {
    use crate::queue::url::element::UrlQueueElement;
    use crate::queue::url::queue::{UrlQueue, UrlQueueWrapper};
    use crate::queue::SupportsSeeding;
    use crate::url::UrlWithDepth;
    use itertools::Itertools;
    use scopeguard::defer;

    pub async fn test_queue1(q: impl UrlQueue<UrlWithDepth>) {
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

    pub async fn test_queue2(q: impl UrlQueue<UrlWithDepth>) {
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
        let values = q
            .dequeue_n(3)
            .await
            .unwrap()
            .into_iter()
            .map(|value| value.take())
            .collect_vec();

        assert_eq!("https://www.test1.de/", values[0].as_ref().as_str());
        assert_eq!("https://www.test2.de/", values[1].as_ref().as_str());
        assert_eq!("https://www.test3.de/", values[2].as_ref().as_str());
        q.enqueue(UrlQueueElement::new(
            true,
            0,
            false,
            UrlWithDepth::from_seed("https://www.test4.de").unwrap(),
        ))
        .await
        .unwrap();

        q.enqueue(UrlQueueElement::new(
            true,
            0,
            false,
            UrlWithDepth::from_seed("https://www.test5.de").unwrap(),
        ))
        .await
        .unwrap();

        assert_eq!(
            "https://www.test4.de/",
            q.dequeue().await.unwrap().unwrap().take().as_ref().as_str()
        );
        assert_eq!(
            "https://www.test5.de/",
            q.dequeue().await.unwrap().unwrap().take().as_ref().as_str()
        );

        q.enqueue(UrlQueueElement::new(
            true,
            0,
            false,
            UrlWithDepth::from_seed("https://www.test6.de").unwrap(),
        ))
        .await
        .unwrap();

        assert_eq!(
            "https://www.test6.de/",
            q.dequeue().await.unwrap().unwrap().take().as_ref().as_str()
        );
    }

    async fn test_queue3(q: impl UrlQueue<UrlWithDepth>) {
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

        let value1 = q.dequeue().await.unwrap().unwrap();
        let value2 = q.dequeue().await.unwrap().unwrap();
        assert_eq!(true, q.has_floating_urls());
        assert_eq!(2, q.floating_url_count());
        drop(value1);
        assert_eq!(true, q.has_floating_urls());
        assert_eq!(1, q.floating_url_count());
        let value3 = q.dequeue().await.unwrap().unwrap();
        let value4 = q.dequeue().await.unwrap().unwrap();
        assert_eq!(true, q.has_floating_urls());
        assert_eq!(3, q.floating_url_count());
        assert_eq!("https://www.test2.de/", value2.as_ref().as_str());
        assert_eq!("https://www.test3.de/", value3.as_ref().as_str());
        assert_eq!("https://www.test1.de/", value4.as_ref().as_str());
        drop(value2);
        drop(value3);
        drop(value4);
        assert_eq!(3, q.len().await);
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
    async fn can_execute_many() {
        defer! {
            let _ = std::fs::remove_file("test2.q");
        }
        let _ = std::fs::remove_file("test2.q");
        test_queue3(UrlQueueWrapper::open("test2.q").unwrap()).await
    }

    #[tokio::test]
    async fn test_impl_behaves_similar() {
        test_queue1(crate::test_impls::TestUrlQueue::default()).await;
        test_queue2(crate::test_impls::TestUrlQueue::default()).await;
        test_queue3(crate::test_impls::TestUrlQueue::default()).await;
    }
}
