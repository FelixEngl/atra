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

#![allow(missing_docs)]

use strum::{AsRefStr, Display, EnumString};

#[derive(Clone, Debug, PartialEq, EnumString, AsRefStr, Display)]
pub enum WarcRecordType {
    #[strum(to_string = "warcinfo")]WarcInfo,
    #[strum(to_string = "response")]Response,
    #[strum(to_string = "resource")]Resource,
    #[strum(to_string = "request")]Request,
    #[strum(to_string = "metadata")]Metadata,
    #[strum(to_string = "revisit")]Revisit,
    #[strum(to_string = "conversion")]Conversion,
    #[strum(to_string = "continuation")]Continuation,
    #[strum(default)] Unknown(String),
}