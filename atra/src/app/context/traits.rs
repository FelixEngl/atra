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

use std::error::Error;
use std::sync::Arc;
use crate::config::Config;
use crate::contexts::traits::{SupportsLinkState, SupportsMetaInfo, SupportsUrlQueue};
use crate::crawl::ExitState;
use crate::link_state::LinkStateManager;
use crate::queue::QueueError;
use crate::runtime::{RuntimeContext, ShutdownReceiverWithWait};
use crate::sync::barrier::WorkerBarrier;


pub trait AtraContextProvider {
    type AtraContext: Send + Sync + AtraContext + 'static;
    type Error: Error + Send + Sync + 'static;

    async fn create(configs: Config, runtime_context: &RuntimeContext) -> Result<Self::AtraContext, Self::Error>;
}

pub trait AtraContext: 'static + Send + Sync + SupportsMetaInfo + SupportsUrlQueue + SupportsLinkState + Sized {
    type Error: Error + Send + Sync + 'static;

    fn run_crawl_task<S>(
        &self,
        worker_id: usize,
        recrawl_ct: usize,
        barrier: Arc<WorkerBarrier>,
        shutdown_handle: S
    ) -> impl std::future::Future<Output=Result<ExitState, Self::Error>> + Send where S: ShutdownReceiverWithWait;
}