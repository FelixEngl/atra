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

#[cfg(test)]
pub use phantom::*;
pub use shutdown::*;

/// Sends a shutdown signal
pub trait ShutdownSender {
    /// Initialize the shutdown of Atra.
    fn shutdown(&self);
}

/// A simple trait for receiving a shutdown command
pub trait ShutdownReceiver: Clone {
    /// Returns `true` if the shutdown signal has been received.
    fn is_shutdown(&self) -> bool;

    /// Wait for the shutdown.
    async fn wait(&self);
}

mod shutdown {
    use crate::runtime::{ShutdownReceiver, ShutdownSender};
    use crate::sync::CancellationTokenProvider;
    use std::sync::Arc;
    use tokio_util::sync::{CancellationToken, DropGuard};

    /// A root shutdown element. Does not provide any significant
    /// functionality to the outside world.
    #[derive(Debug)]
    #[repr(transparent)]
    pub struct ShutdownRoot {
        inner: CancellationToken,
    }

    impl ShutdownRoot {
        fn new() -> Self {
            Self {
                inner: CancellationToken::new(),
            }
        }

        /// Creates a new child shutdown.
        fn create_child(&self) -> ShutdownChild {
            ShutdownChild {
                inner: self.inner.child_token(),
            }
        }

        /// Creates a drob guard for the inner token.
        /// If this guard is dropped, the root token is canceled.
        fn drop_guard(&self) -> DropGuard {
            self.inner.clone().drop_guard()
        }

        /// Returns true if the token is cancelled.
        pub fn is_shutdown(&self) -> bool {
            self.inner.is_cancelled()
        }

        /// Waits until the token is canceled.
        pub async fn wait(&self) {
            self.inner.clone().cancelled_owned().await
        }

        /// Shuts down the root.
        pub unsafe fn shutdown(&self) {
            self.inner.cancel();
        }

        /// The only way to duplicate a root shutdown.
        /// This is only used internally for cloning a graceful shutdown.
        fn explicit_clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }

    impl CancellationTokenProvider for ShutdownRoot {
        fn clone_token(&self) -> CancellationToken {
            self.inner.clone()
        }

        fn child_token(&self) -> CancellationToken {
            self.inner.child_token()
        }
    }

    /// A graceful shutdown shuts down all children but waits until the
    /// root shutdown id canceled.
    ///
    /// As precaution is does not implement any kind of interface to make sure,
    /// that it is not used for shutdown behaviour by mistake.
    #[derive(Debug)]
    pub struct GracefulShutdown {
        root: ShutdownRoot,
        child: ShutdownChild,
    }
    impl GracefulShutdown {
        pub fn new() -> Self {
            let root = ShutdownRoot::new();
            let child = root.create_child();
            Self { root, child }
        }

        #[inline]
        pub fn with_guard(self) -> GracefulShutdownWithGuard {
            GracefulShutdownWithGuard::wrap(self)
        }

        /// Returns a reference to the root shutdown.
        #[inline]
        pub fn root(&self) -> &ShutdownRoot {
            &self.root
        }

        /// Returns a reference to the child shutdown
        #[inline]
        pub fn child(&self) -> &ShutdownChild {
            &self.child
        }

        /// Returns true if the child is canceled.
        #[inline(always)]
        pub fn is_shutdown(&self) -> bool {
            self.child.is_shutdown()
        }

        /// Waits until the root is canceled.
        #[inline(always)]
        pub async fn wait(&self) {
            self.root.wait().await
        }

        #[inline(always)]
        pub fn shutdown(&self) {
            self.child.shutdown();
        }
    }
    impl Clone for GracefulShutdown {
        fn clone(&self) -> Self {
            Self {
                root: self.root.explicit_clone(),
                child: self.child.clone(),
            }
        }
    }

    /// A graceful shutdown bot doesn't implement any interfaces because it
    /// can not be shared by normal means.
    #[derive(Debug)]
    pub struct GracefulShutdownWithGuard {
        inner: GracefulShutdown,
        guard: GracefulShutdownGuard,
    }
    impl GracefulShutdownWithGuard {
        pub fn new() -> Self {
            let new = GracefulShutdown::new();
            Self::wrap(new)
        }

        #[inline]
        pub fn wrap(inner: GracefulShutdown) -> Self {
            let guard = GracefulShutdownGuard::new(inner.root.drop_guard());
            Self { inner, guard }
        }

        /// Returns the underlying shutdown
        #[inline(always)]
        pub fn get(&self) -> &GracefulShutdown {
            &self.inner
        }

        /// Returns a copy from the guard.
        ///
        /// CAREFUL:
        /// The GracefulShutdown is never canceled by dropping,
        /// if there is anywhere a guard that is never dropped.
        #[inline(always)]
        pub fn guard(&self) -> GracefulShutdownGuard {
            self.guard.clone()
        }

