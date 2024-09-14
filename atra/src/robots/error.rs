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

use crate::database::DatabaseError;
use thiserror::Error;
use url::ParseError;

/// Errors while working with robots.txt
#[derive(Error, Debug)]
pub enum RobotsError<ClientError: std::error::Error> {
    #[error("Some kind of parsing error happened for the url.")]
    InvalidUrl(#[from] ParseError),
    #[error("The robots.txt parser had some problems.")]
    InvalidRobotsTxt(#[source] anyhow::Error),
    #[error("The client failed to send the request: {0}")]
    ClientWasNotAbleToSend(ClientError),
    #[error("The url had no domain.")]
    NoDomainForUrl,
    #[error("The database had some kind of issue")]
    Database(#[from] DatabaseError),
    #[error("The serialisation had some kind of issue")]
    Serialisation(#[from] bincode::Error),
}
