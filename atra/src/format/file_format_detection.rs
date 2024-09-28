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

use crate::contexts::traits::SupportsFileSystemAccess;
use crate::format::mime::MimeType;
use crate::format::{FileContentReader, FileFormatData};
use file_format::FileFormat;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DetectedFileFormat {
    Unambiguous(FileFormat),
    Ambiguous(FileFormat, i32, SmallVec<[(FileFormat, i32); 1]>),
}

impl DetectedFileFormat {
    pub fn most_probable_file_format(&self) -> &FileFormat {
        match self {
            DetectedFileFormat::Unambiguous(value) | DetectedFileFormat::Ambiguous(value, _, _) => {
                value
            }
        }
    }
}

/// Infers the file format for some kind of data.
pub(crate) fn infer_file_formats<D>(
    data: &mut FileFormatData<D>,
    mime: Option<&MimeType>,
) -> Option<DetectedFileFormat>
where
    D: FileContentReader,
{
    let mut formats = HashMap::new();
    if let Ok(Some(value)) = data.content.cursor() {
        match FileFormat::from_reader(value) {
            Ok(value) => {
                formats.insert(value, 1);
            }
            Err(err) => {
                log::warn!("Failed to analyze with {err}");
            }
        }
    }

    if let Some(mime) = mime {
        for mim in mime.iter() {
            if let Some(infered) = FileFormat::from_media_type(mim.essence_str()) {
                for inf in infered {
                    match formats.entry(*inf) {
                        Entry::Occupied(mut value) => {
                            *value.get_mut() += 1;
                        }
                        Entry::Vacant(value) => {
                            value.insert(1);
                        }
                    }
                }
            }
        }
    }

    if let Some(file_extension) = data.url.and_then(|value| value.url().file_extension()) {
        if let Some(infered) = FileFormat::from_extension(file_extension) {
            for inf in infered {
                match formats.entry(*inf) {
                    Entry::Occupied(mut value) => {
                        *value.get_mut() += 1;
                    }
                    Entry::Vacant(value) => {
                        value.insert(1);
                    }
                }
            }
        }
    }

    match formats.len() {
        0 => None,
        1 => Some(DetectedFileFormat::Unambiguous(
            formats.into_keys().exactly_one().unwrap(),
        )),
        _ => {
            let mut result: VecDeque<_> = formats
                .into_iter()
                .sorted_by(|(_, cta), (_, ctb)| ctb.cmp(cta))
                .collect();
            let (most_probable, ct) = result.pop_front().unwrap();
            Some(DetectedFileFormat::Ambiguous(
                most_probable,
                ct,
                SmallVec::from_slice(result.make_contiguous()),
            ))
        }
    }
}
