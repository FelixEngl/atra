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

use crate::runtime::{GracefulShutdown, OptionalAtraHandle};

/// A context holding informations about the runtime
#[derive(Debug, Clone)]
pub struct RuntimeContext {
    shutdown: GracefulShutdown,
    handle: OptionalAtraHandle,
}

impl RuntimeContext {
    pub fn new(shutdown: GracefulShutdown, handle: OptionalAtraHandle) -> Self {
        Self {
            shutdown,
            handle,
        }
    }

    /// Creates an unbound
    #[cfg(test)]
    pub fn unbound() -> Self {
        Self::new(GracefulShutdown::new(), OptionalAtraHandle::None)
    }

    pub fn shutdown_guard(&self) -> &GracefulShutdown {
        &self.shutdown
    }

    pub fn handle(&self) -> &OptionalAtraHandle {
        &self.handle
    }
}

impl AsRef<GracefulShutdown> for RuntimeContext {
    fn as_ref(&self) -> &GracefulShutdown {
        &self.shutdown
    }
}

impl AsRef<OptionalAtraHandle> for RuntimeContext {
    fn as_ref(&self) -> &OptionalAtraHandle {
        &self.handle
    }
}
