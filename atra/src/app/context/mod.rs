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

mod error;
mod traits;

pub use traits::*;

pub use error::*;

use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use thiserror::Error;
use time::OffsetDateTime;
use crate::app::consumer::{GlobalError, GlobalErrorConsumer};
use crate::config::Config;
use crate::contexts::BaseContext;
use crate::contexts::local::{LocalContext, LocalContextInitError};
use crate::contexts::traits::{SupportsLinkSeeding, SupportsLinkState, SupportsMetaInfo, SupportsUrlQueue};
use crate::contexts::worker::WorkerContext;
use crate::crawl::{crawl, ErrorConsumer, ExitState};
use crate::extraction::ExtractedLink;
use crate::io::errors::ErrorWithPath;
use crate::link_state::LinkStateManager;
use crate::queue::QueueError;
use crate::runtime::{GracefulShutdown, RuntimeContext, ShutdownHandle, ShutdownReceiverWithWait};
use crate::seed::BasicSeed;
use crate::sync::barrier::WorkerBarrier;
use crate::url::UrlWithDepth;



pub struct AtraRunContextProvider;

impl AtraContextProvider for AtraRunContextProvider {
    type AtraContext = AtraRunContext;
    type Error = LocalContextInitError;

    async fn create(configs: Config, runtime_context: &RuntimeContext) -> Result<Self::AtraContext, Self::Error> {
        Ok(
            AtraRunContext::new(
                Arc::new(
                    LocalContext::new(
                        configs,
                        runtime_context
                    )?
                )
            )
        )
    }
}


#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct AtraRunContext {
    inner: Arc<LocalContext>
}

unsafe impl Send for AtraRunContext{}
unsafe impl Sync for AtraRunContext{}

impl AtraRunContext {
    pub fn new(inner: Arc<LocalContext>) -> Self {
        Self { inner }
    }

    pub fn local_context(&self) -> &LocalContext {
        self.inner.as_ref()
    }

    pub fn local_context_arc(&self) -> Arc<LocalContext> {
        self.inner.clone()
    }

    pub fn into_inner(self) -> Arc<LocalContext> {
        self.inner
    }
}

impl BaseContext for AtraRunContext {}

impl SupportsUrlQueue for AtraRunContext {
    type UrlQueue = <LocalContext as SupportsUrlQueue>::UrlQueue;

    delegate::delegate! {
        to self.inner {
            async fn can_poll(&self) -> bool;
            fn url_queue(&self) -> &Self::UrlQueue;
        }
    }
}

impl SupportsMetaInfo for AtraRunContext {
    delegate::delegate! {
        to self.inner {
            fn crawl_started_at(&self) -> OffsetDateTime;
            fn discovered_websites(&self) -> usize;
        }
    }
}

impl SupportsLinkState for AtraRunContext {
    type LinkStateManager = <LocalContext as SupportsLinkState>::LinkStateManager;

    delegate::delegate! {
        to self.inner {
            fn get_link_state_manager(&self) -> &Self::LinkStateManager;
        }
    }
}

impl AtraContext for AtraRunContext {
    type Error = AtraRunContextError<Self>;

    async fn run_crawl_task<S>(
        &self,
        worker_id: usize,
        recrawl_ct: usize,
        barrier: Arc<WorkerBarrier>,
        shutdown_handle: S
    ) -> Result<ExitState, Self::Error> where S: ShutdownReceiverWithWait {
        match WorkerContext::create(worker_id, recrawl_ct, self.inner.clone()) {
            Ok(value) => {
                Ok(
                    crawl(
                        value,
                        shutdown_handle.clone(),
                        barrier,
                        GlobalErrorConsumer::new(),
                    ).await.map_err(Self::Error::CrawlTaskError)?
                )
            }
            Err(value) => {
                Err(Self::Error::CrawlTaskError(value.into()))
            }
        }

    }
}


impl From<LocalContext> for AtraRunContext {
    fn from(value: LocalContext) -> Self {
        Self::new(Arc::new(value))
    }
}
