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

#![allow(dead_code)]

use log::info;
use std::sync::Arc;
use tokio_util::sync::{CancellationToken, DropGuard};

#[cfg(test)]
mod phantom {
    use crate::runtime::{ShutdownReceiver, ShutdownReceiverWithWait};
    use std::fmt::{Display, Formatter};
    use thiserror::Error;

    /// A struct to help with satisfying the value for an object
    #[derive(Debug, Copy, Clone, Error)]
    pub struct ShutdownPhantom<const ENDLESS: bool = true>;
    impl<const ENDLESS: bool> Display for ShutdownPhantom<ENDLESS> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "ShutdownPhantom")
        }
    }

    #[allow(refining_impl_trait)]
    impl<const ENDLESS: bool> ShutdownReceiver for ShutdownPhantom<ENDLESS> {
        #[inline]
        fn is_shutdown(&self) -> bool {
            false
        }
    }

    impl<const ENDLESS: bool> ShutdownReceiverWithWait for ShutdownPhantom<ENDLESS> {
        async fn wait(&mut self) {
            if ENDLESS {
                tokio::sync::Notify::const_new().notified().await
            }
        }
    }
}

#[cfg(test)]
pub use phantom::*;




/// A simple trait for receiving a shutdown command
#[allow(refining_impl_trait)]
pub trait ShutdownReceiver: Clone {
    /// Returns `true` if the shutdown signal has been received.
    fn is_shutdown(&self) -> bool;
}

pub trait ShutdownReceiverWithWait: ShutdownReceiver {
    async fn wait(&mut self);
}



#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Shutdown {
    token: CancellationToken
}

impl Shutdown {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new()
        }
    }

    fn create_sender(&self) -> ShutdownSignalSender {
        ShutdownSignalSender {
            token: self.token.clone()
        }
    }
}

impl ShutdownReceiver for Shutdown {
    fn is_shutdown(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl ShutdownReceiverWithWait for Shutdown {
    async fn wait(&mut self) {
        self.token.cancelled().await
    }
}



#[derive(Debug, Clone)]
pub struct GracefulShutdown {
    shutdown: Shutdown,
    drop_guard: Arc<DropGuard>
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let shutdown = Shutdown::new();
        let drop_guard = shutdown.token.clone().drop_guard();
        Self {
            shutdown,
            drop_guard: Arc::new(drop_guard)
        }
    }

    pub fn barrier(&self) -> GracefulShutdownBarrier {
        GracefulShutdownBarrier {
            token: self.shutdown.token.child_token()
        }
    }

    pub fn guard(&self) -> GracefulShutdownGuard {
        GracefulShutdownGuard {
            _guard: self.drop_guard.clone()
        }
    }
}

impl ShutdownReceiver for GracefulShutdown {
    fn is_shutdown(&self) -> bool {
        self.is_shutdown()
    }
}

impl ShutdownReceiverWithWait for GracefulShutdown {
    async fn wait(&mut self) {
        self.shutdown.wait().await
    }
}

pub struct GracefulShutdownBarrier {
    token: CancellationToken
}

impl GracefulShutdownBarrier {
    pub async fn wait(&self) {
        // Output is never used and always None
        self.token.cancelled().await;
        info!("Shutting down!")
    }
}

/// Sends a shutdown [ShutdownSignal] to all [ShutdownSignalReceiver]
#[repr(transparent)]
#[derive(Debug)]
pub struct ShutdownSignalSender {
    token: CancellationToken,
}

impl ShutdownSignalSender {
    /// Tries to notify all receivers.
    /// Returns an error if the shutdown signal fails.
    pub async fn cancel(&self) {
        self.token.cancel()
    }
}


#[derive(Clone, Debug)]
pub struct GracefulShutdownGuard {
    _guard: Arc<DropGuard>
}


impl GracefulShutdownGuard {
    pub fn unbounded() -> Self {
        GracefulShutdownGuard {
            _guard: Arc::new(CancellationToken::new().drop_guard())
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
