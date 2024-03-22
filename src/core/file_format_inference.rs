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

use std::io::{Read, Seek};
use file_format::FileFormat;
use log;


pub fn infer_by_content<R: Read + Seek>(content_reader: R) -> FileFormat {
    match FileFormat::from_reader(content_reader) {
        Ok(value) => {
            value
        }
        Err(err) => {
            log::warn!("Failed to analyze with {err}");
            FileFormat::Empty
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/codegen_file_format.rs"));

pub fn infer_by_mime(mime_type: &str) -> Option<&'static [FileFormat]> {
    MEDIA_TYPE_TO_FILE_FORMAT.get(mime_type).map(|value| *value)
}

#[allow(dead_code)]
pub fn infer_by_extension(extension: &str) -> Option<&'static [FileFormat]> {
    EXTENSION_FILE_FORMAT.get(extension).map(|value| *value)
}






