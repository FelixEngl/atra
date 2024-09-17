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

use crate::database::DatabaseError;
use std::array::TryFromSliceError;
use thiserror::Error;
use time::error;

/// The errors when creating or writing a linkstate
#[derive(Debug, Error)]
pub enum LinkStateError {
    #[error("The buffer is emptys")]
    EmptyBuffer,
    #[error("The buffer requires a length of {0} but has only {1}.")]
    BufferTooSmall(usize, usize),
    #[error(transparent)]
    NumberConversionError(#[from] TryFromSliceError),
    #[error("The marker {0} is unknown!")]
    IllegalMarker(u8),
    #[error(transparent)]
    TimestampNotReconstructable(#[from] error::ComponentRange),
    #[error("Not convertible to bool {0}")]
    NotConvertibleToBool(u8),
}

/// Possible errors of an [LinkStateDB]
#[derive(Debug, Error)]
pub enum LinkStateDBError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    LinkStateError(#[from] LinkStateError),
}
