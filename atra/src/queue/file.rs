// Copyright 2024 Felix Engl
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

use crate::queue::traits::RawAgingQueue;
use crate::queue::{AgingQueueElement, QueueError};
use crate::url::queue::EnqueueCalled;
use log::log_enabled;
use queue_file::QueueFile;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;

/// A mutexed queue for urls that are supported by spider.
#[derive(Debug, Clone)]
pub struct RawAgingQueueFile {
    broadcast: tokio::sync::broadcast::Sender<EnqueueCalled>,
    queue: Arc<Mutex<QueueFile>>,
}

impl RawAgingQueueFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, queue_file::Error> {
        Ok(Self::new_with(QueueFile::open(path)?))
    }

    fn new_with(queue: QueueFile) -> Self {
        Self {
            queue: Arc::new(Mutex::new(queue)),
            broadcast: tokio::sync::broadcast::Sender::new(1),
        }
    }
}

impl RawAgingQueue for RawAgingQueueFile {
    async unsafe fn enqueue_any<E: AgingQueueElement + Serialize + Debug>(
        &self,
        mut entry: E,
    ) -> Result<(), QueueError> {
        log::trace!("Acquire lock.");
        let mut lock = self.queue.lock().await;
        log::trace!("Enqueue {:?}", entry);
        entry.age_by_one();
        lock.add(&bincode::serialize(&entry).map_err(QueueError::EncodingError)?)
            .map_err(QueueError::QueueFileError)?;
        let _ = self.broadcast.send(EnqueueCalled);
        Ok(())
    }

    async unsafe fn enqueue_any_all<E: Into<V>, V: AgingQueueElement + Serialize + Debug>(
        &self,
        entries: impl IntoIterator<Item = E> + Clone,
    ) -> Result<(), QueueError> {
        log::trace!("Acquire lock.");
        let mut lock = self.queue.lock().await;
        log::trace!("Enqueue multiple.");

        let urls: Result<Vec<Vec<u8>>, QueueError> = if log_enabled!(log::Level::Trace) {
            let urls: Vec<_> = entries.into_iter().map(|value| value.into()).collect();
            log::trace!("Enqueue: {:?}", &urls);
            urls.into_iter()
                .map(|mut entry| {
                    entry.age_by_one();
                    bincode::serialize(&entry).map_err(QueueError::EncodingError)
                })
                .collect()
        } else {
            entries
                .into_iter()
                .map(|entry| bincode::serialize(&entry.into()).map_err(QueueError::EncodingError))
                .collect()
        };

        lock.add_n(urls?).map_err(QueueError::QueueFileError)?;
        let _ = self.broadcast.send(EnqueueCalled);
        Ok(())
    }

    async unsafe fn dequeue_any<E: AgingQueueElement + DeserializeOwned + Debug>(
        &self,
    ) -> Result<Option<E>, QueueError> {
        let mut lock = self.queue.lock().await;
        let extracted = lock.peek()?;
        if let Some(extracted) = extracted {
            lock.remove()?;
            let value: E = bincode::deserialize(extracted.as_ref())?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    async unsafe fn dequeue_any_n<E: AgingQueueElement + DeserializeOwned + Debug>(
        &self,
        n: usize,
    ) -> Result<Vec<E>, QueueError> {
        let mut lock = self.queue.lock().await;
        lock.iter()
            .take(n)
            .map(|value| match bincode::deserialize(value.as_ref()) {
                Ok(value) => Ok(value),
                Err(err) => Err(QueueError::EncodingError(err)),
            })
            .collect()
    }

    async fn len(&self) -> usize {
        let lock = self.queue.lock().await;
        lock.size()
    }

    async fn is_empty(&self) -> bool {
        let lock = self.queue.lock().await;
        lock.is_empty()
    }

    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled> {
        self.broadcast.subscribe()
    }
}

impl Default for RawAgingQueueFile {
    fn default() -> Self {
        let mut temp_queue_file = std::env::temp_dir();
        temp_queue_file.push(env!("CARGO_PKG_NAME"));
        temp_queue_file.push(env!("CARGO_PKG_VERSION"));
        temp_queue_file.push(uuid::Uuid::new_v4().as_simple().to_string());
        std::fs::create_dir_all(temp_queue_file.clone()).unwrap();
        temp_queue_file.push("queue");
        Self {
            queue: Arc::new(Mutex::new(
                QueueFile::open(temp_queue_file.as_path()).unwrap(),
            )),
            broadcast: tokio::sync::broadcast::Sender::new(1),
        }
    }
}
