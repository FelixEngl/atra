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

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use camino::Utf8PathBuf;
use rocksdb::IteratorMode;
use serde::Serialize;
use crate::app::instruction::{InstructionError, string_to_config_path};
use crate::contexts::local::LocalContext;
use crate::crawl::{SlimCrawlResult, StoredDataHint};
use crate::url::AtraUri;
use crate::warc_ext::WarcSkipInstruction;

pub(crate) fn dump(crawl_path: String, output_dir: Option<String>) -> Result<(), InstructionError> {
    let config = string_to_config_path(&crawl_path)?;
    let local = LocalContext::new_without_runtime(config)
        .expect("Was not able to load context for reading!");
    let output_dir = if let Some(output_dir) = output_dir {
        let new_dir = Utf8PathBuf::from(output_dir);
        if !new_dir.exists() {
            std::fs::create_dir_all(&new_dir)?;
        }
        new_dir
    } else {
        Utf8PathBuf::from(crawl_path)
    };
    assert!(output_dir.is_dir());
    let output_data = output_dir.join("meta.jsonbulk");
    let mut writer = BufWriter::new(File::options().write(true).create_new(true).open(output_data)?);
    let mut warc_files = HashSet::new();
    for value in local.crawl_db().iter(IteratorMode::Start) {
        match value {
            Ok((k, v)) => {
                let uri: AtraUri = unsafe{std::str::from_utf8_unchecked(k.as_ref())}.parse().expect("This should never fail!");
                let data: SlimCrawlResult = match bincode::deserialize_from(v.as_ref()) {
                    Ok(value) => {
                        value
                    }
                    Err(err) => {
                        log::warn!("Failed to deserialize data from {uri} with: {err}");
                        continue
                    }
                };
                match &data.stored_data_hint {
                    StoredDataHint::Warc(value) => {
                        match value {
                            WarcSkipInstruction::Single { pointer, .. } => {
                                if !warc_files.contains(pointer.path()) {
                                    warc_files.insert(pointer.path().to_path_buf());
                                }
                            }
                            WarcSkipInstruction::Multiple { pointers, .. } => {
                                for pointer in pointers {
                                    if !warc_files.contains(pointer.path()) {
                                        warc_files.insert(pointer.path().to_path_buf());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                serde_json::to_writer(&mut writer, &Entry{url: uri, meta: data}).map_err(InstructionError::DumbSerialisationError)?;
            }
            Err(_) => {
                continue
            }
        }
    }
    writer.flush()?;
    drop(writer);
    let warc_path = output_dir.join("warc_files.txt");
    let mut writer = BufWriter::new(File::options().write(true).create_new(true).open(warc_path)?);
    for value in warc_files {
        write!(&mut writer, "{}\n", value.canonicalize_utf8()?)?;
    }
    writer.flush()?;
    Ok(())
}


#[derive(Debug, Serialize)]
struct Entry {
    url: AtraUri,
    meta: SlimCrawlResult
}
