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

use std::process::ExitCode;
use crate::app::{exec_args, AtraArgs};
use clap::Parser;

mod app;
mod blacklist;
mod client;
mod config;
mod contexts;
mod crawl;
mod data;
mod database;
mod decoding;
mod extraction;
mod fetching;
mod format;
mod gdbr;
mod html;
mod io;
mod link_state;
mod queue;
mod recrawl_management;
mod robots;
mod runtime;
mod seed;
mod stores;
mod sync;
#[cfg(test)]
mod test_impls;
mod toolkit;
mod url;
mod warc_ext;
mod web_graph;

fn main() -> ExitCode {
    exec_args(AtraArgs::parse())
}
