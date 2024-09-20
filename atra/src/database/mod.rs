// Copyright 2024 Felix Engl
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

mod rocksdb_ext;

mod database_error;
mod options;

use rocksdb::{DBIteratorWithThreadMode, DBWithThreadMode, IteratorMode, MultiThreaded, ReadOptions, DB};
pub use database_error::*;
pub use options::*;
pub use rocksdb_ext::*;


pub fn get_len(db: &DB, handle: std::sync::Arc<rocksdb::BoundColumnFamily>) -> usize {
    let mut options = ReadOptions::default();
    options.fill_cache(false);
    match db.flush_cf(&handle) {
        Ok(_) => {}
        Err(err) => {
            log::warn!("Failed to flush before scanning {err}");
        }
    };

    let mut iter = db.raw_iterator_cf_opt(
        &handle,
        options
    );
    iter.seek_to_first();
    let mut ct: usize = 0;
    while iter.valid() {
        ct += 1;
        iter.next();
    }
    ct
}

pub fn execute_iter<'a>(db: &'a DB, handle: std::sync::Arc<rocksdb::BoundColumnFamily<'a>>) -> DBIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>> {
    let mut options = ReadOptions::default();
    options.fill_cache(false);
    match db.flush_cf(&handle) {
        Ok(_) => {}
        Err(err) => {
            log::warn!("Failed to flush before scanning {err}");
        }
    };

    db.iterator_cf_opt(
        &handle,
        options,
        IteratorMode::Start
    )
}