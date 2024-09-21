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
