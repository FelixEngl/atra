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

use crate::blacklist::traits::{Blacklist, ManageableBlacklist};
use crate::blacklist::PolyBlackList;
use std::sync::{Arc, RwLock};

/// Creates a managed blacklist and the corresponding reference to the managed blacklist for
/// update actions.
pub fn create_managed_blacklist<T>(blacklist: T) -> (ManagedBlacklist<T>, ManagedBlacklistSender<T>)
where
    T: ManageableBlacklist,
{
    let blacklist = Arc::new(RwLock::new(blacklist));
    let managed = ManagedBlacklist::new(blacklist.clone());
    (managed, ManagedBlacklistSender::new(blacklist))
}

///
#[derive(Debug)]
pub struct ManagedBlacklistSender<T: ManageableBlacklist> {
    sender: Arc<RwLock<T>>,
}

impl<T> Clone for ManagedBlacklistSender<T>
where
    T: ManageableBlacklist,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T> ManagedBlacklistSender<T>
where
    T: ManageableBlacklist,
{
    fn new(sender: Arc<RwLock<T>>) -> Self {
        Self { sender }
    }

    /// Update the blacklist.
    pub fn update(&self, new: T) {
        let mut received = self.sender.write().unwrap();
        *received = new
    }
}

#[derive(Debug)]
pub struct ManagedBlacklist<T = PolyBlackList>
where
    T: ManageableBlacklist,
{
    inner: Arc<RwLock<T>>,
}

impl<T> Clone for ManagedBlacklist<T>
where
    T: ManageableBlacklist,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> ManagedBlacklist<T>
where
    T: ManageableBlacklist,
{
    fn new(inner: Arc<RwLock<T>>) -> Self {
        Self { inner }
    }
}

impl<T> Blacklist for ManagedBlacklist<T>
where
    T: ManageableBlacklist,
{
    fn version(&self) -> u64 {
        self.inner.read().unwrap().version()
    }

    fn has_match_for(&self, url: &str) -> bool {
        self.inner.read().unwrap().has_match_for(url)
    }
}

#[cfg(test)]
mod test {
    use crate::blacklist::{create_managed_blacklist, Blacklist, BlacklistType, PolyBlackList};

    #[test]
    fn can_properly_update() {
        let (a, b) = create_managed_blacklist(PolyBlackList::default());

        assert!(!a.has_match_for("google.de"));
        b.update(PolyBlackList::new(1, vec!["google\\.de".to_string()]).unwrap());
        assert!(a.has_match_for("google.de"));
    }
}
