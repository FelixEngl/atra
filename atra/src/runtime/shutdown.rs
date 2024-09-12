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

use log::info;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;
use tokio::sync::broadcast::error::{RecvError, SendError};
use tokio::sync::{broadcast, mpsc};

// Inspired by https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs
// But we work wit an atomic to make is a little easier

/// A struct to help with satisfying the value for an object
#[derive(Debug, Copy, Clone, Error)]
pub struct ShutdownPhantom;
impl Display for ShutdownPhantom {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShutdownPhantom")
    }
}

#[allow(refining_impl_trait)]
impl ShutdownReceiver for ShutdownPhantom {
    #[inline]
    fn is_shutdown(&self) -> bool {
        false
    }

    fn weak_handle<'a>(&'a self) -> ShutdownHandle<'a, ShutdownPhantom> {
        ShutdownHandle { shutdown: &self }
    }
}

/// A simple trait for receiving a shutdown command
#[allow(refining_impl_trait)]
pub trait ShutdownReceiver: Clone {
    /// Returns `true` if the shutdown signal has been received.
    fn is_shutdown(&self) -> bool;

    /// Returns a weak handle to the receiver
    fn weak_handle<'a>(&'a self) -> ShutdownHandle<'a, impl ShutdownReceiver>;
}

/// A simple signal class for a shutdown sender
#[derive(Debug, Clone)]
pub struct ShutdownSignal;

#[derive(Debug, Clone)]
pub struct GracefulShutdownSignal;

/// Acts as a guard for a [GracefulShutdownBarrier]
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GracefulShutdownGuard {
    /// Not used but needed
    _sender: mpsc::Sender<GracefulShutdownSignal>,
}

impl GracefulShutdownGuard {
    pub fn to_unsafe(self) -> UnsafeShutdownGuard {
        UnsafeShutdownGuard::Guarded(self)
    }
}

/// Acts as a unsafe guard for a [GracefulShutdownBarrier]
#[derive(Debug, Clone)]
pub enum UnsafeShutdownGuard {
    Guarded(GracefulShutdownGuard),
    Unguarded,
}

/// Acts as a barrier for a [GracefulShutdownGuard]
#[repr(transparent)]
#[derive(Debug)]
pub struct GracefulShutdownBarrier {
    receiver: mpsc::Receiver<GracefulShutdownSignal>,
}

impl GracefulShutdownBarrier {
    pub async fn wait(&mut self) {
        // Output is never used and always None
        self.receiver.recv().await;
        info!("Shutting down!")
    }
}

/// Sends a shutdown [ShutdownSignal] to all [ShutdownSignalReceiver]
#[repr(transparent)]
#[derive(Debug)]
pub struct ShutdownSignalSender {
    sender: broadcast::Sender<ShutdownSignal>,
}

impl ShutdownSignalSender {
    /// Tries to notify all receivers.
    /// Returns an error if the shutdoen signal fails.
    #[allow(dead_code)]
    pub async fn notify(&self) -> Result<usize, SendError<ShutdownSignal>> {
        self.sender.send(ShutdownSignal)
    }
}

/// Receives the [ShutdownSignal].
#[repr(transparent)]
#[derive(Debug)]
pub struct ShutdownSignalReceiver {
    receiver: broadcast::Receiver<ShutdownSignal>,
}

impl ShutdownSignalReceiver {
    /// Waits for a shutdown signal or fails
    #[allow(dead_code)]
    pub async fn recv(&mut self) -> Result<ShutdownSignal, RecvError> {
        self.receiver.recv().await
    }
}

impl Clone for ShutdownSignalReceiver {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.resubscribe(),
        }
    }
}

/// Creates the tools for graceful shutdown handling
pub fn graceful_shutdown() -> (
    ShutdownSignalSender,
    GracefulShutdown,
    GracefulShutdownBarrier,
) {
    let (shutdown_sender, shutdown) = shutdown();

    let (sender, receiver) = mpsc::channel(1);

    (
        shutdown_sender,
        GracefulShutdown::new(shutdown, GracefulShutdownGuard { _sender: sender }),
        GracefulShutdownBarrier { receiver },
    )
}

/// Creates the simple tools for shutdown handling
pub fn shutdown() -> (ShutdownSignalSender, Shutdown) {
    let (sender, receiver) = broadcast::channel(1);
    (
        ShutdownSignalSender { sender },
        Shutdown::new(ShutdownSignalReceiver { receiver }),
    )
}

/// Basically a shutdown, but it holds a reference to a shutdown_complete_tx
#[derive(Debug, Clone)]
pub struct GracefulShutdown {
    /// The shutdown instance
    shutdown: Shutdown,
    /// Not used, but necessary for graceful shutdown
    guard: GracefulShutdownGuard,
}

impl GracefulShutdown {
    fn new(shutdown: Shutdown, guard: GracefulShutdownGuard) -> Self {
        Self { shutdown, guard }
    }

    delegate::delegate! {
        to self.shutdown {
            #[allow(dead_code)] pub async fn recv(&mut self);
        }
    }

    /// Downgrades the [GracefulShutdown] to a [Shutdown]
    #[allow(dead_code)]
    pub fn downgrade(self) -> Shutdown {
        self.shutdown
    }

