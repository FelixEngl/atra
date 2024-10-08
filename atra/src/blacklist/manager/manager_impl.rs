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

use crate::blacklist::manage::ManagedBlacklist;
use crate::blacklist::manager::BlacklistError;
use crate::blacklist::traits::{Blacklist, BlacklistType, ManageableBlacklist};
use crate::blacklist::{create_managed_blacklist, BlacklistManager, ManagedBlacklistSender};
use crate::io::simple_line::SupportsSimpleLineReader;
use crate::runtime::GracefulShutdownWithGuard;
use indexmap::IndexSet;
use itertools::Itertools;
use regex::RegexSet;
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum InMemoryBlacklistManagerInitialisationError<T: ManageableBlacklist> {
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error(transparent)]
    InitError(T::Error),
}

/// Manages a blacklist in a thread safe way.
#[derive(Debug)]
pub struct InMemoryBlacklistManager<T>
where
    T: ManageableBlacklist,
{
    inner: RwLock<InnerBlacklistManager>,
    sender: ManagedBlacklistSender<T>,
    managed: ManagedBlacklist<T>,
    _shutdown_guard: GracefulShutdownWithGuard,
}

impl<T> InMemoryBlacklistManager<T>
where
    T: ManageableBlacklist,
{
    pub fn open<P: AsRef<Path>>(
        path: P,
        shutdown_guard: GracefulShutdownWithGuard,
    ) -> Result<Self, InMemoryBlacklistManagerInitialisationError<T>> {
        let inner = RwLock::new(InnerBlacklistManager::open(path)?);
        let lock = inner.try_read().unwrap();
        let blacklist = lock
            .create_current_blacklist::<T>()
            .map_err(InMemoryBlacklistManagerInitialisationError::InitError)?;
        let (managed, sender) = create_managed_blacklist(blacklist);
        drop(lock);
        Ok(Self {
            sender,
            managed,
            _shutdown_guard: shutdown_guard,
            inner,
        })
    }

    async fn patch(&self) -> bool {
        let read = self.inner.read().await;
        if self.managed.version() < read.current_version() {
            drop(read);
            let write = self.inner.write().await;
            if self.managed.version() < write.current_version() {
                return match write.create_current_blacklist::<T>() {
                    Ok(value) => {
                        self.sender.update(value);
                        true
                    }
                    Err(err) => {
                        log::error!("Failed to update the blacklist with {err}");
                        false
                    }
                };
            }
        }
        false
    }
}

impl<T> BlacklistManager for InMemoryBlacklistManager<T>
where
    T: ManageableBlacklist,
{
    type Blacklist = T;

    async fn current_version(&self) -> u64 {
        self.inner.read().await.current_version()
    }

    async fn add(&self, value: String) -> Result<bool, BlacklistError> {
        let result = self.inner.write().await.add(value)?;
        if result {
            self.patch().await;
        }
        return Ok(result);
    }

    async fn apply_patch<I: IntoIterator<Item = String>>(&self, patch: I) {
        if self.inner.write().await.apply_patch(patch) {
            self.patch().await;
        }
    }

    async fn get_patch(&self, since_version: u64) -> Option<Vec<String>> {
        self.inner.read().await.get_patch(since_version)
    }

    async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }

    async fn get_blacklist(&self) -> ManagedBlacklist<Self::Blacklist> {
        self.managed.clone()
    }
}

/// Manages a blacklist in a not thread safe way.
/// The used info for the blacklist entries is a hashset.
#[derive(Debug)]
struct InnerBlacklistManager {
    file: BufWriter<File>,
    version_on_hdd: Option<u64>,
    blacklist_entries: IndexSet<BlacklistEntry>,
    cached_set: Option<RegexSet>,
}

