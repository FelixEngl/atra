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

use crate::contexts::traits::{SupportsUrlGuarding, SupportsUrlQueue, SupportsWorkerId};
use crate::queue::UrlQueue;
use crate::url::guard::UrlGuardian;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::select;
use tokio_util::sync::CancellationToken;
use crate::runtime::ShutdownChild;
use crate::sync::CancellationTokenProvider;

/// The result of the [WorkerBarrier]
#[derive(Debug)]
pub enum ContinueOrStop<T, C = T> {
    Continue(T),
    Cancelled(C),
}

/// A barrier to help with the synchronisation of the workers.
/// Allows to recover if the workload changes.
pub struct WorkerBarrier {
    number_of_workers: NonZeroUsize,
    cancel_requester_count_plus_one: AtomicUsize,
    cancellation_token: CancellationToken,
}

impl WorkerBarrier {
    pub fn new(number_of_workers: NonZeroUsize, cancellation_token: CancellationToken) -> Self {
        Self {
            number_of_workers,
            // Start one greater than 0, this way we can make sure that increment counter returns true if all decide to quit.
            cancel_requester_count_plus_one: AtomicUsize::new(1),
            cancellation_token,
        }
    }

    pub fn new_with_dependence_to<C: CancellationTokenProvider>(
        number_of_workers: NonZeroUsize,
        token_provider: &C
    ) -> Self {
        Self::new(
            number_of_workers,
            token_provider.child_token()
        )
    }

    /// Check if it was cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Trigger the cancellation manually
    pub fn trigger_cancellation(&self) {
        self.cancellation_token.cancel()
    }

    fn subscription_triggered<C, T, F>(
        &self,
        context: &C,
        cause_provider: F,
        target_name: &str,
    ) -> ContinueOrStop<T>
    where
        C: SupportsWorkerId,
        F: FnOnce() -> T,
    {
        let state = self
            .cancel_requester_count_plus_one
            .fetch_sub(1, Ordering::SeqCst);
        assert_ne!(
            0,
            state,
            "Worker {} encountered an illegal state with the barrier!",
            context.worker_id()
        );
        if self.cancellation_token.is_cancelled() {
            log::error!(
                "Worker {} was cancelled but {target_name} changed!",
                context.worker_id()
            );
            ContinueOrStop::Cancelled(cause_provider())
        } else {
            log::info!(
                "Worker {} can continue because {target_name} changed!",
                context.worker_id()
            );
            ContinueOrStop::Continue(cause_provider())
        }
    }

    pub async fn wait_for_is_cancelled<C, T>(&self, context: &C, cause: T) -> ContinueOrStop<T>
    where
        C: SupportsWorkerId + SupportsUrlQueue + SupportsUrlGuarding,
    {
        self.wait_for_is_cancelled_with(context, || cause).await
    }

    /// Waits for a specific context until either all decide to stop orthe queue has some kind of change
    pub async fn wait_for_is_cancelled_with<C, T, F>(
        &self,
        context: &C,
        cause_provider: F,
    ) -> ContinueOrStop<T>
    where
        C: SupportsWorkerId + SupportsUrlQueue + SupportsUrlGuarding,
        F: FnOnce() -> T,
    {
        if self.cancellation_token.is_cancelled() {
            return ContinueOrStop::Cancelled(cause_provider());
        }
        let mut queue_changed_subscription = context.url_queue().subscribe_to_change();
        let mut guardian_changed_subscription = context.get_guardian().subscribe();
        log::info!(
            "Worker {} starts waiting for stop or queue event.",
            context.worker_id()
        );
        let count = self
            .cancel_requester_count_plus_one
            .fetch_add(1, Ordering::SeqCst);
        assert_ne!(
            0,
            count,
            "Worker {} encountered an illegal state with the barrier!",
            context.worker_id()
        );
        if count == self.number_of_workers.get() {
            log::debug!("Worker {} Send cancellation!", context.worker_id());
            self.cancellation_token.cancel();
        } else {
            log::debug!(
                "Worker {} Wait for cancellation! ({count}|{})",
                context.worker_id(),
                self.number_of_workers.get()
            );
        }

        select! {
            _ = self.cancellation_token.cancelled() => {
                log::info!("Worker {} stopping!.", context.worker_id());
                ContinueOrStop::Cancelled(cause_provider())
            }
            _ = queue_changed_subscription.changed() => {
                self.subscription_triggered(context, cause_provider, "queue")
            }
            _ = guardian_changed_subscription.changed() => {
                self.subscription_triggered(context, cause_provider, "guardian")
            }
        }


    }
}
