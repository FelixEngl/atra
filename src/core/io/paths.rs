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

use std::borrow::{Borrow, BorrowMut};
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::{transmute};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use camino::{Utf8Path, Utf8PathBuf};
use encoding_rs::Encoding;
use serde::{Deserialize, Serialize};

/// A trait to seal some traits
trait Sealed{}

/// A path buf that knows what it is in the world of atra
#[derive(Serialize, Deserialize, Debug, Clone)]
#[repr(transparent)]
pub struct AtraPathBuf<T: PathKindMarker> {
    inner: Utf8PathBuf,
    _marker: PhantomData<T>
}

impl<T: PathKindMarker> Sealed for AtraPathBuf<T>{}


impl<T: PathKindMarker> AtraPathBuf<T> {
    pub fn new(inner: Utf8PathBuf) -> Self {
        Self{inner, _marker: PhantomData}
    }

    #[allow(dead_code)]
    #[inline] pub fn retype<O: PathKindMarker>(self) -> AtraPathBuf<O> {
        unsafe {transmute(self)}
    }

    #[allow(dead_code)]
    #[inline] pub fn retype_ref<O: PathKindMarker>(&self) -> &AtraPathBuf<O> {
        unsafe {transmute(self)}
    }
}

impl<T: PathKindMarker> PartialEq<Self> for AtraPathBuf<T> {
    #[inline] fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}
impl<T: PathKindMarker> Eq for AtraPathBuf<T> {}

impl<T: PathKindMarker> PartialOrd<Self> for AtraPathBuf<T> {
    #[inline] fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<T: PathKindMarker> Ord for AtraPathBuf<T> {
    #[inline] fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<T: PathKindMarker> Hash for AtraPathBuf<T> {
    #[inline] fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<T: PathKindMarker> Display for AtraPathBuf<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}



impl<T: PathKindMarker> Borrow<Utf8PathBuf> for AtraPathBuf<T> {
    #[inline] fn borrow(&self) -> &Utf8PathBuf {
        self
    }
}

impl<T: PathKindMarker> Borrow<Utf8Path> for AtraPathBuf<T> {
    #[inline] fn borrow(&self) -> &Utf8Path {
        self
    }
}

impl<T: PathKindMarker> BorrowMut<Utf8PathBuf> for AtraPathBuf<T> {
    #[inline] fn borrow_mut(&mut self) -> &mut Utf8PathBuf {
        self
    }
}

impl<T: PathKindMarker> Deref for AtraPathBuf<T> {
    type Target = Utf8PathBuf;

    #[inline] fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: PathKindMarker> DerefMut for AtraPathBuf<T> {
    #[inline] fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: PathKindMarker> AsRef<Utf8PathBuf> for AtraPathBuf<T> {
    #[inline] fn as_ref(&self) -> &Utf8PathBuf {
        &self.inner
    }
}


impl<T: PathKindMarker> AsRef<Utf8Path> for AtraPathBuf<T> {
    fn as_ref(&self) -> &Utf8Path {
        self.inner.as_ref()
    }
}

impl<T: PathKindMarker> AsRef<Path> for AtraPathBuf<T> {
    fn as_ref(&self) -> &Path {
        self.inner.as_ref()
    }
}

impl<T: PathKindMarker> From<Utf8PathBuf> for AtraPathBuf<T> {
    #[inline] fn from(value: Utf8PathBuf) -> Self {
        Self::new(value)
    }
}

#[allow(private_bounds)]
pub trait PathKindMarker: Sealed{}

macro_rules! create_path_kind {
    ($name: ident) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $name;
        impl Sealed for $name{}
        impl PathKindMarker for $name{}


        paste::paste! {
            #[allow(dead_code)]
            pub type [<$name PathBuf>] = AtraPathBuf<$name>;
        }

    };
}


create_path_kind!(DataFile);
create_path_kind!(DecodedDataFile);
create_path_kind!(Worker);
create_path_kind!(WarcFile);
create_path_kind!(RDFFile);

#[allow(private_bounds)]
pub trait TypedJoin<A: PathKindMarker, B: PathKindMarker>: Sealed {
    fn join_typed(&self, sub_path: impl AsRef<Path>) -> AtraPathBuf<B>;
}

macro_rules! join_mapping {
    ($a: ident => $b: ident) => {
        impl TypedJoin<$a, $b> for AtraPathBuf<$a> {
            #[inline] fn join_typed(&self, sub_path: impl AsRef<Path>) -> AtraPathBuf<$b> {
                AtraPathBuf::new(self.join(sub_path))
            }
        }
    };

    ($a: ident => $b: ident with fn $name: ident ($($t:tt)+) $body: block) => {
        impl<T: PathKindMarker> AtraPathBuf<T> {
            pub fn $name ($($t)+) -> AtraPathBuf<$b> $body
        }
    };
}

join_mapping! {
    DataFile => DecodedDataFile with fn join_to_decode(&self, encoding: &'static Encoding) {
        let mut copy = self.to_path_buf();
        let mut name = copy.file_name().expect("A DataFile always has a name!").to_string();
        name.push_str("_decoded_");
        name.push_str(encoding.name());
        copy.set_file_name(name);
        return AtraPathBuf::new(copy)
    }
}
