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

pub use file_content::*;

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};

mod file_content;
pub mod file_format_detection;
mod information;
pub mod mime;
pub mod mime_ext;
pub(crate) mod mime_serialize;
pub mod supported;

use crate::fetching::ResponseData;
pub use information::*;

#[inline(always)]
pub fn determine_format_for_response<C>(
    context: &C,
    response: &mut ResponseData,
) -> AtraFileInformation
where
    C: SupportsConfigs + SupportsFileSystemAccess,
{
    determine_format(
        context,
        FileFormatData::new(
            response.headers.as_ref(),
            &mut response.content,
            Some(&response.url),
            None,
        ),
    )
}

#[inline(always)]
pub fn determine_format<C, D>(context: &C, mut data: FileFormatData<D>) -> AtraFileInformation
where
    C: SupportsConfigs + SupportsFileSystemAccess,
    D: FileContentReader,
{
    AtraFileInformation::determine(context, &mut data)
}
