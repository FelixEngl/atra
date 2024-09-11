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

use std::cmp::{max, min, Ordering};
use std::fmt::{Debug, Display, Formatter};
use std::iter;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeStruct, SerializeTuple};
use smallvec::SmallVec;
use std::ops::{Add, Sub, BitAnd, BitOr, AddAssign, SubAssign, BitOrAssign, BitAndAssign};
use crate::next_key_from_map;

/// Describes the depth of an url
#[derive(Default, Copy, Clone, Eq, Ord, Hash)]
pub struct DepthDescriptor {
    /// The depth on the website
    pub depth_on_website: u64,
    /// The distance to the original seed.
    pub distance_to_seed: u64,
    /// The total amount of jumps from the seed
    pub total_distance_to_seed: u64,
}

impl DepthDescriptor {
    pub const ZERO: Self = Self::new(0,0,0);

    pub const fn new(
        depth_on_website: u64,
        distance_to_seed: u64,
        total_distance_to_seed: u64
    ) -> Self {
        Self {
            depth_on_website,
            distance_to_seed,
            total_distance_to_seed
        }
    }

    /// Merges the values to the lowest possible entry url
    pub fn merge_to_lowes(&self, rhs: &Self) -> Self {
        Self::new(
            min(self.depth_on_website, rhs.depth_on_website),
            min(self.distance_to_seed, rhs.distance_to_seed),
            min(self.total_distance_to_seed, rhs.total_distance_to_seed),
        )
    }

    pub fn as_tuple(&self) -> (u64, u64, u64) {
        (self.depth_on_website, self.distance_to_seed, self.total_distance_to_seed)
    }

    pub fn set(&mut self, value: DepthField) {
        match value {
            DepthField::DepthOnWebsite(value) => {self.depth_on_website = value}
            DepthField::DistanceToSeed(value) => {self.distance_to_seed = value}
            DepthField::TotalDistanceToSeed(value) => {self.total_distance_to_seed = value}
        }
    }
}


impl PartialEq for DepthDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.distance_to_seed == other.distance_to_seed
            && self.depth_on_website == other.depth_on_website
            && self.total_distance_to_seed == other.total_distance_to_seed
    }
}

impl PartialOrd for DepthDescriptor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if PartialEq::eq(self, other) {
            return Some(Ordering::Equal)
        }
        if self.distance_to_seed < other.distance_to_seed {
            return Some(Ordering::Less)
        }
        if self.depth_on_website < other.depth_on_website {
            return Some(Ordering::Less)
        }
        if self.total_distance_to_seed < other.total_distance_to_seed {
            return Some(Ordering::Less)
        }
        return Some(Ordering::Greater)
    }
}

impl Debug for DepthDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("DepthDescriptor");
        s.field("depth_on_website", &self.depth_on_website);
        s.field("distance_to_seed", &self.distance_to_seed);
        s.field("total_distance_to_seed", &self.total_distance_to_seed);
        s.finish()
    }
}

impl Display for DepthDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DepthDescriptor(dow:{}, d2s:{}, td2s: {})",
            self.depth_on_website,
            self.distance_to_seed,
            self.total_distance_to_seed
        )
    }
}


impl Add for DepthDescriptor {
    type Output = DepthDescriptor;

    fn add(self, rhs: Self) -> Self::Output {
        DepthDescriptor::new(
            self.depth_on_website + rhs.depth_on_website,
            self.distance_to_seed + rhs.distance_to_seed,
            self.total_distance_to_seed + rhs.total_distance_to_seed,
        )
    }
}

impl AddAssign for DepthDescriptor {
    fn add_assign(&mut self, rhs: Self) {
        self.depth_on_website += rhs.depth_on_website;
        self.distance_to_seed += rhs.distance_to_seed;
        self.total_distance_to_seed += rhs.total_distance_to_seed;
    }
}

impl Sub for DepthDescriptor {
    type Output = DepthDescriptor;

