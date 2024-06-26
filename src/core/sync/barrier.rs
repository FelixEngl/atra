use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::select;
use tokio_util::sync::CancellationToken;
use crate::core::contexts::{Context, SlimCrawlTaskContext};
use crate::core::contexts::worker_context::WorkerContext;
use crate::core::url::queue::UrlQueue;

/// The result of the [WorkerBarrier]
#[derive(Debug)]
pub enum ContinueOrStop<T> {
    Continue(T),
    Cancelled(T)
}

/// The command send if it can continue
#[derive(Debug, Copy, Clone)]
struct ContinueCommand;


/// A barrier to help with the synchronisation of the workers.
/// Allows to recover if the workload changes.
pub struct WorkerBarrier {
    number_of_workers: NonZeroUsize,
    cancel_requester_count_plus_one: AtomicUsize,
    cancellation_token: CancellationToken,
}

impl WorkerBarrier {

    pub fn new(number_of_worker: NonZeroUsize) -> Self {
        Self {
            number_of_workers: number_of_worker,
            // Start one greater than 0, this way we can make sure that increment counter returns true if all decide to quit.
            cancel_requester_count_plus_one: AtomicUsize::new(1),
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Check if it was cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Trigger the cancellation manually
    #[allow(dead_code)]
    pub fn trigger_cancellation(&self) {
        self.cancellation_token.cancel()
    }

    pub async fn wait_for_is_cancelled<C: SlimCrawlTaskContext, T>(&self, context: &WorkerContext<C>, cause: T) -> ContinueOrStop<T> {
        self.wait_for_is_cancelled_with(context, || cause).await
    }

    /// Waits for a specific context until either all decide to stop orthe queue has some kind of change
    pub async fn wait_for_is_cancelled_with<C: SlimCrawlTaskContext, T, F: FnOnce() -> T>(&self, context: &WorkerContext<C>, cause_provider: F) -> ContinueOrStop<T> {
        if self.cancellation_token.is_cancelled() {
            return ContinueOrStop::Cancelled(cause_provider())
        }
        let mut queue_changed_subscription = context.url_queue().subscribe_to_change();
        log::info!("Worker {} starts waiting for stop or queue event.", context.worker_id());
        let count = self.cancel_requester_count_plus_one.fetch_add(1, Ordering::SeqCst);
        assert_ne!(0, count, "Worker {} encountered an illegal state with the barrier!", context.worker_id());
        if count == self.number_of_workers.get() {
            self.cancellation_token.cancel();
        }
        select! {
            _ = queue_changed_subscription.recv() => {
                let state = self.cancel_requester_count_plus_one.fetch_sub(1, Ordering::SeqCst);
                assert_ne!(0, state, "Worker {} encountered an illegal state with the barrier!", context.worker_id());
                if self.cancellation_token.is_cancelled() {
                    log::error!("Worker {} was cancelled but queue changed!", context.worker_id());
                    ContinueOrStop::Cancelled(cause_provider())
                } else{
                    log::debug!("Worker {} can continue!", context.worker_id());
                    ContinueOrStop::Continue(cause_provider())
                }
            }
            _ = self.cancellation_token.cancelled() => {
                log::debug!("Worker {} stopping!.", context.worker_id());
                ContinueOrStop::Cancelled(cause_provider())
            }
        }
    }
}
