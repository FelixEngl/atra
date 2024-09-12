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

use crate::io::errors::ErrorWithPath;
use camino::Utf8Path;

/// The owner of some kind of file. This trait allows to perform various actions on the read/write process.
pub trait FileOwner {
    /// Returns true if [path] is in use.
    #[allow(dead_code)]
    fn is_in_use<Q: AsRef<Utf8Path>>(&self, path: Q) -> bool;

    /// Waits until the target [path] is free or frees it in some way.
    async fn wait_until_free_path<Q: AsRef<Utf8Path>>(
        &self,
        target: Q,
    ) -> Result<(), ErrorWithPath>;
}
