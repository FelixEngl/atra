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

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::fetching::FetchedRequestData;
use reqwest::{IntoUrl, StatusCode};
use std::error::Error;

/// The client used by Atra to download the data.
pub trait AtraClient {
    type Error: Error + Send + Sync;

    type Response: AtraResponse<Error = Self::Error>;

    fn user_agent(&self) -> &str;

    async fn get<U>(&self, url: U) -> Result<Self::Response, Self::Error>
    where
        U: IntoUrl;

    /// Perform a network request to a resource extracting all content
    async fn retrieve<C, U>(&self, context: &C, url: U) -> Result<FetchedRequestData, Self::Error>
    where
        C: SupportsConfigs + SupportsFileSystemAccess,
        U: IntoUrl;
}

pub trait AtraResponse {
    type Error: Error + Send + Sync;

    type Bytes: AsRef<[u8]>;

    fn status(&self) -> StatusCode;
    async fn text(self) -> Result<String, Self::Error>;
    async fn bytes(self) -> Result<Self::Bytes, Self::Error>;
}
