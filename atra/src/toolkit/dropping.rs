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

use tokio::sync::watch::{channel, Receiver, Sender};

/// The sender of the poll waiter. Only sends on drop.
/// It is basically responsible for executing the drop logic if
/// all instances are dropped.
#[clippy::has_significant_drop]
#[repr(transparent)]
pub struct DropNotifyer<T>
where
    T: DropNotifyerEvent,
{
    sender: Sender<T>,
}

impl<T> DropNotifyer<T>
where
    T: DropNotifyerEvent + Default,
{
    #[inline]
    pub fn create() -> (Self, Receiver<T>) {
        Self::create_with(T::default())
    }
}

impl<T> DropNotifyer<T>
where
    T: DropNotifyerEvent,
{
    pub fn create_with(init: T) -> (Self, Receiver<T>) {
        let (sender, receiver) = channel(init);
        (Self::new(sender), receiver)
    }

    pub fn new(value: Sender<T>) -> Self {
        Self { sender: value }
    }
}

impl<T> From<Sender<T>> for DropNotifyer<T>
where
    T: DropNotifyerEvent,
{
    #[inline]
    fn from(value: Sender<T>) -> Self {
        Self::new(value)
    }
}

impl<T> Drop for DropNotifyer<T>
where
    T: DropNotifyerEvent,
{
    fn drop(&mut self) {
        self.sender.send_if_modified(|value| value.on_drop());
    }
}

pub trait DropNotifyerEvent {
    /// Returns true we have to notify the other listeners.
    fn on_drop(&mut self) -> bool;
}
