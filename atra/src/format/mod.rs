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

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{Read, Seek};

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

pub use file_content::*;
use warc::media_type::MediaType;

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::data::RawVecData;
use crate::fetching::ResponseData;
use crate::format::file_format_detection::{DetectedFileFormat, infer_file_formats};
use crate::format::mime::{determine_mime_information, MimeType};
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::url::UrlWithDepth;

pub mod file_format_detection;
pub mod mime;
pub mod mime_ext;
pub(crate) mod mime_serialize;
pub mod supported;
mod file_content;
mod information;

pub use information::*;

