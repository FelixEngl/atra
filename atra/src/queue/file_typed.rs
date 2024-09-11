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

use std::fmt::{Debug};
use std::marker::PhantomData;
use std::path::Path;
use serde::{Serialize};
use serde::de::DeserializeOwned;
use tokio::sync::broadcast::Receiver;
use crate::queue::{AgingQueueElement, QueueError};
use crate::queue::file::RawAgingQueueFile;
use crate::queue::traits::{AgingQueue, RawAgingQueue};
use crate::url::queue::EnqueueCalled;

pub trait TypedAgingQueueElement: AgingQueueElement + Serialize + DeserializeOwned + Debug{}

impl<T: AgingQueueElement + Serialize + DeserializeOwned + Debug> TypedAgingQueueElement for T{}

// todo: cache structure for helper?

/// A mutexed queue for urls that are supported by spider.
#[derive(Debug, Clone)]
pub struct AgingQueueFile<T: TypedAgingQueueElement> {
    queue: RawAgingQueueFile,
    _element_typ: PhantomData<T>
}

impl<T: TypedAgingQueueElement> AgingQueueFile<T> {
    #[allow(dead_code)]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, queue_file::Error> {
        Ok(Self::new_with(RawAgingQueueFile::open(path)?))
    }

    #[allow(dead_code)]
    fn new_with(queue: RawAgingQueueFile) -> Self {
        Self {
            queue: queue,
            _element_typ: PhantomData
        }
    }
}

impl<T: TypedAgingQueueElement> RawAgingQueue for AgingQueueFile<T> {
    delegate::delegate! {
        to self.queue {
            async unsafe fn enqueue_any<E: AgingQueueElement + Serialize + Debug>(&self, entry: E) -> Result<(), QueueError>;

            async unsafe fn enqueue_any_all<E: Into<V>, V: AgingQueueElement + Serialize + Debug>(&self, entries: impl IntoIterator<Item = E> + Clone) -> Result<(), QueueError>;

            async unsafe fn dequeue_any<E: AgingQueueElement + DeserializeOwned + Debug>(&self) -> Result<Option<E>, QueueError>;

            async unsafe fn dequeue_any_n<E: AgingQueueElement + DeserializeOwned + Debug>(&self, n: usize) -> Result<Vec<E>, QueueError>;

            async fn len(&self) -> usize;

            async fn is_empty(&self) -> bool;

            fn subscribe_to_change(&self) -> Receiver<EnqueueCalled>;
        }
    }
}

impl<T: TypedAgingQueueElement> AgingQueue<T> for AgingQueueFile<T>{}

impl<T: TypedAgingQueueElement> Default for AgingQueueFile<T> {
    fn default() -> Self {
        Self {
            queue: RawAgingQueueFile::default(),
            _element_typ: PhantomData
        }
    }
}

