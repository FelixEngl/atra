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

mod db;
mod errors;
mod kind;
mod manager;
mod state;
mod traits;

pub use db::*;
pub use errors::*;
pub use kind::LinkStateKind;
pub use manager::DatabaseLinkStateManager;
pub use state::LinkState;
pub use traits::*;

#[cfg(test)]
mod test {
    use super::*;
    use crate::database::{destroy_db, open_db};
    use crate::link_state::kind::LinkStateKind;
    use crate::link_state::state::LinkState;
    use crate::url::{Depth, UrlWithDepth};
    use scopeguard::defer;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn ser_and_deser_work() {
        let new = LinkState::with_payload(
            LinkStateKind::Crawled,
            LinkStateKind::Crawled,
            OffsetDateTime::now_utc().replace_nanosecond(0).unwrap(),
            Depth::ZERO + (1, 2, 3),
            vec![1, 2, 3, 4, 5],
        );

        let x = new.as_db_entry();

        let deser = LinkState::from_db_entry(&x).unwrap();

        assert_eq!(new, deser)
    }

    #[test]
    fn can_initialize() {
        defer! {let  _ = destroy_db("test.db1");}

        let db = Arc::new(open_db("test.db1").unwrap());
        let db = LinkStateRockDB::new(db);

        db.set_state(
            &UrlWithDepth::from_seed("https://google.de").unwrap(),
            &LinkState::without_payload(
                LinkStateKind::Discovered,
                LinkStateKind::Discovered,
                OffsetDateTime::now_utc(),
                Depth::ZERO,
            ),
        )
        .unwrap();

        db.set_state(
            &UrlWithDepth::from_seed("https://amazon.de").unwrap(),
            &LinkState::without_payload(
                LinkStateKind::Crawled,
                LinkStateKind::Discovered,
                OffsetDateTime::now_utc(),
                Depth::ZERO,
            ),
        )
        .unwrap();

        db.upsert_state(
            &UrlWithDepth::from_seed("https://google.de").unwrap(),
            &LinkState::without_payload(
                LinkStateKind::InternalError,
                LinkStateKind::Discovered,
                OffsetDateTime::now_utc(),
                Depth::ZERO,
            ),
        )
        .unwrap();

        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
                .unwrap()
        );
        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn can_initialize_weak() {
        defer! {let  _ = destroy_db("test.db2");}

        let db = Arc::new(open_db("test.db2").unwrap());
        let db = LinkStateRockDB::new(db);

        {
            let db = db.weak();

            db.set_state(
                &UrlWithDepth::from_seed("https://amazon.de").unwrap(),
                &LinkState::without_payload(
                    LinkStateKind::Discovered,
                    LinkStateKind::Discovered,
                    OffsetDateTime::now_utc(),
                    Depth::ZERO,
                ),
            )
            .unwrap();

            db.set_state(
                &UrlWithDepth::from_seed("https://google.de").unwrap(),
                &LinkState::without_payload(
                    LinkStateKind::Crawled,
                    LinkStateKind::Discovered,
                    OffsetDateTime::now_utc(),
                    Depth::ZERO,
                ),
            )
            .unwrap();
        }

        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
                .unwrap()
        );
        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn can_upset_properly() {
        defer! {let  _ = destroy_db("test.db3");}

        let db = Arc::new(open_db("test.db3").unwrap());

        let db = LinkStateRockDB::new(db);

        {
            let db = db.weak();

            db.update_state(
                &UrlWithDepth::from_seed("https://amazon.de").unwrap(),
                LinkStateKind::Discovered,
            )
            .unwrap();

            db.update_state(
                &UrlWithDepth::from_seed("https://google.de").unwrap(),
                LinkStateKind::Discovered,
            )
            .unwrap();

            db.update_state(
                &UrlWithDepth::from_seed("https://google.de").unwrap(),
                LinkStateKind::Crawled,
            )
            .unwrap();

            println!(
                "Amazon: {:?}",
                db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
            );
            println!(
                "Google: {:?}",
                db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
            );
        }

        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
                .unwrap()
        );
        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
                .unwrap()
        );
    }
}
