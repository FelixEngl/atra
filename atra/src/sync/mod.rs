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

mod barrier;

pub use barrier::{ContinueOrStop, WorkerBarrier};
use tokio_util::sync::CancellationToken;

/// A provider for cancellation tokens.
pub trait CancellationTokenProvider {
    /// Provides a clone of the owned token
    fn clone_token(&self) -> CancellationToken;

    /// Provides a child of the owned token
    fn child_token(&self) -> CancellationToken;
}

impl CancellationTokenProvider for CancellationToken {
    #[inline(always)]
    fn clone_token(&self) -> CancellationToken {
        self.clone()
    }

    #[inline(always)]
    fn child_token(&self) -> CancellationToken {
        CancellationToken::clone_token(self)
    }
}
