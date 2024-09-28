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

use crate::client::traits::AtraClient;
use crate::client::{build_classic_client, ClientWithUserAgent};
use crate::contexts::traits::{SupportsConfigs, SupportsCrawling};
use crate::seed::BasicSeed;
use crate::test_impls::{FakeClient, FakeResponse, FakeResponseError};
use crate::url::AtraUri;
use std::error::Error;

/// A provider for a client used to download things.
pub trait ClientProvider {
    type Client: AtraClient;

    type Error: Error + Send + Sync;

    /// Provide a client for a context and a specific seed.
    fn provide<C, T>(&self, context: &C, seed: &T) -> Result<Self::Client, Self::Error>
    where
        C: SupportsCrawling + SupportsConfigs,
        T: BasicSeed;
}

/// The default implementation for Atra
#[derive(Default)]
pub struct DefaultAtraProvider;

impl ClientProvider for DefaultAtraProvider {
    type Client = ClientWithUserAgent;
    type Error = reqwest::Error;

    fn provide<C, T>(&self, context: &C, seed: &T) -> Result<Self::Client, Self::Error>
    where
        C: SupportsCrawling + SupportsConfigs,
        T: BasicSeed,
    {
        let useragent = context
            .configs()
            .crawl
            .user_agent
            .get_user_agent()
            .to_string();
        let client = build_classic_client(context, seed, &useragent)?;
        let client = ClientWithUserAgent::new(useragent, client);
        Ok(client)
    }
}

/// A fake client provider.
pub struct FakeClientProvider {
    inner: FakeClient,
}

impl FakeClientProvider {
    pub fn new() -> Self {
        Self {
            inner: FakeClient::new(),
        }
    }

    pub fn clear(&self) {
        self.inner.clear()
    }

    pub fn insert(&self, key: AtraUri, value: Result<FakeResponse, FakeResponseError>) {
        self.inner.insert(key, value);
    }
}

impl ClientProvider for FakeClientProvider {
    type Client = FakeClient;
    type Error = FakeResponseError;

    fn provide<C, T>(&self, _: &C, _: &T) -> Result<Self::Client, Self::Error>
    where
        C: SupportsCrawling + SupportsConfigs,
        T: BasicSeed,
    {
        Ok(self.inner.clone())
    }
}
