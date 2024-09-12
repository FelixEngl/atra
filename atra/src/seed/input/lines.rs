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
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

/// A simple reader for some seeds. Allows to ignore single seeds by using #
pub fn read_seeds<P: AsRef<Path>>(path: P) -> Result<HashSet<String>, std::io::Error> {
    let mut seeds = HashSet::new();

    let lines = BufReader::new(File::open(path)?).lines();

    for line in lines.flatten() {
        let line = line.trim();
        if line.starts_with("#") || line.is_empty() {
            continue;
        }
        let line = if line.starts_with("\\#") {
            &line[1..]
        } else {
            &line
        };

        seeds.insert(line.to_string());
    }
    Ok(seeds)
}
