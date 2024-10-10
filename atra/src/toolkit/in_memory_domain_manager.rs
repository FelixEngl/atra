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

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock};
use crate::url::AtraUrlOrigin;


#[derive(Debug, Default)]
pub struct InMemoryDomainMappingManager<T> {
    default: Arc<RwLock<Arc<T>>>,
    per_domain: Arc<RwLock<Option<HashMap<AtraUrlOrigin, Arc<T>>>>>
}

impl<T> InMemoryDomainMappingManager<T> {

    pub fn into_inner(self) -> (Arc<RwLock<Arc<T>>>, Arc<RwLock<Option<HashMap<AtraUrlOrigin, Arc<T>>>>>) {
        (self.default, self.per_domain)
    }

    pub fn new(default: T) -> Self {
        Self {
            default: Arc::new(RwLock::new(Arc::new(default))),
            per_domain: Default::default()
        }
    }

    pub fn get_default(&self) -> Arc<T> {
        self.default.read().unwrap().clone()
    }

    pub fn get_for<Q: ?Sized>(&self, origin: &Q) -> Arc<T>
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq
    {
        let per_domain = self.per_domain.read().unwrap();
        match per_domain.as_ref() {
            None => {
                drop(per_domain);
                self.default.read().unwrap().clone()
            }
            Some(m) => {
                match m.get(origin) {
                    None => {
                        drop(per_domain);
                        self.default.read().unwrap().clone()
                    }
                    Some(found) => {
                        found.clone()
                    }
                }
            }
        }
    }

    pub fn set(&self, key: AtraUrlOrigin, value: T) {
        let mut per_domain = self.per_domain.write().unwrap();
        match per_domain.as_mut() {
            None => {
                per_domain.insert(HashMap::new()).insert(key, Arc::new(value));
            }
            Some(targ) => {
                targ.insert(key, Arc::new(value));
            }
        }
    }

    pub fn set_default(&self, value: T) {
        let mut targ = self.default.write().unwrap();
        let _ = std::mem::replace(targ.deref_mut(), Arc::new(value));
    }

    pub fn get_content(&self) -> Content<T> {
        let default = self.default.read().unwrap().clone();
        let per_domain = self.per_domain.read().unwrap().clone();
        Content(default, per_domain)
    }
}

pub struct Content<T>(pub Arc<T>, pub Option<HashMap<AtraUrlOrigin, Arc<T>>>);

impl<T> Into<(T, Option<HashMap<AtraUrlOrigin, T>>)> for Content<T> where T: Clone {
    fn into(self) -> (T, Option<HashMap<AtraUrlOrigin, T>>) {
        (
            Arc::unwrap_or_clone(self.0),
            self.1
                .map(
                    |value|
                    value.into_iter()
                        .map(|(k, v)| (k, Arc::unwrap_or_clone(v)))
                        .collect()
                )
        )
    }
}

impl<T> Into<(T, Option<HashMap<AtraUrlOrigin, T>>)> for InMemoryDomainMappingManager<T> where T: Clone {

    fn into(self) -> (T, Option<HashMap<AtraUrlOrigin, T>>) {
        let default = match Arc::try_unwrap(self.default) {
            Ok(value) => {
                Arc::unwrap_or_clone(value.into_inner().unwrap())
            }
            Err(value) => {
                value.read().unwrap().as_ref().clone()
            }
        };

        let per_domain = match Arc::try_unwrap(self.per_domain) {
            Ok(value) => {
                value.into_inner()
                    .unwrap()
                    .map(
                        |value|
                        value
                            .into_iter()
                            .map(|(k, v)| (k, Arc::unwrap_or_clone(v)))
                            .collect()
                    )
            }
            Err(value) => {
                value.read().unwrap().as_ref().map(|value| {
                    value.iter().map(|(k, v)| {
                        (k.clone(), v.as_ref().clone())
                    }).collect()
                })
            }
        };

        (default, per_domain)
    }
}