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

use crate::database::{
    CRAWL_DB_CF, DOMAIN_MANAGER_DB_CF, LINK_STATE_DB_CF, ROBOTS_TXT_DB_CF, SEED_ID_DB_CF,
};
use crate::link_state::RawLinkState;
use rocksdb::statistics::StatsLevel;
use rocksdb::{BlockBasedOptions, DBCompressionType, Options, SliceTransform};

/// Creates the open option
pub(crate) fn create_open_options() -> (Options, [(&'static str, Options); 5]) {
    let db_options = db_options();
    let cf_options = [
        (LINK_STATE_DB_CF, link_state_cf_options()),
        (CRAWL_DB_CF, crawled_page_cf_options()),
        (ROBOTS_TXT_DB_CF, robots_txt_cf_options()),
        (SEED_ID_DB_CF, seed_id_cf_options()),
        (DOMAIN_MANAGER_DB_CF, domain_manager_cf_options()),
    ];
    (db_options, cf_options)
}

fn db_options() -> Options {
    // May need https://github.com/facebook/rocksdb/wiki/BlobDB#performance-tuning

    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    if cfg!(test) {
        options.set_statistics_level(StatsLevel::All)
    } else {
        options.set_statistics_level(StatsLevel::DisableAll)
    }

    options
}

pub fn link_state_cf_options() -> Options {
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options.set_merge_operator_associative("merge_linkstate", RawLinkState::merge_linkstate);
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

pub fn domain_manager_cf_options() -> Options {
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
