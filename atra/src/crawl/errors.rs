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

use thiserror::Error;
use crate::origin::AtraUrlOrigin;

/// Errors when creating a url
#[derive(Debug, Error)]
pub enum SeedCreationError {
    /// This error is returned when the domain of a guard and an url is not the same.
    #[error("The host {origin_from_guard} (Guard) is not the same as {origin_from_url} (url)!")]
    GuardAndUrlDifferInOrigin {
        origin_from_guard: AtraUrlOrigin,
        origin_from_url: AtraUrlOrigin
    },

    #[error("No origin found for the url!")]
    NoOrigin
}