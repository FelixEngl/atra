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

use itertools::Itertools;

/// Extracts everything from the [`file_name`] that is written after the first dot.
pub fn extract_file_extensions_from_file_name(file_name: &str) -> Option<Vec<&str>> {
    let sep = file_name.find('.')?;
    if sep == file_name.len() - 1 {
        return None;
    }
    let result = (&file_name[sep + 1..])
        .split_terminator('.')
        .filter(|value| !value.is_empty())
        .collect_vec();
    (!result.is_empty()).then_some(result)
}
