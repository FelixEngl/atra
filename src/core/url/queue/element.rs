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

use std::any::type_name;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use crate::core::queue::AgingQueueElement;

/// An entry for the url queue.
pub struct UrlQueueElementBase<T> {
    /// The distance between this url and the origin.
    pub is_seed: bool,
    pub age: u32,
    pub host_was_in_use: bool,
    pub target: T
}

impl<T> AgingQueueElement for UrlQueueElementBase<T> {
    fn age_by_one(&mut self) {
        self.age += 1
    }
}


impl<T> UrlQueueElementBase<T> {
    pub fn new(is_seed: bool, age: u32, host_was_in_use: bool, target: T) -> Self {
        Self {
            is_seed,
            age,
            host_was_in_use,
            target
        }
    }

    pub fn map<R, F>(self, mapping: F) -> UrlQueueElementBase<R> where F: FnOnce(T) -> R {
        UrlQueueElementBase::new(
            self.is_seed,
            self.age,
            self.host_was_in_use,
            mapping(self.target)
        )
    }

    pub fn map_or<R, F>(self, mapping: F) -> Option<UrlQueueElementBase<R>> where F: FnOnce(T) -> Option<R> {
        Some(
            UrlQueueElementBase::new(
                self.is_seed,
                self.age,
                self.host_was_in_use,
                mapping(self.target)?
            )
        )
    }

    pub fn map_or_err<R, E, F>(self, mapping: F) -> Result<UrlQueueElementBase<R>, E> where F: FnOnce(T) -> Result<R, E> {
        Ok(
            UrlQueueElementBase::new(
                self.is_seed,
                self.age,
                self.host_was_in_use,
                mapping(self.target)?
            )
        )
    }
}



impl<T: Clone> Clone for UrlQueueElementBase<T> {
    fn clone(&self) -> Self {
        Self {
            is_seed: self.is_seed,
            age: self.age,
            host_was_in_use: self.host_was_in_use,
            target: self.target.clone()
        }
    }
}

impl<T: Debug> Debug for UrlQueueElementBase<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut deb = f.debug_struct(type_name::<UrlQueueElementBase<T>>());
        deb.field("is_seed", &self.is_seed);
        deb.field("age", &self.age);
        deb.field("host_was_in_use", &self.host_was_in_use);
        deb.field("target", &self.target);
        deb.finish()
    }
}

impl<T: Copy> Copy for UrlQueueElementBase<T> {}

impl<T: Hash> Hash for UrlQueueElementBase<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_seed.hash(state);
        self.target.hash(state)
    }
}

impl<T: Display> Display for UrlQueueElementBase<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,
               "CrawlElement(is_seed: {}, age: {}, host_was_in_use: {}, target: {})",
               self.is_seed,
               self.age,
               self.host_was_in_use,
               self.target
        )
    }
}


impl<T> From<UrlQueueElementBase<T>> for (bool, u32, bool, T) {
    fn from(value: UrlQueueElementBase<T>) -> Self {
        (value.is_seed, value.age, value.host_was_in_use, value.target)
    }
}

impl<U: Into<T>, T> From<(bool, u32, bool, U)> for UrlQueueElementBase<T> {
    fn from(value: (bool, u32, bool, U)) -> Self {
        Self::new(value.0, value.1, value.2, value.3.into())
    }
}

impl<T> AsRef<T> for UrlQueueElementBase<T> {
    fn as_ref(&self) -> &T {
        &self.target
    }
}

impl<T: PartialEq> PartialEq<UrlQueueElementBase<T>> for UrlQueueElementBase<T>   {
    fn eq(&self, other: &UrlQueueElementBase<T>) -> bool {
        self.target.eq(&other.target)
    }
}


impl<T: Serialize> Serialize for UrlQueueElementBase<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut tup = serializer.serialize_tuple(4)?;
        tup.serialize_element(&self.is_seed)?;
        tup.serialize_element(&self.age)?;
        tup.serialize_element(&self.host_was_in_use)?;
        tup.serialize_element(&self.target)?;
        tup.end()
    }
}

struct CrawlMetaContainerVisitor<T>{
    _phantom: PhantomData<T>
}

impl<T> CrawlMetaContainerVisitor<T> {
    fn new() -> Self {
        Self{_phantom: PhantomData}
    }
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for CrawlMetaContainerVisitor<T> {
    type Value = UrlQueueElementBase<T>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a UrlQueueEntry in tuple format.")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let is_seed: bool = seq.next_element()?.unwrap();
        let age: u32 = seq.next_element()?.unwrap();
        let domain_was_in_use: bool = seq.next_element()?.unwrap();
        let target: T = seq.next_element()?.unwrap();
        Ok(
            UrlQueueElementBase {
                is_seed,
                age,
                host_was_in_use: domain_was_in_use,
                target
            }
        )
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for UrlQueueElementBase<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_tuple(4, CrawlMetaContainerVisitor::new())
    }
}

