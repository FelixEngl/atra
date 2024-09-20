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

use std::sync::Arc;
use tokio_util::sync::{CancellationToken, DropGuard};

#[derive(Debug)]
pub struct GracefulShutdown {
    shutdown: Shutdown,
    guard: Arc<DropGuard>
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let shutdown = Shutdown::new();
        let drop = shutdown.notify.clone().drop_guard();
        Self {
            shutdown,
            guard: Arc::new(drop)
        }
    }

    pub fn create_shutdown(&self) -> Shutdown {
        self.shutdown.clone()
    }
}

impl Clone for GracefulShutdown {
    fn clone(&self) -> Self {
        Self {
            shutdown: self.shutdown.clone(),
            guard: self.guard.clone()
        }
    }
}

impl ShutdownReceiver for GracefulShutdown {
    fn is_shutdown(&self) -> bool {
        self.shutdown.is_shutdown()
    }
}

impl ShutdownReceiverWithWait for GracefulShutdown {
    async fn wait(&self) {
        self.shutdown.wait().await
    }
}


#[derive(Debug)]
pub struct Shutdown {
    notify: CancellationToken
}

impl Shutdown {

    pub fn shutdown(&self) {
        self.notify.cancel();
    }

    pub fn new() -> Self {
        Self { notify: CancellationToken::new() }
    }
}

impl Clone for Shutdown {
    fn clone(&self) -> Self {
        Self {
            notify: self.notify.clone()
        }
    }
}

impl ShutdownReceiver for Shutdown {
    fn is_shutdown(&self) -> bool {
        self.notify.is_cancelled()
    }
}

impl ShutdownReceiverWithWait for Shutdown {
    async fn wait(&self) {
        self.notify.clone().cancelled_owned().await
    }
}


// Inspired by https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs
// But we work wit an atomic to make is a little easier

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
        async fn wait(&self) {
            if ENDLESS {
                tokio::sync::Notify::const_new().notified().await
            }
        }
    }
}

#[cfg(test)]
pub use phantom::*;

/// A simple trait for receiving a shutdown command
pub trait ShutdownReceiver: Clone {
    /// Returns `true` if the shutdown signal has been received.
    fn is_shutdown(&self) -> bool;
}

pub trait ShutdownReceiverWithWait: ShutdownReceiver {
    async fn wait(&self);
}
