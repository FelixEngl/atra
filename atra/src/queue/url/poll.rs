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
use std::sync::Arc;
use mockall::Any;
use tokio::sync::watch::{channel, Receiver, Sender};


#[derive(Debug, Clone)]
pub struct PollWaiterFactory {
    send: Sender<PollUseState>,
    rec: Receiver<PollUseState>,
}

unsafe impl Send for PollWaiterFactory{}
unsafe impl Sync for PollWaiterFactory{}

impl PollWaiterFactory {
    pub fn new() -> Self {
        let (send, rec) = channel(PollUseState::ZERO);
        Self {
            send,
            rec,
        }
    }

    /// Creates a cheap ref that has drop logic.
    pub fn create(&self) -> PollWaiter {
        let new = PollWaiter::new(
            self.send.clone(),
            self.rec.clone(),
        );
        self.send.send_modify(|value| value.increase());
        new
    }

    #[cfg(test)]
    pub fn current_state(&self) -> usize {
        self.rec.borrow().current
    }
}


pub struct PollWaiter<'a> {
    inner: Arc<PollWaiterInner>,
    rec: Receiver<PollUseState>,
    _ll:PhantomData<&'a ()>
}

unsafe impl<'a> Send for PollWaiter<'a>{}
unsafe impl<'a> Sync for PollWaiter<'a>{}

impl<'a> PollWaiter<'a> {

    fn new(send: Sender<PollUseState>, rec: Receiver<PollUseState>) -> Self {
        Self {
            inner: Arc::new(PollWaiterInner::new(send)),
            rec,
            _ll: PhantomData
        }
    }

    pub fn has_changed(&self) -> bool {
        self.rec.has_changed().unwrap()
    }

    pub fn peek_other_waiter_count(&self) -> usize {
        self.rec.borrow().current
    }

    pub fn peek_has_other_waiters(&mut self) -> bool {
        self.peek_other_waiter_count() > 1
    }

    pub fn has_other_waiters(&mut self) -> bool {
        self.rec.borrow_and_update().current > 1
    }

    /// Returns the number ob other waiters.
    pub async fn wait_for_other_waiter_count_changed(&mut self) -> usize {
        if self.has_other_waiters() {
            self.rec.wait_for(|value| {
                value.changed_or_one()
            }).await.unwrap().current - 1
        } else {
            0
        }
    }

    pub async fn wait_for_has_other_waiters(&mut self) -> bool {
        self.wait_for_other_waiter_count_changed().await == 0
    }
}

impl<'a> Clone for PollWaiter<'a> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            rec: self.rec.clone(),
            _ll: PhantomData
        }
    }
}


/// The sender of the poll waiter. Only sends on drop.
/// It is basically responsible for executing the drop logic if
/// all instances are dropped.
#[clippy::has_significant_drop]
#[repr(transparent)]
struct PollWaiterInner {
    send: Sender<PollUseState>
}

impl PollWaiterInner {
    #[inline]
    fn new(send: Sender<PollUseState>) -> Self {
        Self {
            send,
        }
    }
}

impl Drop for PollWaiterInner {
    fn drop(&mut self) {
        self.send.send_if_modified(|value| {
            value.decrease();
            value.changed()
        });
    }
}

/// The state of the pollers
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(C)]
struct PollUseState {
    pub current: usize,
    pub last: usize
}

impl PollUseState {
    pub const ZERO: PollUseState = PollUseState::new(0);
    pub const fn new(current: usize) -> Self {
        Self { current, last: 0 }
    }

    #[inline]
    pub fn increase(&mut self) {
        self.last = self.current;
        self.current = self.current.saturating_add(1)
    }

    #[inline]
    pub fn decrease(&mut self) {
        self.last = self.current;
        self.current = self.current.saturating_sub(1)
    }

    #[inline]
    pub fn changed(&self) -> bool {
        self.current != self.last
    }

    #[inline]
    pub fn changed_or_one(&self) -> bool {
        self.current == 1 || self.current != self.last
    }
}

impl Default for PollUseState {
    fn default() -> Self {
        Self::ZERO
    }
}




#[cfg(test)]
mod test {
    use crate::queue::url::PollWaiterFactory;

    #[test]
    fn drop_check(){
        let origin = PollWaiterFactory::new();
        let r1 = origin.create();
        let r11 = r1.clone();
        assert_eq!(1, origin.current_state());
        let r2 = origin.create();
        assert_eq!(2, origin.current_state());
        drop(r11);
        assert_eq!(2, origin.current_state());
        drop(r1);
        assert_eq!(1, origin.current_state());
        drop(r2);
        assert_eq!(0, origin.current_state());
    }
}