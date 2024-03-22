//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::ops::Deref;
use thiserror::Error;
use tokio::runtime::{Handle, Runtime, TryCurrentError};



/// Provides access to the atra runtime
pub struct AtraRuntime {
    main: Runtime,
    io: Option<Runtime>, // io is always dropped last.
}

impl AtraRuntime {
    pub fn new(general: Runtime, io: Option<Runtime>) -> Self {
        Self {
            main: general,
            io
        }
    }

    /// Returns a reference to a special handle used for io tasks
    #[allow(dead_code)] #[inline] pub fn io(&self) -> Option<&Runtime> {
        self.io.as_ref()
    }

    /// Returns a reference to the main runtime used of all tasks
    #[allow(dead_code)] #[inline] pub fn main(&self) -> &Runtime {
        &self.main
    }

    pub fn handle(&self) -> AtraHandle {
        AtraHandle::new(
            self.main.handle().clone(),
            match &self.io {
                None => {None}
                Some(value) => {Some(value.handle().clone())}
            }
        )
    }
}

impl Deref for AtraRuntime {
    type Target = Runtime;

    fn deref(&self) -> &Self::Target {
        &self.main
    }
}


/// A handle to the atra runtime
#[derive(Debug, Clone)]
pub struct AtraHandle {
    main: Handle,
    io: Option<Handle>,
}

impl AtraHandle {
    pub fn new(general: Handle, io: Option<Handle>) -> Self {
        Self {
            main: general,
            io
        }
    }

    pub fn some(general: Handle, io: Option<Handle>) -> OptionalAtraHandle {
        OptionalAtraHandle::Some(Self::new(general, io))
    }

    pub fn none() -> OptionalAtraHandle {
        OptionalAtraHandle::None
    }


    /// Returns a reference to a special handle used for io tasks
    #[inline] pub fn io(&self) -> Option<&Handle> {
        self.io.as_ref()
    }

    /// Returns a reference to the main runtime used of all tasks
    #[inline] pub fn main(&self) -> &Handle {
        &self.main
    }

    /// Returns either the io handle or the main handle
    #[inline] pub fn io_or_main(&self) -> &Handle {
        match self.io() {
            None => {self.main()}
            Some(handle) => {handle}
        }
    }

    pub fn as_optional(&self) -> OptionalAtraHandle {
        OptionalAtraHandle::Some(self.clone())
    }
}

impl Deref for AtraHandle {
    type Target = Handle;

    fn deref(&self) -> &Self::Target {
        &self.main
    }
}

/// An optional Atra handle
#[derive(Debug, Clone)]
pub enum OptionalAtraHandle {
    Some(AtraHandle),
    None
}

impl OptionalAtraHandle {
    /// Panics if None and not called in an async runtime.
    /// See [Handle::current] for more information.
    pub fn io_or_main_or_current(&self) -> Handle {
        match self {
            Self::None => {Handle::current()}
            Self::Some(handle) => {
                handle.io_or_main().clone()
            }
        }
    }

    /// Returns [TryCurrentError] if None and not called in an async runtime.
    /// See [Handle::try_current] for more information.
    pub fn try_io_or_main_or_current(&self) -> Result<Handle, TryCurrentError>{
        match self {
            Self::None => {Handle::try_current()}
            Self::Some(handle) => {
                Ok(handle.io_or_main().clone())
            }
        }
    }

    pub fn try_io(&self) -> Result<Handle, TryIOError> {
        match self {
            Self::None => {Err(TryIOError)}
            Self::Some(handle) => {
                match handle.io {
                    None => {Err(TryIOError)}
                    Some(ref handle) => {Ok(handle.clone())}
                }
            }
        }
    }

    pub fn try_main(&self) -> Result<Handle, TryMainError> {
        match self {
            Self::None => {Err(TryMainError)}
            Self::Some(handle) => {
                Ok(handle.main.clone())
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Error)]
#[error("No io handle found")]
pub struct TryIOError;

#[derive(Debug, Copy, Clone, Error)]
#[error("No main handle found")]
pub struct TryMainError;
