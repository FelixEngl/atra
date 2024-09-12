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

use crate::link_state::{LinkState, LinkStateType};
use rocksdb::{BlockBasedOptions, DBCompressionType, MergeOperands, Options, SliceTransform, DB};
use std::fmt::Debug;
use std::path::Path;
use thiserror::Error;

pub const LINK_STATE_DB_CF: &'static str = "ls";
pub const CRAWL_DB_CF: &'static str = "cr";
pub const ROBOTS_TXT_DB_CF: &'static str = "rt";
pub const SEED_ID_DB_CF: &'static str = "si";

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

#[cfg(test)]
use rocksdb::Error;

/// Deletes a db
#[cfg(test)]
pub fn destroy_db<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    if path.as_ref().exists() {
        DB::destroy(&db_options(), path)
    } else {
        Ok(())
    }
}

/// Creates the open option
fn create_open_options() -> (Options, [(&'static str, Options); 4]) {
    let db_options = db_options();
    let cf_options = [
        (LINK_STATE_DB_CF, link_state_cf_options()),
        (CRAWL_DB_CF, crawled_page_cf_options()),
        (ROBOTS_TXT_DB_CF, robots_txt_cf_options()),
        (SEED_ID_DB_CF, seed_id_cf_options()),
    ];
    (db_options, cf_options)
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

fn db_options() -> Options {
    // May need https://github.com/facebook/rocksdb/wiki/BlobDB#performance-tuning

    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    // let max_wal_file_size = match sys_info::mem_info() {
    //     Ok(result) => {
    //         min(result.free / 10, (64*20).megabytes().bytes().as_u64())
    //     }
    //     Err(_) => {
    //         (64*20).megabytes().bytes().as_u64()
    //     }
    // };
    // options.set_max_total_wal_size(max_wal_file_size);
    // options.set_bottommost_compression_options(1.megabytes().bytes().as_u64() as i32, true);
    // options.com
    // options.set_bottommost_compression_type(DBCompressionType::Zstd);
    // options.set_bottommost_zstd_max_train_bytes(1.megabytes().bytes().as_u64() as i32, true);
    options
}

fn merge_linkstate(
    new_key: &[u8],
    existing_val: Option<&[u8]>,
    operands: &MergeOperands,
) -> Option<Vec<u8>> {
    let mut merge_result = if let Some(first) = existing_val {
        Vec::from(first)
    } else {
        Vec::new()
    };
    for operand in operands {
        if operand.is_empty() {
            continue;
        }
        if merge_result.is_empty() {
            merge_result.extend_from_slice(operand);
            continue;
        }
        let upsert_time = LinkState::read_timestamp(&merge_result);
        let new_time = LinkState::read_timestamp(operand);

        let upsert_time = if let Ok(upsert_time) = upsert_time {
            upsert_time
        } else {
            if new_time.is_ok() {
                log::error!("Illegal value for {:?}. Does not contain a timestamp in the merge target, but can fallback to new!", new_key);
                merge_result.clear();
                merge_result.extend_from_slice(operand);
            } else {
                log::error!("Illegal value for {:?}. Does not contain a timestamp in the merge target or the new value!", new_key);
            }
            continue;
        };

        let new_time = if let Ok(new_time) = new_time {
            new_time
        } else {
            log::error!(
                "Illegal value for {:?}. Does not contain a timestamp in the new value!",
                new_key
            );
            continue;
        };

        if upsert_time < new_time {
            let mut last_significant = merge_result[LinkState::LAST_SIGNIFICANT_TYP_POS];
            let old = merge_result[LinkState::TYP_POS];
            if LinkStateType::is_significant_raw(old) && old > last_significant {
                last_significant = merge_result[LinkState::TYP_POS];
            }
            merge_result.clear();
            merge_result.extend_from_slice(operand);
            merge_result[LinkState::LAST_SIGNIFICANT_TYP_POS] = last_significant;
        }
    }
    Some(merge_result)
}

pub fn link_state_cf_options() -> Options {
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options.set_merge_operator_associative("merge_linkstate", merge_linkstate);
    options
}

pub fn robots_txt_cf_options() -> Options {
    let mut options: Options = Default::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options.set_enable_blob_files(true);
    options.set_blob_compression_type(DBCompressionType::Zstd);
    options
}

pub fn seed_id_cf_options() -> Options {
    let mut options: Options = Default::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options
}

pub fn crawled_page_cf_options() -> Options {
    let mut options: Options = Default::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    // https://github.com/facebook/rocksdb/wiki/RocksDB-Bloom-Filter
    let mut bb_options = BlockBasedOptions::default();
    bb_options.set_bloom_filter(10.0, true);
    bb_options.set_whole_key_filtering(true);
    options.set_block_based_table_factory(&bb_options);

    options.set_prefix_extractor(SliceTransform::create_fixed_prefix(15));

    options
}

// pub fn crawled_page_body_cf_options() -> Options {
//     let mut options: Options = Default::default();
//     options.create_missing_column_families(true);
//     options.create_if_missing(true);
//
//     options.set_enable_blob_files(true);
//     options.set_blob_compression_type(DBCompressionType::Zstd);
//
//     // Alternative??
//     // // https://github.com/facebook/rocksdb/wiki/Prefix-Seek
//     // options.set_prefix_extractor(
//     //     SliceTransform::create(
//     //         "url_prefix",
//     //         transform_binary_url_to_prefix_slice,
//     //         None
//     //     )
//     // );
//     options
// }

#[cfg(test)]
mod test {
    #[test]
    fn can_extract() {}
}