    fn sub(self, rhs: Self) -> Self::Output {
        DepthDescriptor::new(
            self.depth_on_website - rhs.depth_on_website,
            self.distance_to_seed - rhs.distance_to_seed,
            self.total_distance_to_seed - rhs.total_distance_to_seed,
        )
    }
}

impl SubAssign for DepthDescriptor {
    fn sub_assign(&mut self, rhs: Self) {
        self.depth_on_website -= rhs.depth_on_website;
        self.distance_to_seed -= rhs.distance_to_seed;
        self.total_distance_to_seed -= rhs.total_distance_to_seed;
    }
}

impl BitOr for DepthDescriptor {
    type Output = DepthDescriptor;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::new(
            min(self.depth_on_website, rhs.depth_on_website),
            min(self.distance_to_seed, rhs.distance_to_seed),
            min(self.total_distance_to_seed, rhs.total_distance_to_seed),
        )
    }
}

impl BitOrAssign for DepthDescriptor {
    fn bitor_assign(&mut self, rhs: Self) {
        self.depth_on_website = min(self.depth_on_website, rhs.depth_on_website);
        self.distance_to_seed = min(self.distance_to_seed, rhs.distance_to_seed);
        self.total_distance_to_seed = min(self.total_distance_to_seed, rhs.total_distance_to_seed);
    }
}

impl BitAnd for DepthDescriptor {
    type Output = DepthDescriptor;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::new(
            max(self.depth_on_website, rhs.depth_on_website),
            max(self.distance_to_seed, rhs.distance_to_seed),
            max(self.total_distance_to_seed, rhs.total_distance_to_seed),
        )
    }
}

impl BitAndAssign for DepthDescriptor {
    fn bitand_assign(&mut self, rhs: Self) {
        self.depth_on_website = max(self.depth_on_website, rhs.depth_on_website);
        self.distance_to_seed = max(self.distance_to_seed, rhs.distance_to_seed);
        self.total_distance_to_seed = max(self.total_distance_to_seed, rhs.total_distance_to_seed);
    }
}

macro_rules! impl_arith_for {
    ($t: ty) => {
        impl Add<$t> for DepthDescriptor {
            type Output = DepthDescriptor;

            #[inline]
            fn add(self, rhs: $t) -> Self::Output {
                self.add(DepthDescriptor::from(rhs))
            }
        }

        impl AddAssign<$t> for DepthDescriptor {
            #[inline]
            fn add_assign(&mut self, rhs: $t) {
                self.add_assign(DepthDescriptor::from(rhs))
            }
        }


        impl Sub<$t> for DepthDescriptor {
            type Output = DepthDescriptor;

            #[inline]
            fn sub(self, rhs: $t) -> Self::Output {
                self.add(DepthDescriptor::from(rhs))
            }
        }

        impl SubAssign<$t> for DepthDescriptor {
            #[inline]
            fn sub_assign(&mut self, rhs: $t) {
                self.sub_assign(DepthDescriptor::from(rhs))
            }
        }

        impl BitAnd<$t> for DepthDescriptor {
            type Output = DepthDescriptor;

            #[inline]
            fn bitand(self, rhs: $t) -> Self::Output {
                self.bitand(DepthDescriptor::from(rhs))
            }
        }

        impl BitAndAssign<$t> for DepthDescriptor {
            #[inline]
            fn bitand_assign(&mut self, rhs: $t) {
                self.bitand_assign(DepthDescriptor::from(rhs))
            }
        }

        impl BitOr<$t> for DepthDescriptor {
            type Output = DepthDescriptor;

            #[inline]
            fn bitor(self, rhs: $t) -> Self::Output {
                self.bitor(DepthDescriptor::from(rhs))
            }
        }

        impl BitOrAssign<$t> for DepthDescriptor {
            #[inline]
            fn bitor_assign(&mut self, rhs: $t) {
                self.bitor_assign(DepthDescriptor::from(rhs))
            }
        }
    };
}


impl_arith_for!((u64, u64, u64));
impl_arith_for!(DepthField);


