use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Provides a serial
pub trait SerialProvider: Sync + Send {
    type Serial: Display;

    fn provide_serial(&self) -> Option<Self::Serial>;
}

#[derive(Debug, Copy, Clone)]
pub struct NoSerial<S=u8>{
    _phantom: PhantomData<S>
}

unsafe impl<S> Send for NoSerial<S>{}
unsafe impl<S> Sync for NoSerial<S>{}

impl<S> SerialProvider for NoSerial<S> where S: Display {
    type Serial = S;

    #[inline(always)]
    fn provide_serial(&self) -> Option<Self::Serial> {
        None
    }
}

impl<S> Default for NoSerial<S> {
    fn default() -> Self {
        Self{_phantom: PhantomData}
    }
}

#[derive(Debug, Clone, Default)]
pub struct StaticSerialProvider<S> {
    value: S
}

unsafe impl<S> Send for StaticSerialProvider<S>{}
unsafe impl<S> Sync for StaticSerialProvider<S>{}

impl<S> StaticSerialProvider<S> {
    pub const fn new(value: S) -> Self {
        Self { value }
    }
}

impl<S> SerialProvider for StaticSerialProvider<S> where S: Display + Clone {
    type Serial = S;

    fn provide_serial(&self) -> Option<Self::Serial> {
        Some(self.value.clone())
    }
}


#[derive(Debug, Clone, Default)]
pub struct DefaultSerialProvider {
    state: Arc<AtomicU32>,
}

impl DefaultSerialProvider {
    pub fn get_next_serial(&self) -> u32 {
        unsafe {
            self.state.fetch_update(
                Ordering::SeqCst,
                Ordering::Relaxed,
                |next| Some(next.overflowing_add(1).0)
            ).unwrap_unchecked()
        }
    }
}

impl SerialProvider for DefaultSerialProvider {
    type Serial = u32;
    fn provide_serial(&self) -> Option<Self::Serial> {
        Some(self.get_next_serial())
    }
}
