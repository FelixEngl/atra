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

pub mod link_state;
pub mod page_writers;
pub mod mime;
pub mod page_processing;
pub mod util_selectors;
pub mod decoding;
pub mod response;
pub mod fetching;
pub mod robots;
pub mod header_map_extensions;
pub mod page_type;
pub mod blacklist;
pub mod depth;
pub mod serde_util;
pub mod rocksdb_ext;
pub mod seed_provider;
pub mod shutdown;
pub mod crawl;
pub mod database_error;
pub mod extraction;
pub mod file_format_inference;
pub mod ini_ext;
pub mod config;
pub mod io;
pub mod seeds;
pub mod contexts;
pub mod domain;
pub mod sitemaps;
pub mod queue;
pub mod url;
pub mod data_holder;
pub mod system;
pub mod web_graph;
pub mod warc;
pub mod digest;
pub mod runtime;
pub mod worker;
pub mod sync;

pub use url::url_with_depth::UrlWithDepth;
pub use data_holder::*;


// Bare metal platforms usually have very small amounts of RAM
// (in the order of hundreds of KB)
pub const DEFAULT_BUF_SIZE: usize = if cfg!(target_os = "espidf") { 512 } else { 8 * 1024 };
