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

use std::time::SystemTime;
use case_insensitive_string::CaseInsensitiveString;
use compact_str::CompactString;
use thiserror::Error;
use crate::core::UrlWithDepth;

/// Errors of the domain manager
#[derive(Debug, Error)]
pub enum DomainManagerError {
    #[error("There was no domain in the url")]
    NoDomainError(UrlWithDepth),
    #[error("The domain is already in use {0:?}")]
    AlreadyOccupied(CaseInsensitiveString)
}


/// Returns the poison state of the guard at this specific moment.
#[derive(Debug, Error, Clone)]
pub enum GuardPoisonedError {
    #[error("The domain {0} is not registered but guarded!")]
    DomainMissing(CompactString),
    #[error("The guard flag of the domain {0} is not set!")]
    InUseNotSet(CompactString),
    #[error("The guard timestamp of the domain {0} is not set!")]
    NoTimestampSet(CompactString),
    #[error("The guard timestamp of the domain {0} is set to {2:?} but should be {1:?}!")]
    WrongTimestampSet(CompactString, SystemTime, SystemTime)
}
