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

pub mod implementation;

use crate::queue::errors::RawQueueError;
use crate::queue::QueueError;
use itertools::Either;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use tokio::sync::watch::Receiver;

/// A signal sent when enqueue is called on a [RawAgingQueue]
#[derive(Debug, Copy, Clone)]
pub struct EnqueueCalled;

/// An aging queue element
pub trait AgingQueueElement {
    fn age_by_one(&mut self);
}

pub trait RawSupportsForcedQueueElement {
    unsafe fn force_enqueue<T>(&self, entry: T) -> Result<(), QueueError>
    where
        T: AgingQueueElement + Serialize + Debug;
}

/// An unsafe aging queue
pub trait RawAgingQueue: Send + Sync + RawSupportsForcedQueueElement {
    /// Enqueue a value of type [E].
    unsafe fn enqueue_any<T>(
        &self,
        entry: Either<T, Vec<u8>>,
    ) -> Result<(), RawQueueError<Vec<u8>>>
    where
        T: AgingQueueElement + Serialize + Debug;

    /// Enqueue all values of type [E].
    unsafe fn enqueue_any_all<T, I>(
        &self,
        entries: Either<I, Vec<Vec<u8>>>,
    ) -> Result<(), RawQueueError<Vec<Vec<u8>>>>
    where
        T: AgingQueueElement + Serialize + Debug,
        I: IntoIterator<Item = T>;

    /// Dequeues a value of type [E]
    unsafe fn dequeue_any<T>(&self) -> Result<Option<T>, RawQueueError<()>>
    where
        T: AgingQueueElement + DeserializeOwned + Debug;

    /// Dequeues [n] values of type [E]
    unsafe fn dequeue_any_n<T>(&self, n: usize) -> Result<Vec<T>, RawQueueError<()>>
    where
        T: AgingQueueElement + DeserializeOwned + Debug;

    /// Returns the len of the queue
    fn len(&self) -> usize;

    /// Returns the len of the queue
    fn len_nonblocking(&self) -> Result<usize, RawQueueError<()>>;

    /// Returns true if the queue is empty
    fn is_empty(&self) -> bool;

    /// Returns the len of the queue
    fn is_empty_nonblocking(&self) -> Result<bool, RawQueueError<()>>;

    ///Returns a subscribtion to the queue
    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
}
