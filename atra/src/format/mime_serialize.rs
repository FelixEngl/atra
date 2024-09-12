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
