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

use std::fmt::Debug;
use serde::{Serialize};
use serde::de::DeserializeOwned;
use tokio::sync::broadcast::Receiver;
use crate::core::queue::QueueError;
use crate::core::url::queue::EnqueueCalled;

/// An aging queue element
pub trait AgingQueueElement {
    fn age_by_one(&mut self);
}



/// An unsafe aging queue
pub trait RawAgingQueue {

    /// Enqueue a value of type [E].
    async unsafe fn enqueue_any<E: AgingQueueElement + Serialize + Debug>(&self, entry: E) -> Result<(), QueueError>;

    /// Enqueue all values of type [E].
    async unsafe fn enqueue_any_all<E: Into<V>, V: AgingQueueElement + Serialize + Debug>(&self, entries: impl IntoIterator<Item = E> + Clone) -> Result<(), QueueError>;

    /// Dequeues a value of type [E]
    async unsafe fn dequeue_any<E: AgingQueueElement + DeserializeOwned + Debug>(&self) -> Result<Option<E>, QueueError>;

    /// Dequeues [n] values of type [E]
    async unsafe fn dequeue_any_n<E: AgingQueueElement + DeserializeOwned + Debug>(&self, n: usize) -> Result<Vec<E>, QueueError>;

    /// Returns the len of the queue
    async fn len(&self) -> usize;

    /// Returns true if the queue is empty
    async fn is_empty(&self) -> bool;

    ///Returns a subscribtion to the queue
    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
}

/// A typed version of the [RawAgingQueue]
#[allow(dead_code)]
pub trait AgingQueue<T: AgingQueueElement + Serialize + DeserializeOwned + Debug>: RawAgingQueue {
    /// Enqueue a value of type [T].
    #[inline] async fn enqueue(&self, entry: T) -> Result<(), QueueError> {
        unsafe {self.enqueue_any(entry).await}
    }

    /// Enqueue all values of type [T].
    #[inline] async fn enqueue_all<E: Into<T>>(&self, entries: impl IntoIterator<Item = E> + Clone) -> Result<(), QueueError> {
        unsafe { self.enqueue_any_all(entries).await }
    }

    /// Dequeues a value of type [T]
    #[inline] async fn dequeue(&self) -> Result<Option<T>, QueueError> {
        unsafe {self.dequeue_any().await}
    }

    /// Dequeues [n] values of type [T]
    #[inline] async fn dequeue_n(&self, n: usize) -> Result<Vec<T>, QueueError> {
        unsafe {self.dequeue_any_n(n).await}
    }
}