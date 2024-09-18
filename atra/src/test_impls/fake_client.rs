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

use crate::client::traits::{AtraClient, AtraResponse};
use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::data::RawData;
use crate::fetching::FetchedRequestData;
use crate::url::AtraUri;
use reqwest::{IntoUrl, StatusCode};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Clone)]
pub struct FakeClient {
    value: Arc<RwLock<HashMap<AtraUri, Result<FakeResponse, FakeResponseError>>>>,
}

impl FakeClient {
    pub fn new() -> Self {
        Self {
            value: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.value.write().unwrap().clear()
    }

    pub fn insert(&self, key: AtraUri, value: Result<FakeResponse, FakeResponseError>) {
        self.value.write().unwrap().insert(key, value);
    }
}

impl AtraClient for FakeClient {
    type Error = FakeResponseError;
    type Response = FakeResponse;
    const NAME: &'static str = "FakeClient";

    fn user_agent(&self) -> &str {
        "FakeClient"
    }

    async fn get<U>(&self, url: U) -> Result<Self::Response, Self::Error>
    where
        U: IntoUrl,
    {
        let url: AtraUri = url.as_str().parse().unwrap();
        match self.value.read().unwrap().get(&url) {
            None => Ok(FakeResponse::new(Some(empty()), 1)),
            Some(value) => value.clone(),
        }
    }

    async fn retrieve<C, U>(&self, _: &C, url: U) -> Result<FetchedRequestData, Self::Error>
    where
        C: SupportsConfigs + SupportsFileSystemAccess,
        U: IntoUrl,
    {
        Ok(self.get(url).await?.req_data())
    }
}

#[derive(Debug, Error, Copy, Clone)]
#[error("FakeResponseError error_id: {0} - {1}")]
pub struct FakeResponseError(usize, FakeErrorKind);

#[derive(Debug, strum::Display, Copy, Clone)]
pub enum FakeErrorKind {
    NoData,
    NoUtf8,
}

#[derive(Clone)]
pub struct FakeResponse {
    error_id: usize,
    value: Option<FetchedRequestData>,
}

impl FakeResponse {
    pub fn new(value: Option<FetchedRequestData>, error_id: usize) -> Self {
        Self { value, error_id }
    }

    fn req_data(&self) -> FetchedRequestData {
        self.value.clone().unwrap_or_else(empty)
    }

    fn data(&self) -> Option<&Vec<u8>> {
        self.value.as_ref()?.content.as_in_memory()
    }
}

fn empty() -> FetchedRequestData {
    FetchedRequestData::new(
        RawData::None,
        None,
        StatusCode::NOT_FOUND,
        None,
        None,
        false,
    )
}

impl AtraResponse for FakeResponse {
    type Error = FakeResponseError;
    type Bytes = Vec<u8>;

    fn status(&self) -> StatusCode {
        match &self.value {
            None => StatusCode::NOT_FOUND,
            Some(value) => value.status_code,
        }
    }

    async fn text(self) -> Result<String, Self::Error> {
        match self.data() {
            Some(value) => Ok(String::from_utf8_lossy(value).to_string()),
            None => Err(FakeResponseError(self.error_id, FakeErrorKind::NoData)),
        }
    }

    async fn bytes(self) -> Result<Self::Bytes, Self::Error> {
        match self.data() {
            None => Err(FakeResponseError(self.error_id, FakeErrorKind::NoData)),
            Some(value) => Ok(value.clone()),
        }
    }
}