impl InnerBlacklistManager {
    /// Opens the file at [path] and reads a SimpleLine-File
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        let created = if file.metadata()?.len() > 0 {
            let mut blacklist_entries = IndexSet::new();
            let lines = BufReader::new(&file)
                .to_simple_line_reader()
                .filter_ok(|value| !value.is_empty());

            for line in lines.flatten() {
                blacklist_entries.insert(BlacklistEntry::new_from_file(line));
            }

            Self {
                file: BufWriter::new(file),
                version_on_hdd: None,
                blacklist_entries,
                cached_set: None,
            }
        } else {
            let mut file = file;
            file.write(
                b"# A list of Regex-Expressions and/or URLs to be filtered by this blacklist.\
                \n# Comments can be written by starting with a #.\
                \n# To ignore the # at the beginning write \\#.\
                \n",
            )?;
            Self {
                file: BufWriter::new(file),
                version_on_hdd: None,
                blacklist_entries: IndexSet::new(),
                cached_set: None,
            }
        };

        Ok(created)
    }

    pub fn current_version(&self) -> u64 {
        return self.blacklist_entries.len() as u64;
    }

    /// Returns true if the patch changed something
    pub fn add(&mut self, value: String) -> Result<bool, BlacklistError> {
        log::debug!("Add {:?}", value);

        if value.is_empty() {
            return Err(BlacklistError::EmptyStringsNotAllowed);
        }
        if value.contains("\n") {
            return Err(BlacklistError::NewLinesNotAllowed);
        }
        if self.cached_set.is_some() {
            self.cached_set = None;
        }
        if self.version_on_hdd.is_none() {
            self.version_on_hdd = Some(self.blacklist_entries.len() as u64);
        }
        Ok(self.blacklist_entries.insert(BlacklistEntry::new(value)))
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(size) = self.version_on_hdd {
            if self.blacklist_entries.len() <= size as usize {
                assert!(
                    self.blacklist_entries
                        .iter()
                        .all(|value| value.on_file.load(Ordering::Relaxed)),
                    "Some entries are not on file but should be!"
                );
                return Ok(());
            }
        } else {
            return Ok(());
        }
        for value in self.blacklist_entries.iter() {
            if !value.on_file.load(Ordering::Relaxed) {
                self.file
                    .write_all(value.value.as_bytes())
                    .and_then(|_| self.file.write_all(b"\n"))?;
                value.set_on_file_flag()
            }
        }
        self.version_on_hdd = None;
        self.file.flush()
    }

    /// Returns true if the patch changed something
    pub fn apply_patch<I: IntoIterator<Item = String>>(&mut self, patch: I) -> bool {
        if self.version_on_hdd.is_none() {
            self.version_on_hdd = Some(self.blacklist_entries.len() as u64)
        }
        let old = self.blacklist_entries.len();
        self.blacklist_entries
            .extend(patch.into_iter().map(|it| BlacklistEntry::new(it)));
        return old != self.blacklist_entries.len();
    }

    pub fn get_patch(&self, since_version: u64) -> Option<Vec<String>> {
        if self.current_version() <= since_version {
            None
        } else {
            Some(
                self.blacklist_entries
                    .iter()
                    .dropping(since_version as usize)
                    .collect(),
            )
        }
    }

    pub fn is_empty(&self) -> bool {
        self.blacklist_entries.is_empty()
    }

    pub fn create_current_blacklist<T: BlacklistType>(&self) -> Result<T, T::Error> {
        return T::new(self.current_version(), self.blacklist_entries.iter());
    }

    #[cfg(test)]
    pub fn get_string_vec(&self) -> Vec<String> {
        Vec::from_iter(self.blacklist_entries.iter())
    }
}

impl AsRef<IndexSet<BlacklistEntry>> for InnerBlacklistManager {
    fn as_ref(&self) -> &IndexSet<BlacklistEntry> {
        &self.blacklist_entries
    }
}

impl Drop for InnerBlacklistManager {
    fn drop(&mut self) {
        // Try to flush, ignore if not necessary
        let _ = self.flush();
    }
}

