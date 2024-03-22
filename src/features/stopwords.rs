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

use std::collections::HashSet;
use std::fmt::{Display};
use std::io;
use std::ops::Deref;
use std::path::Path;
use compact_str::CompactString;
use thiserror::Error;
use crate::features::stopwords::StopWordsError::NoStopWordListFound;

#[derive(Debug, Error)]
pub enum StopWordsError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error("No stopword list was foiund!")]
    NoStopWordListFound
}

#[derive(Debug)]
#[repr(transparent)]
pub struct StopWords(HashSet<CompactString>);

impl StopWords {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, StopWordsError> {
        // let mut set = HashSet::new();

        todo!()
    }

    pub fn load_from_dir<P: AsRef<Path>>(path: P, lang: isolang::Language) -> Result<Self, StopWordsError> {
        let root = path.as_ref();
        if let Some(s) = lang.to_639_1() {
            let target = root.join(format!("{s}.txt"));
            if target.exists() {
                return Self::load(target)
            }
        }
        let target = root.join(format!("{}.txt", lang.to_639_3()));
        if target.exists() {
            return Self::load(target)
        }

        let target = root.join(format!("{}.txt", lang.to_name()));
        if target.exists() {
            return Self::load(target)
        }

        Err(NoStopWordListFound)
    }
}



impl Deref for StopWords {
    type Target = HashSet<CompactString>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}