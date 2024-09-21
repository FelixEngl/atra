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

mod in_memory;
mod raw;
mod traits;

pub use in_memory::LinkState;
pub use raw::RawLinkState;
pub use traits::LinkStateLike;

#[cfg(test)]
mod test {
    use crate::link_state::{
        IsSeedYesNo, LinkState, LinkStateKind, LinkStateLike, RawLinkState, RecrawlYesNo,
    };
    use crate::url::Depth;
    use time::{Duration, OffsetDateTime};

    #[test]
    fn can_serialize_and_deserialize() {
        let test = LinkState::new(
            LinkStateKind::Crawled,
            LinkStateKind::Discovered,
            RecrawlYesNo::Yes,
            IsSeedYesNo::Yes,
            OffsetDateTime::now_utc(),
            Depth::new(1, 2, 3),
            None,
        );

        let serialized = test.as_bytes();

        assert_eq!(RawLinkState::IDEAL_SIZE, serialized.len());

        let deserialized = unsafe { RawLinkState::from_slice_unchecked(&serialized) };

        assert_eq!(test, deserialized);
    }

    #[test]
    fn can_serialize_and_deserialize_payload() {
        let test = LinkState::new(
            LinkStateKind::Crawled,
            LinkStateKind::Discovered,
            RecrawlYesNo::Yes,
            IsSeedYesNo::Yes,
            OffsetDateTime::now_utc(),
            Depth::new(1, 2, 3),
            Some(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
        );

        let serialized = test.as_raw_link_state();

        assert_eq!(RawLinkState::IDEAL_SIZE + 11, serialized.len());

        let deserialized = serialized.as_link_state().into_owned();

        assert_eq!(test, deserialized);
    }

    #[test]
    fn can_upsert() {
        let stored = LinkState::new(
            LinkStateKind::Crawled,
            LinkStateKind::Discovered,
            RecrawlYesNo::No,
            IsSeedYesNo::Yes,
            OffsetDateTime::now_utc(),
            Depth::new(1, 2, 3),
            None,
        );

        let stored_db_entry = stored.as_raw_link_state().into_owned();

        let ts = OffsetDateTime::now_utc() + Duration::weeks(1);

        let to_upsert = LinkState::new(
            LinkStateKind::ProcessedAndStored,
            LinkStateKind::Unset,
            RecrawlYesNo::Yes,
            IsSeedYesNo::Yes,
            ts,
            Depth::new(2, 2, 3),
            Some(vec![1, 2, 3, 4, 5]),
        );

        let result = RawLinkState::merge_linkstate_simulated(
            &[1, 2, 3, 4, 5],
            Some(&stored_db_entry),
            &[to_upsert.as_raw_link_state().into_owned()],
        )
        .expect("We need an update!");

        let upsert_result = unsafe { RawLinkState::from_vec_unchecked(result) };

        let expected = LinkState::new(
            LinkStateKind::ProcessedAndStored,
            LinkStateKind::Crawled,
            RecrawlYesNo::Yes,
            IsSeedYesNo::Yes,
            ts,
            Depth::new(2, 2, 3),
            Some(vec![1, 2, 3, 4, 5]),
        );

        assert_eq!(expected, upsert_result);
        println!("{:?}", expected);
        println!("{:?}", upsert_result.as_link_state());
    }
}
