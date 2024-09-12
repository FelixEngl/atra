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

use case_insensitive_string::CaseInsensitiveString;
use psl::Domain;
use url::Url;

/// Get the domain name from the [url] as [CaseInsensitiveString].
/// Returns None if there is no domain
pub fn domain_name(url: &Url) -> Option<CaseInsensitiveString> {
    domain_name_raw(url).map(|value| CaseInsensitiveString::new(value.as_bytes()))
}

/// Get the raw domain name from [url]
/// Returns None if there is no domain
pub fn domain_name_raw(url: &Url) -> Option<Domain> {
    psl::domain(url.host_str()?.as_bytes())
}