/// An entry of a blacklist
#[derive(Debug)]
pub struct BlacklistEntry {
    value: String,
    on_file: AtomicBool,
}

impl BlacklistEntry {
    fn new(value: String) -> Self {
        Self {
            value,
            on_file: AtomicBool::new(false),
        }
    }

    fn new_from_file(value: String) -> Self {
        Self {
            value,
            on_file: AtomicBool::new(true),
        }
    }

    fn set_on_file_flag(&self) {
        log::trace!("Set flag for {}", self.value);
        self.on_file.swap(true, Ordering::Relaxed);
    }
}

impl FromIterator<BlacklistEntry> for Vec<String> {
    fn from_iter<T: IntoIterator<Item = BlacklistEntry>>(iter: T) -> Self {
        iter.into_iter().map(|value| value.value).collect()
    }
}

impl<'a> FromIterator<&'a BlacklistEntry> for Vec<String> {
    fn from_iter<T: IntoIterator<Item = &'a BlacklistEntry>>(iter: T) -> Self {
        iter.into_iter().map(|value| value.value.clone()).collect()
    }
}

impl From<BlacklistEntry> for String {
    fn from(value: BlacklistEntry) -> Self {
        value.value
    }
}

impl AsRef<str> for BlacklistEntry {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl Hash for BlacklistEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state)
    }
}

impl Eq for BlacklistEntry {}
impl PartialEq<Self> for BlacklistEntry {
    fn eq(&self, other: &Self) -> bool {
        return self.value == other.value;
    }
}

#[cfg(test)]
mod test {
    use super::InnerBlacklistManager;
    use scopeguard::defer;

    #[test]
    fn can_initialize() {
        defer! {
            let _ = std::fs::remove_file("blacklist1.txt");
        }
        let _ = std::fs::remove_file("blacklist1.txt");

        let mut manager = InnerBlacklistManager::open("blacklist1.txt").unwrap();
        manager.add("Test1".to_string()).unwrap();
        manager.add("Test2".to_string()).unwrap();
        manager.add("Test3".to_string()).unwrap();
        manager.add("Test4".to_string()).unwrap();

        let values = manager.get_string_vec();
        assert_eq!(4, values.len());
        assert!(values.contains(&"Test1".into()));
        assert!(values.contains(&"Test2".into()));
        assert!(values.contains(&"Test3".into()));
        assert!(values.contains(&"Test4".into()));

        drop(manager)
    }

    #[test]
    fn can_read_existing() {
        defer! {
            let _ = std::fs::remove_file("blacklist2.txt");
        }

        let _ = std::fs::remove_file("blacklist2.txt");

        let mut manager = InnerBlacklistManager::open("blacklist2.txt").unwrap();
        manager.add("Test1".to_string()).unwrap();
        manager.add("Test2".to_string()).unwrap();
        manager.add("Test3".to_string()).unwrap();
        manager.add("Test4".to_string()).unwrap();

        let values = manager.get_string_vec();
        assert_eq!(4, values.len());
        assert!(values.contains(&"Test1".into()));
        assert!(values.contains(&"Test2".into()));
        assert!(values.contains(&"Test3".into()));
        assert!(values.contains(&"Test4".into()));

        drop(manager);

        let manager = InnerBlacklistManager::open("blacklist2.txt").unwrap();
        let values = manager.get_string_vec();
        assert_eq!(4, values.len());
        assert!(values.contains(&"Test1".into()));
        assert!(values.contains(&"Test2".into()));
        assert!(values.contains(&"Test3".into()));
        assert!(values.contains(&"Test4".into()));
        drop(manager);
    }

    #[test]
    fn can_interpret_test_file() {
        let manager = InnerBlacklistManager::open("testdata/blacklist.txt").unwrap();
        let values = manager.get_string_vec();
        assert_eq!(2, values.len());
        assert!(values.contains(&"www.google.de".into()));
        assert!(values.contains(&"#.Ebay.com".into()));
    }
}
