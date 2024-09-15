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

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::watch::{channel, Receiver, Sender};

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
struct PollerEvent {
    pub poller_left: usize
}

impl PollerEvent {
    pub const ZERO: PollerEvent = PollerEvent::new(0);
    pub const fn new(left: usize) -> Self {
        Self { poller_left: left }
    }
}

impl Default for PollerEvent {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone)]
pub struct PollWaiter {
    send: Sender<PollerEvent>,
    rec: Receiver<PollerEvent>,
}

unsafe impl Send for PollWaiter{}
unsafe impl Sync for PollWaiter{}

impl PollWaiter {
    pub fn new() -> Self {
        let (send, rec) = channel(PollerEvent::ZERO);
        Self {
            send,
            rec,
        }
    }

    pub fn has_changed(&self) -> bool {
        self.rec.has_changed().unwrap()
    }

    pub fn has_other_waiters(&mut self) -> bool {
        self.rec.borrow_and_update().poller_left > 1
    }

    pub async fn wait_for_has_other_waiters(&mut self) -> bool {
        self.rec.wait_for(|_| true).await.unwrap().poller_left > 1
    }

    #[cfg(test)]
    pub fn get_waiter_count(&self) -> usize {
        self.rec.borrow().poller_left
    }


    /// Creates a cheap ref that has drop logic.
    pub fn create_ref(&self) -> PollWaiterRef {
        let new = Self {
            rec: self.rec.clone(),
            send: self.send.clone()
        };
        self.send.send_modify(|value| { value.poller_left += 1 });
        PollWaiterRef::new(new)
    }

    fn drop_logic(&mut self) {
        self.send.send_if_modified(|value| {
            let old = value.poller_left;
            value.poller_left = value.poller_left.saturating_sub(1);
            old != value.poller_left
        });
    }
}





#[derive(Debug)]
#[clippy::has_significant_drop]
pub struct PollWaiterRef<'a> {
    inner: PollWaiter,
    borrow_count: Arc<AtomicUsize>,
    _ll:PhantomData<&'a ()>
}

unsafe impl<'a> Send for PollWaiterRef<'a>{}
unsafe impl<'a> Sync for PollWaiterRef<'a>{}

impl<'a> PollWaiterRef<'a> {
    fn new(inner: PollWaiter) -> Self {
        Self {
            inner,
            borrow_count: Arc::new(AtomicUsize::new(1)),
            _ll: PhantomData
        }
    }
}

impl<'a> Deref for PollWaiterRef<'a> {
    type Target = PollWaiter;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for PollWaiterRef<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a> Clone for PollWaiterRef<'a> {
    fn clone(&self) -> Self {
        self.borrow_count.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: self.inner.clone(),
            borrow_count: self.borrow_count.clone(),
            _ll: PhantomData
        }
    }
}

impl<'a> Drop for PollWaiterRef<'a> {
    fn drop(&mut self) {
        if self.borrow_count.fetch_sub(1, Ordering::Release) == 1 {
            self.inner.drop_logic()
        }
    }
}

#[cfg(test)]
mod test {
    use crate::queue::PollWaiter;

    #[test]
    fn drop_check(){
        let origin = PollWaiter::new();
        let r1 = origin.create_ref();
        let r11 = r1.clone();
        assert_eq!(1, origin.get_waiter_count());
        let r2 = origin.create_ref();
        assert_eq!(2, origin.get_waiter_count());
        drop(r11);
        assert_eq!(2, origin.get_waiter_count());
        drop(r1);
        assert_eq!(1, origin.get_waiter_count());
        drop(r2);
        assert_eq!(0, origin.get_waiter_count());
    }
}