impl From<(u64, u64, u64)> for DepthDescriptor {
    fn from(value: (u64, u64, u64)) -> Self {
        DepthDescriptor::new(
            value.0,
            value.1,
            value.2
        )
    }
}

impl From<DepthDescriptor> for (u64, u64, u64) {
    fn from(value: DepthDescriptor) -> Self {
        (value.depth_on_website, value.distance_to_seed, value.total_distance_to_seed)
    }
}

impl From<DepthField> for DepthDescriptor {
    fn from(value: DepthField) -> Self {
        match value {
            DepthField::DepthOnWebsite(value) => {
                DepthDescriptor::new(value, 0, 0)
            }
            DepthField::DistanceToSeed(value) => {
                DepthDescriptor::new(0, value, 0)
            }
            DepthField::TotalDistanceToSeed(value) => {
                DepthDescriptor::new(0, 0, value)
            }
        }
    }
}

impl Serialize for DepthDescriptor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        if serializer.is_human_readable() {
            let mut serializer = serializer.serialize_struct("DepthDescriptor", 3)?;
            serializer.serialize_field("depth_on_website", &self.depth_on_website)?;
            serializer.serialize_field("distance_to_seed", &self.distance_to_seed)?;
            serializer.serialize_field("total_distance_to_seed", &self.total_distance_to_seed)?;
            serializer.end()
        } else {
            let mut serializer = serializer.serialize_tuple(3)?;
            serializer.serialize_element(&self.depth_on_website)?;
            serializer.serialize_element(&self.distance_to_seed)?;
            serializer.serialize_element(&self.total_distance_to_seed)?;
            serializer.end()
        }
    }
}

struct DepthDescriptionVisitor;

const FIELDS: [&'static str; 3] = ["depth_on_website", "distance_to_seed", "total_distance_to_seed"];

impl<'de> Visitor<'de> for DepthDescriptionVisitor {
    type Value = DepthDescriptor;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("Something went wrong while deserializing.")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut init = Self::Value::ZERO;

        for _ in 0..3 {
            let key = next_key_from_map!(self, map, 3, &FIELDS);
            let value: u64 = map.next_value()?;
            match key {
                "depth_on_website" => init.depth_on_website = value,
                "distance_to_seed" => init.distance_to_seed = value,
                "total_distance_to_seed" => init.total_distance_to_seed = value,
                illegal => return Err(A::Error::unknown_field(illegal, &FIELDS))
            }
        }

        Ok(init)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let vec_found: SmallVec<[u64; 3]> = iter::from_fn(|| seq.next_element::<u64>().transpose()).collect::<Result<SmallVec<[u64; 3]>, A::Error>>()?;
        if vec_found.len() != 3 {
            return Err(A::Error::invalid_length(vec_found.len(), &self))
        }
        Ok(
            DepthDescriptor::new(
                vec_found[0],
                vec_found[1],
                vec_found[2],
            )
        )
    }
}

impl<'de> Deserialize<'de> for DepthDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_struct("DepthDescriptor", &FIELDS, DepthDescriptionVisitor)
        } else {
            deserializer.deserialize_tuple(3, DepthDescriptionVisitor)
        }
    }
}

/// Targets a specific field
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum DepthField {
    DepthOnWebsite(u64),
    DistanceToSeed(u64),
    TotalDistanceToSeed(u64),
}

macro_rules! add_impl {
    ($($t:ty)*) => ($(
        impl Add<$t> for DepthField {
            type Output = DepthField;

            fn add(self, other: $t) -> DepthField {
                match self {
                    Self::DepthOnWebsite(value) => Self::DepthOnWebsite(value + other as u64),
                    Self::DistanceToSeed(value) => Self::DistanceToSeed(value + other as u64),
                    Self::TotalDistanceToSeed(value) => Self::TotalDistanceToSeed(value + other as u64),
                }
            }
        }

        forward_ref_binop! { impl Add, add for DepthField, $t }
    )*)
}

