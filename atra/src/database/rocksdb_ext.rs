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

use crate::database::options::create_open_options;
#[cfg(test)]
use rocksdb::Error;
use rocksdb::{Options, DB};
use std::fmt::Debug;
use std::path::Path;
use thiserror::Error;

pub const LINK_STATE_DB_CF: &'static str = "ls";
pub const CRAWL_DB_CF: &'static str = "cr";
pub const ROBOTS_TXT_DB_CF: &'static str = "rt";
pub const SEED_ID_DB_CF: &'static str = "si";
pub const DOMAIN_MANAGER_DB_CF: &'static str = "dm";

/// Errors when opening a database.
#[derive(Debug, Error)]
pub enum OpenDBError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    RocksDB(#[from] rocksdb::Error),
}

#[macro_export]
macro_rules! declare_column_families {
    ($($self:ident.$db:ident => $name: ident($imported_name: ident))+) => {
        $(
            const $imported_name: &'static str = $crate::database::$imported_name;
            fn $name(&$self) -> std::sync::Arc<rocksdb::BoundColumnFamily> {
                unsafe{$self.$db.cf_handle(Self::$imported_name).unwrap_unchecked()}
            }
        )+
    };
}

#[macro_export]
macro_rules! db_health_check {
    ($db: ident: [$($handle_name: expr => (if test $init: ident else $message: literal))+]) => {
        $(
            if $db.cf_handle($handle_name).is_none() {
                if cfg!(test) {
                    $db.create_cf($handle_name, &$crate::database::$init()).expect(
                        format!("Handle {} was not found: '{}'", $handle_name, $message).as_str()
                    );
                } else {
                    panic!("Handle {} was not found: '{}'", $handle_name, $message);
                }
            }
        )*
    };
}

/// Opens the database in a standardized way.
pub fn open_db<P: AsRef<Path>>(path: P) -> Result<DB, OpenDBError> {
    let (db, cfs) = create_open_options();
    open_db_internal(&db, path, cfs)
}

/// A save method to open a [DB] without knowing all the cfs
fn open_db_internal<P, I, N>(opts: &Options, path: P, cf_options: I) -> Result<DB, OpenDBError>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = (N, Options)>,
    N: AsRef<str>,
{
    let path = path.as_ref();
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(DB::open_cf_with_opts(&opts, path, cf_options)?)
}

/// Deletes a db
#[cfg(test)]
pub fn destroy_db<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    if path.as_ref().exists() {
        DB::destroy(&db_options(), path)
    } else {
        Ok(())
    }
}

fn db_options() -> Options {
    todo!()
}

#[cfg(test)]
mod test {
    #[test]
    fn can_extract() {}
}
