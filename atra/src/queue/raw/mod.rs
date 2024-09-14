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

pub mod errors;
pub mod implementation;

use crate::queue::raw::errors::QueueError;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use tokio::sync::broadcast::Receiver;

/// A signal sent when enqueue is called on a [RawAgingQueue]
#[derive(Debug, Copy, Clone)]
pub struct EnqueueCalled;

/// An aging queue element
pub trait AgingQueueElement {
    fn age_by_one(&mut self);
}

/// An unsafe aging queue
#[allow(dead_code)]
pub trait RawAgingQueue {
    /// Enqueue a value of type [E].
    async unsafe fn enqueue_any<T>(&self, entry: T) -> Result<(), QueueError>
    where
        T: AgingQueueElement + Serialize + Debug;

    /// Enqueue all values of type [E].
    async unsafe fn enqueue_any_all<T>(
        &self,
        entries: impl IntoIterator<Item = T>,
    ) -> Result<(), QueueError>
    where
        T: AgingQueueElement + Serialize + Debug;

    /// Dequeues a value of type [E]
    async unsafe fn dequeue_any<T>(&self) -> Result<Option<T>, QueueError>
    where
        T: AgingQueueElement + DeserializeOwned + Debug;

    /// Dequeues [n] values of type [E]
    async unsafe fn dequeue_any_n<T>(&self, n: usize) -> Result<Vec<T>, QueueError>
    where
        T: AgingQueueElement + DeserializeOwned + Debug;

    /// Returns the len of the queue
    async fn len(&self) -> usize;

    /// Returns true if the queue is empty
    async fn is_empty(&self) -> bool;

    ///Returns a subscribtion to the queue
    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
}