macro_rules! bitand_impl {
    ($($t:ty)*) => ($(
        impl BitAnd<$t> for DepthField {
            type Output = DepthField;

            fn bitand(self, other: $t) -> DepthField {
                match self {
                    Self::DepthOnWebsite(value) => Self::DepthOnWebsite(max(value, other as u64)),
                    Self::DistanceToSeed(value) => Self::DistanceToSeed(max(value, other as u64)),
                    Self::TotalDistanceToSeed(value) => Self::TotalDistanceToSeed(max(value, other as u64)),
                }
            }
        }

        forward_ref_binop! { impl BitAnd, bitand for DepthField, $t }
    )*)
}

macro_rules! bitor_impl {
    ($($t:ty)*) => ($(
        impl BitOr<$t> for DepthField {
            type Output = DepthField;

            fn bitor(self, other: $t) -> DepthField {
                match self {
                    Self::DepthOnWebsite(value) => Self::DepthOnWebsite(min(value, other as u64)),
                    Self::DistanceToSeed(value) => Self::DistanceToSeed(min(value, other as u64)),
                    Self::TotalDistanceToSeed(value) => Self::TotalDistanceToSeed(min(value, other as u64)),
                }
            }
        }

        forward_ref_binop! { impl BitOr, bitor for DepthField, $t }
    )*)
}

macro_rules! sub_impl {
    ($($t:ty)*) => ($(
        impl Sub<$t> for DepthField {
            type Output = DepthField;

            fn sub(self, other: $t) -> DepthField {
                match self {
                    Self::DepthOnWebsite(value) => Self::DepthOnWebsite(value - other as u64),
                    Self::DistanceToSeed(value) => Self::DistanceToSeed(value - other as u64),
                    Self::TotalDistanceToSeed(value) => Self::TotalDistanceToSeed(value - other as u64),
                }
            }
        }

        forward_ref_binop! { impl Sub, sub for DepthField, $t }
    )*)
}

// implements binary operators "&T op U", "T op &U", "&T op &U"
// based on "T op U" where T and U are expected to be `Copy`able
macro_rules! forward_ref_binop {
    (impl $imp:ident, $method:ident for $t:ty, $u:ty) => {
        impl<'a> $imp<$u> for &'a $t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            fn $method(self, other: $u) -> <$t as $imp<$u>>::Output {
                $imp::$method(*self, other)
            }
        }

        impl $imp<&$u> for $t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            fn $method(self, other: &$u) -> <$t as $imp<$u>>::Output {
                $imp::$method(self, *other)
            }
        }

        impl $imp<&$u> for &$t {
            type Output = <$t as $imp<$u>>::Output;

            #[inline]
            fn $method(self, other: &$u) -> <$t as $imp<$u>>::Output {
                $imp::$method(*self, *other)
            }
        }
    }
}

add_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f32 f64 }
sub_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f32 f64 }
bitand_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f32 f64 }
bitor_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f32 f64 }



#[cfg(test)]
mod test {
    use crate::depth::DepthDescriptor;

    #[test]
    fn depth_works() {
        let depth_a = DepthDescriptor::new(3,4,5);
        let depth_b = DepthDescriptor::new(2,4,6);
        let depth_c = DepthDescriptor::new(3,4,5);
        let depth_expected = DepthDescriptor::new(2,4,5);

        assert_eq!(depth_a, depth_c);
        assert!(depth_a >= depth_c);
        assert!(depth_b < depth_c);
        assert_eq!(depth_expected, depth_a.merge_to_lowes(&depth_b).merge_to_lowes(&depth_c))
    }

    #[test]
    fn can_serialize_nonhuman(){
        let depth = DepthDescriptor::ZERO + (2, 3, 5);
        let data = bincode::serialize(&depth).expect("Why?");
        let deser = bincode::deserialize(&data).expect("Why?");
        assert_eq!(depth, deser);
    }

    #[test]
    fn can_serialize_human(){
        let depth = DepthDescriptor::ZERO + (2, 3, 5);
        let data = serde_json::to_string(&depth).expect("Why?");
        let deser = serde_json::from_str(&data).expect("Why?");
        assert_eq!(depth, deser);
    }
}