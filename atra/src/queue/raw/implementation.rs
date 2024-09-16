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

use crate::queue::errors::RawQueueError;
use crate::queue::raw::{AgingQueueElement, EnqueueCalled, RawAgingQueue, RawSupportsForcedQueueElement};
use queue_file::QueueFile;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError, TryLockResult};
use tokio::sync::watch::Receiver;
use itertools::{Either, Itertools};
use crate::queue::QueueError;

/// A mutexed queue for urls that are supported by spider.
#[derive(Debug, Clone)]
pub struct RawAgingQueueFile {
    broadcast: tokio::sync::watch::Sender<EnqueueCalled>,
    queue: Arc<RwLock<QueueFile>>,
}

impl RawAgingQueueFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, queue_file::Error> {
        Ok(Self::new_with(QueueFile::open(path)?))
    }

    fn new_with(queue: QueueFile) -> Self {
        Self {
            queue: Arc::new(RwLock::new(queue)),
            broadcast: tokio::sync::watch::Sender::new(EnqueueCalled),
        }
    }
}

impl RawSupportsForcedQueueElement for RawAgingQueueFile {
    unsafe fn force_enqueue<T>(&self, mut entry: T) -> Result<(), QueueError>
    where
        T: AgingQueueElement + Serialize + Debug
    {
        log::trace!("Encode {:?}", entry);
        entry.age_by_one();
        let encoded = bincode::serialize(&entry).map_err(QueueError::EncodingError)?;

        log::trace!("Acquire lock.");
        let mut lock = self.queue.write().unwrap();
        log::trace!("Enqueue the entry {:?}", entry);
        lock.add(&encoded).map_err(QueueError::QueueFileError)?;
        drop(lock);

        let _ = self.broadcast.send(EnqueueCalled);
        Ok(())
    }
}

impl RawAgingQueue for RawAgingQueueFile {
    unsafe fn enqueue_any<E: AgingQueueElement + Serialize + Debug>(
        &self,
        entry: Either<E, Vec<u8>>,
    ) -> Result<(), RawQueueError<Vec<u8>>> {
        let encoded = match entry {
            Either::Left(mut entry) => {
                entry.age_by_one();
                bincode::serialize(&entry).map_err(RawQueueError::EncodingError)?
            }
            Either::Right(encoded) => {
                encoded
            }
        };

        match self.queue.try_write() {
            Ok(mut lock) => {
                lock.add(&encoded).map_err(RawQueueError::QueueFileError)?;
                drop(lock);
            }
            Err(err) => {
                match err {
                    TryLockError::Poisoned(_) => {}
                    TryLockError::WouldBlock => {
                        return Err(RawQueueError::Blocked(encoded))
                    }
                }
            }
        }

        let _ = self.broadcast.send(EnqueueCalled);
        Ok(())
    }

    unsafe fn enqueue_any_all<V, I>(
        &self,
        entries: Either<I, Vec<Vec<u8>>>,
    ) -> Result<(), RawQueueError<Vec<Vec<u8>>>>
    where
        V: AgingQueueElement + Serialize + Debug,
        I: IntoIterator<Item = V>
    {
        let urls: Vec<Vec<u8>> = match entries {
            Either::Left(entries) => {
                entries
                    .into_iter()
                    .map(|mut entry| {
                        entry.age_by_one();
                        bincode::serialize(&entry).map_err(RawQueueError::EncodingError)
                    })
                    .collect::<Result<_, _>>()?
            }
            Either::Right(urls) => {
                urls
            }
        };
        match self.queue.try_write() {
            Ok(mut lock) => {
                lock.add_n(urls).map_err(RawQueueError::QueueFileError)?;
                drop(lock);
            }
            Err(err) => {
                match err {
                    TryLockError::Poisoned(_) => {}
                    TryLockError::WouldBlock => {
                        return Err(RawQueueError::Blocked(urls))
                    }
                }
            }
        }

        let _ = self.broadcast.send(EnqueueCalled);
        Ok(())
    }

    unsafe fn dequeue_any<E: AgingQueueElement + DeserializeOwned + Debug>(
        &self,
    ) -> Result<Option<E>, RawQueueError<()>> {
        let mut lock = match self.queue.try_write() {
            Ok(lock) => {lock}
            Err(err) => {
                match err {
                    TryLockError::Poisoned(_) => {
                        return Err(RawQueueError::LockPoisoned)
                    }
                    TryLockError::WouldBlock => {
                        return Err(RawQueueError::Blocked(()))
                    }
                }
            }
        };
        let extracted = lock.peek()?;
        if let Some(extracted) = extracted {
            lock.remove()?;
            drop(lock);
            let value: E = bincode::deserialize(extracted.as_ref())?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    unsafe fn dequeue_any_n<E: AgingQueueElement + DeserializeOwned + Debug>(
        &self,
        n: usize,
    ) -> Result<Vec<E>, RawQueueError<()>> {
        let mut lock = match self.queue.try_write() {
            Ok(lock) => {lock}
            Err(err) => {
                match err {
                    TryLockError::Poisoned(_) => {
                        return Err(RawQueueError::LockPoisoned)
                    }
                    TryLockError::WouldBlock => {
                        return Err(RawQueueError::Blocked(()))
                    }
                }
            }
        };
        let found = lock.iter().take(n).collect_vec();
        lock.remove_n(n)?;
        drop(lock);
        found.into_iter().map(|value| match bincode::deserialize(value.as_ref()) {
            Ok(value) => Ok(value),
            Err(err) => Err(RawQueueError::EncodingError(err)),
        }).collect::<Result<Vec<_>, _>>()
    }

    fn len(&self) -> usize {
        let lock = self.queue.read().unwrap();
        lock.size()
    }

    fn len_nonblocking(&self) -> Result<usize, RawQueueError<()>> {
        match self.queue.try_read() {
            Ok(lock) => {
                Ok(lock.size())
            }
            Err(err) => {
                match err {
                    TryLockError::Poisoned(_) => {
                        Err(RawQueueError::LockPoisoned)
                    }
                    TryLockError::WouldBlock => {
                        Err(RawQueueError::Blocked(()))
                    }
                }
            }
        }

    }


    fn is_empty(&self) -> bool {
        let lock = self.queue.read().unwrap();
        lock.is_empty()
    }

    fn is_empty_nonblocking(&self) -> Result<bool, RawQueueError<()>> {
        match self.queue.try_read() {
            Ok(lock) => {
                Ok(lock.is_empty())
            }
            Err(err) => {
                match err {
                    TryLockError::Poisoned(_) => {
                        Err(RawQueueError::LockPoisoned)
                    }
                    TryLockError::WouldBlock => {
                        Err(RawQueueError::Blocked(()))
                    }
                }
            }
        }
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
            queue: Arc::new(RwLock::new(
                QueueFile::open(temp_queue_file.as_path()).unwrap(),
            )),
            broadcast: tokio::sync::watch::Sender::new(EnqueueCalled),
        }
    }
}
