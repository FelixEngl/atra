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

mod manager;
mod cookies;

pub use manager::InMemoryCookieManager;
pub use cookies::CookieSettings;

use std::borrow::Borrow;
use std::hash::Hash;
use std::sync::Arc;
use crate::url::AtraUrlOrigin;

pub trait CookieManager {
    fn get_default_cookie(&self) -> Arc<Option<String>>;

    fn get_cookies_for<Q: ?Sized>(&self, domain: &Q) -> Arc<Option<String>>
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq;

    fn set_cookie(&self, key: AtraUrlOrigin, value: Option<String>);
    fn set_default_cookie(&self, value: Option<String>);
}