    /// Returns a copy of the shutdown instance
    #[allow(dead_code)]
    pub fn new_shutdown_instance(&self) -> Shutdown {
        self.shutdown.clone()
    }

    /// Returns a copy of the guard
    #[allow(dead_code)]
    pub fn new_guard_instance(&self) -> GracefulShutdownGuard {
        self.guard.clone()
    }

    /// Returns the inner value
    #[allow(dead_code)]
    pub fn into_inner(self) -> (Shutdown, GracefulShutdownGuard) {
        (self.shutdown, self.guard)
    }

    #[allow(dead_code)]
    #[inline]
    fn subscribe(&self) -> ShutdownHandle<'_, Shutdown> {
        self.shutdown.subscribe()
    }
}

#[allow(refining_impl_trait)]
impl ShutdownReceiver for GracefulShutdown {
    delegate::delegate! {
        to self.shutdown {
            fn is_shutdown(&self) -> bool;
        }
    }

    fn weak_handle<'a>(&'a self) -> ShutdownHandle<'a, Shutdown> {
        self.shutdown.weak_handle()
    }
}

impl From<(Shutdown, GracefulShutdownGuard)> for GracefulShutdown {
    fn from(value: (Shutdown, GracefulShutdownGuard)) -> Self {
        Self::new(value.0, value.1)
    }
}

/// Listens for the server shutdown signal.
///
/// Shutdown is signalled using a `broadcast::Receiver`. Only a single value is
/// ever sent. Once a value has been sent via the broadcast channel, the server
/// should shutdown.
///
/// The `Shutdown` struct listens for the signal and tracks that the signal has
/// been received. Callers may query for whether the shutdown signal has been
/// received or not.
#[derive(Debug)]
pub struct Shutdown {
    /// `true` if the shutdown signal has been received
    is_shutdown: AtomicBool,

    /// The receive half of the channel used to listen for shutdown.
    notify: ShutdownSignalReceiver,
}

impl Shutdown {
    /// Create a new `Shutdown` backed by the given `broadcast::Receiver`.
    fn new(notify: ShutdownSignalReceiver) -> Shutdown {
        Shutdown {
            is_shutdown: AtomicBool::new(false),
            notify,
        }
    }

    /// Upgrades a [Shutdown] to a[GracefulShutdown]
    #[allow(dead_code)]
    pub fn upgrade(self, guard: GracefulShutdownGuard) -> GracefulShutdown {
        GracefulShutdown::new(self, guard)
    }

    /// Receive the shutdown notice, waiting if necessary.
    #[allow(dead_code)]
    pub async fn recv(&mut self) {
        // If the shutdown signal has already been received, then return
        // immediately.
        if self.is_shutdown() {
            return;
        }

        // Cannot receive a "lag error" as only one value is ever sent.
        let _ = self.notify.recv().await;

        // Remember that the signal has been received.
        self.is_shutdown.store(true, Ordering::Release);
    }

    /// Returns a weak shutdown handle
    #[allow(dead_code)]
    fn subscribe<'a>(&'a self) -> ShutdownHandle<'a, Shutdown> {
        ShutdownHandle { shutdown: self }
    }
}

#[allow(refining_impl_trait)]
impl ShutdownReceiver for Shutdown {
    /// Returns `true` if the shutdown signal has been received.
    fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Acquire)
    }

    fn weak_handle<'a>(&'a self) -> ShutdownHandle<'a, Shutdown> {
        ShutdownHandle { shutdown: self }
    }
}

impl From<ShutdownSignalReceiver> for Shutdown {
    fn from(value: ShutdownSignalReceiver) -> Self {
        Self::new(value)
    }
}

impl Clone for Shutdown {
    fn clone(&self) -> Self {
        let notify = self.notify.clone();
        // Here we have to make sure, that nothing changes before copying.
        let is_shutdown_after_resubscribe = self.is_shutdown.load(Ordering::SeqCst);

        Self {
            notify,
            is_shutdown: AtomicBool::new(is_shutdown_after_resubscribe),
        }
    }
}

/// A simple handle for a shutdown. Depends on some kind of shutdown
#[derive(Debug)]
pub struct ShutdownHandle<'a, T: ShutdownReceiver> {
    shutdown: &'a T,
}

unsafe impl<T: ShutdownReceiver> Sync for ShutdownHandle<'_, T> {}
unsafe impl<T: ShutdownReceiver> Send for ShutdownHandle<'_, T> {}

impl<T: ShutdownReceiver> Clone for ShutdownHandle<'_, T> {
    fn clone(&self) -> Self {
        Self {
            shutdown: self.shutdown,
        }
    }
}

#[allow(refining_impl_trait)]
impl<T: ShutdownReceiver> ShutdownReceiver for ShutdownHandle<'_, T> {
    delegate::delegate! {
        to self.shutdown {
            fn is_shutdown(&self) -> bool;
        }
    }

    fn weak_handle<'a>(&'a self) -> ShutdownHandle<'a, T> {
        ShutdownHandle {
            shutdown: self.shutdown,
        }
    }
}
