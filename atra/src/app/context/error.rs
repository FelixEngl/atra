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
use thiserror::Error;
use crate::app::consumer::GlobalError;
use crate::app::context::AtraContext;
use crate::contexts::traits::SupportsLinkState;
use crate::link_state::LinkStateManager;
use crate::queue::QueueError;



#[derive(Debug, Error)]
pub enum AtraRunContextError<C: AtraContext> {
    #[error(transparent)]
    LinkStateManagerError(<<C as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error),
    #[error(transparent)]
    UrlQueueError(#[from] QueueError),
    #[error(transparent)]
    CrawlTaskError(#[from] GlobalError),
}