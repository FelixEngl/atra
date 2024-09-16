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

use crate::queue::url::element::UrlQueueElement;
use crate::queue::url::SupportsForcedQueueElement;
use crate::toolkit::dropping::{DropNotifyer, DropNotifyerEvent};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use tokio::sync::watch::{channel, Receiver, Sender};

#[derive(Clone)]
pub struct UrlQueueElementRefCounter {
    receiver: Receiver<UrlQueueElementRefCounterEvent>,
    sender: Sender<UrlQueueElementRefCounterEvent>,
}

impl UrlQueueElementRefCounter {
    pub fn new() -> Self {
        let (sender, receiver) = channel(UrlQueueElementRefCounterEvent::new(0));
        Self { sender, receiver }
    }

    pub fn get_count(&self) -> usize {
        self.receiver.borrow().count
    }

    pub fn awaits_drops(&self) -> bool {
        self.get_count() > 0
    }

    pub fn create_drop_notifyer(&self) -> DropNotifyer<UrlQueueElementRefCounterEvent> {
        self.sender.send_modify(|value| value.inc());
        let new = DropNotifyer::new(self.sender.clone());
        new
    }
}

impl Debug for UrlQueueElementRefCounter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlQueueElementRefCounter")
            .field("count", &self.get_count())
            .finish()
    }
}

#[repr(transparent)]
pub struct UrlQueueElementRefCounterEvent {
    count: usize,
}

impl UrlQueueElementRefCounterEvent {
    pub fn inc(&mut self) {
        self.count += 1;
    }

    pub fn new(count: usize) -> Self {
        Self { count }
    }
}

impl DropNotifyerEvent for UrlQueueElementRefCounterEvent {
    fn on_drop(&mut self) -> bool {
        #[cfg(not(test))]
        {
            self.count -= 1;
        }
        #[cfg(test)]
        {
            let (new, overflow) = self.count.overflowing_sub(1);
            self.count = new;
            debug_assert!(!overflow, "Overflow when dropping!")
        }

        true
    }
}

#[clippy::has_significant_drop]
pub struct UrlQueueElementRef<'a, T>
where
    T: 'static + Serialize + DeserializeOwned,
{
    element: Option<UrlQueueElement<T>>,
    backend: &'a dyn SupportsForcedQueueElement<T>,
    _drop_informer: DropNotifyer<UrlQueueElementRefCounterEvent>,
}

unsafe impl<'a, T> Send for UrlQueueElementRef<'a, T> where T: 'static + Serialize + DeserializeOwned
{}
unsafe impl<'a, T> Sync for UrlQueueElementRef<'a, T> where T: 'static + Serialize + DeserializeOwned
{}

impl<'a, T> UrlQueueElementRef<'a, T>
where
    T: 'static + Serialize + DeserializeOwned,
{
    pub fn new(
        element: UrlQueueElement<T>,
        backend: &'a impl SupportsForcedQueueElement<T>,
        notifyer: DropNotifyer<UrlQueueElementRefCounterEvent>,
    ) -> Self {
        Self {
            element: Some(element),
            backend,
            _drop_informer: notifyer,
        }
    }

    #[inline(always)]
    fn take_impl(mut self) -> UrlQueueElement<T> {
        unsafe { self.element.take().unwrap_unchecked() }
    }

    pub fn take(self) -> UrlQueueElement<T> {
        self.take_impl()
    }

    pub fn drop_from_queue(self) {
        self.take_impl();
    }
}

impl<'a, T> Deref for UrlQueueElementRef<'a, T>
where
    T: 'static + Serialize + DeserializeOwned,
{
    type Target = UrlQueueElement<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { self.element.as_ref().unwrap_unchecked() }
    }
}

impl<'a, T> Drop for UrlQueueElementRef<'a, T>
where
    T: 'static + Serialize + DeserializeOwned,
{
    fn drop(&mut self) {
        if let Some(value) = self.element.take() {
            match self.backend.force_enqueue(value) {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Failed to return a value to the queue with error: {}", err)
                }
            }
        }
    }
}
