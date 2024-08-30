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

use std::sync::LazyLock;
use data_encoding::BASE32;

static EMPTY_HASH: LazyLock<Vec<u8>> = LazyLock::new(|| {
    labeled_xxh128_digest_impl(b"")
});

#[inline] fn labeled_xxh128_digest_impl<B: AsRef<[u8]>>(data: B) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend(b"XXH128:");
    let digest = twox_hash::xxh3::hash128(data.as_ref());
    output.extend(BASE32.encode(&digest.to_be_bytes()).as_bytes());
    output
}


/// Writes a labeled, padded Base32 digest of some optional [data] into [output].
/// If no data is given it returns an empty hash
pub fn labeled_xxh128_digest<B: AsRef<[u8]>>(data: B) -> Vec<u8> {
    let bytes = data.as_ref();
    if bytes.is_empty() {
        return EMPTY_HASH.clone()
    }
    labeled_xxh128_digest_impl(data)
}


