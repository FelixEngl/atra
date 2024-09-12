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

use mime::Mime;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(remote = "Mime")]
pub(crate) struct MimeDef(#[serde(getter = "Mime::to_string")] String);

impl From<MimeDef> for Mime {
    fn from(value: MimeDef) -> Self {
        Mime::from_str(&value.0).unwrap()
    }
}

impl From<Mime> for MimeDef {
    fn from(value: Mime) -> Self {
        MimeDef(value.to_string())
    }
}

impl From<&Mime> for MimeDef {
    fn from(value: &Mime) -> Self {
        MimeDef(value.to_string())
    }
}

pub(crate) mod for_vec {
    use crate::format::mime_serialize::MimeDef;
    use itertools::Itertools;
    use mime::Mime;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(values: &Vec<Mime>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Helper<'a>(#[serde(with = "MimeDef")] &'a Mime);
        ser.collect_seq(values.iter().map(Helper))
    }

    pub fn deserialize<'de, D>(deser: D) -> Result<Vec<Mime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(#[serde(with = "MimeDef")] Mime);
        Ok(Vec::deserialize(deser)?
            .into_iter()
            .map(|Helper(value)| value)
            .collect_vec())
    }
}