        /// Drops the guard and returns the inner shutdown.
        pub fn consume(self) -> GracefulShutdown {
            self.inner
        }
    }
    impl Clone for GracefulShutdownWithGuard {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                guard: self.guard.clone(),
            }
        }
    }

    #[derive(Debug, Clone)]
    #[repr(transparent)]
    #[clippy::has_significant_drop]
    pub struct GracefulShutdownGuard {
        _inner: Arc<DropGuard>,
    }
    impl GracefulShutdownGuard {
        pub fn new(inner: DropGuard) -> Self {
            Self {
                _inner: Arc::new(inner),
            }
        }
    }

    /// A normal shutdown that is canceled when the associated [`ShutdownRoot`] is
    /// canceled.
    #[derive(Debug)]
    #[repr(transparent)]
    pub struct ShutdownChild {
        inner: CancellationToken,
    }

    impl CancellationTokenProvider for ShutdownChild {
        fn clone_token(&self) -> CancellationToken {
            self.inner.clone()
        }

        fn child_token(&self) -> CancellationToken {
            self.inner.child_token()
        }
    }

    impl ShutdownSender for ShutdownChild {
        fn shutdown(&self) {
            self.inner.cancel();
        }
    }
    impl ShutdownReceiver for ShutdownChild {
        fn is_shutdown(&self) -> bool {
            self.inner.is_cancelled()
        }

        async fn wait(&self) {
            self.inner.clone().cancelled_owned().await
        }
    }
    impl Clone for ShutdownChild {
        #[inline]
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
}

// Inspired by https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs
// But we work wit an atomic to make is a little easier

#[cfg(test)]
mod phantom {
    use crate::runtime::ShutdownReceiver;
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

        async fn wait(&self) {
            if ENDLESS {
                tokio::sync::Notify::const_new().notified().await
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::runtime::{
        AtraHandleOption, AtraRuntime, GracefulShutdown, GracefulShutdownWithGuard,
        OptionalAtraHandle, RuntimeContext, ShutdownReceiver,
    };
    use rand::Rng;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::LazyLock;
    use std::time::Duration;
    use tokio::task::JoinSet;
    use tokio::time::sleep;

    struct OnDropProtected;

    static COUNTER: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

    impl OnDropProtected {
        pub fn new(shutdown_and_handle: &RuntimeContext) -> Self {
            if let Ok(handle) = shutdown_and_handle.handle().try_io_or_main_or_current() {
                let guard = shutdown_and_handle.shutdown_guard().guard();
                handle.spawn(async move {
                    let _guard = guard;
                    println!("OnDropProtected: Start Waiting");
                    sleep(Duration::from_millis(6_000)).await;
                    println!("OnDropProtected: Finished Work");
                    COUNTER.fetch_add(1, Ordering::SeqCst);
                });
            };

            Self
        }
    }

    async fn sidequest<S: ShutdownReceiver>(shutdown: S, i: i32) -> (i32, usize) {
        let mut ct = 0;
        while !shutdown.is_shutdown() {
            sleep(Duration::from_millis(10)).await;
            ct += i as usize;
        }
        let wait_time = { rand::thread_rng().gen_range(500..1500) };
        sleep(Duration::from_millis(wait_time)).await;
        (i, ct)
    }

    struct Application {
        shutdown: GracefulShutdownWithGuard,
        handle: OptionalAtraHandle,
    }

    impl Application {
        pub fn new() -> (Self, AtraRuntime) {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Fatal: Was not able to initialize runtime!");

            let runtime = AtraRuntime::new(runtime, None);
            (
                Self {
                    shutdown: GracefulShutdownWithGuard::new(),
                    handle: runtime.handle().as_optional(),
                },
                runtime,
            )
        }

        pub fn shutdown(&self) -> &GracefulShutdown {
            self.shutdown.get()
        }

        pub async fn run(&mut self) -> Vec<(i32, usize)> {
            let shutdown_and_handle =
                RuntimeContext::new(self.shutdown.clone(), self.handle.clone());

            let _worker = OnDropProtected::new(&shutdown_and_handle);
            drop(shutdown_and_handle);

            let mut threads = JoinSet::new();

            for i in 0..8 {
                let shutdown = self.shutdown.clone();
                threads.spawn(async move {
                    let shutdown = shutdown;
                    sidequest(shutdown.get().child().clone(), i).await
                });
            }

            threads.join_all().await
        }
    }

    #[test]
    fn shutdown_works_as_expected() {
        let (mut app, runtime) = Application::new();
        let shutdown = app.shutdown().clone();

        runtime.block_on(async move {
            let result = {
                let future = app.run();
                tokio::pin!(future);

                let signal = sleep(Duration::from_millis(3_000));

                let mut shutdown_result: Option<Vec<(i32, usize)>> = None;

                tokio::select! {
                    res = &mut future => {
                        shutdown_result.replace(res);
                        println!("Future finished before signal.")
                    }
                    _ = signal => {
                        println!("Signal for shutdown!");
                        shutdown.shutdown();
                    }
                }

                if let Some(result) = shutdown_result {
                    result
                } else {
                    println!("Wait for future!");
                    future.await
                }
            };

            for value in result {
                println!("{:?}", value);
            }

            println!("Drop App");

            drop(app);
            println!("Wait for shutdown!");
            println!("CT Before: {}", COUNTER.load(Ordering::Relaxed));
            shutdown.wait().await;
            println!("Finished!");
            println!("CT After: {}", COUNTER.load(Ordering::SeqCst));
        })
    }
}
