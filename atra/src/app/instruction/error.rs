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

use camino::Utf8PathBuf;
use thiserror::Error;

/// Error while parsing an instruction.
#[derive(Debug, Error)]
pub enum InstructionError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    ConfigError(#[from] config::ConfigError),
    #[error(transparent)]
    ConfigDeserializationError(serde_json::Error),
    #[error("The path {0} already exists.")]
    RootAlreadyExists(Utf8PathBuf),
    #[error(transparent)]
    DumbSerialisationError(serde_json::Error),
}
