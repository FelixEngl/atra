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

use std::num::NonZeroUsize;
use deranged::RangedU64;
use ubyte::ByteUnit;

/// The maximum entry size of a DB entry in bytes
pub type MaxEntrySize = RangedU64<{ByteUnit::Megabyte(10).as_u64()}, {ByteUnit::Gigabyte(3).as_u64()}>;

/// The default cache size for the robots cache
pub const DEFAULT_CACHE_SIZE_ROBOTS: NonZeroUsize = unsafe{NonZeroUsize::new_unchecked(32)};

/// The default size of a fetched side that can be stored in memory (in byte)
pub const DEFAULT_MAX_SIZE_IN_MEMORY_DOWNLOAD: u64 =
    ByteUnit::Megabyte(100).as_u64();



#[allow(private_bounds)] trait Seal{}

/// A sealed trait that allows an easy conversion to the byte units
#[allow(private_bounds)]
pub trait MaxEntrySizeExt: Seal {
    fn as_byte_unit(&self) -> ByteUnit;
    fn to_byte_unit(self) -> ByteUnit;
}

impl Seal for MaxEntrySize{}
#[allow(private_bounds)]
impl MaxEntrySizeExt for MaxEntrySize {
    fn as_byte_unit(&self) -> ByteUnit {
        ByteUnit::Byte(self.get())
    }

    fn to_byte_unit(self) -> ByteUnit {
        ByteUnit::Byte(self.get())
    }
}