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

use crate::queue::{SupportsSeeding, UrlQueue};
use crate::seed::read_seeds;
use crate::url::UrlWithDepth;
use camino::Utf8PathBuf;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::char;
use nom::character::complete::multispace0;
use nom::combinator::{map, rest, verify};
use nom::error::ParseError;
use nom::multi::separated_list1;
use nom::sequence::{delimited, preceded};
use nom::IResult;
use nom::Parser;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::str::FromStr;

/// Defines what kind of seed is used
/// CLI Syntax:
/// ```text
/// - command... file: path to a file>
/// - command... single: url
/// - command... single: "url"
/// - command... multi: "url","url"....
/// - command... path to a file
/// - command... url
/// - command... "url"
/// - command... "url","url"....
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeedDefinition {
    Single(String),
    Multi(Vec<String>),
    File(Utf8PathBuf),
}

impl SeedDefinition {
    pub async fn fill_queue(&self, queue: &impl UrlQueue<UrlWithDepth>) {
        match self {
            SeedDefinition::File(path) => queue
                .enqueue_seeds(read_seeds(path).expect("Was not able to read file"))
                .await
                .expect("Can not write any kind of seeds to the queue!"),
            SeedDefinition::Single(entry) => queue
                .enqueue_seed(&entry)
                .await
                .expect("Can not write any kind of seeds to the queue!"),
            SeedDefinition::Multi(entries) => {
                for entry in entries {
                    queue
                        .enqueue_seed(&entry)
                        .await
                        .expect("Can not write any kind of seeds to the queue!")
                }
            }
        }
    }
}

fn parse(s: &str) -> IResult<&str, SeedDefinition> {
    /// Something surrounded by whitespaces
    fn ws<'a, O, E: ParseError<&'a str>, F: Parser<&'a str, O, E>>(
        f: F,
    ) -> impl Parser<&'a str, O, E> {
        delimited(multispace0, f, multispace0)
    }

    fn delimited_str(s: &str) -> IResult<&str, String> {
        map(
            ws(delimited(
                tag("\""),
                take_while1(|value| value != '"'),
                tag("\""),
            )),
            |s: &str| s.to_string(),
        )(s)
    }

    fn multi_list(s: &str) -> IResult<&str, SeedDefinition> {
        map(separated_list1(ws(char(',')), delimited_str), |values| {
            if values.len() == 1 {
                SeedDefinition::Single(values.into_iter().next().unwrap())
            } else {
                SeedDefinition::Multi(values)
            }
        })(s)
    }

    fn file_or_single(s: &str) -> IResult<&str, SeedDefinition> {
        map(
            verify(rest, |s: &str| !s.starts_with('"')),
            |value: &str| {
                if std::fs::metadata(value).is_ok() {
                    SeedDefinition::File(Utf8PathBuf::from(value))
                } else {
                    SeedDefinition::Single(value.to_string())
                }
            },
        )(s)
    }

    alt((
        preceded(
            ws(tag("file:")),
            map(
                alt((delimited_str, map(rest, |s: &str| s.to_string()))),
                |value| SeedDefinition::File(Utf8PathBuf::from(value)),
            ),
        ),
        preceded(
            ws(tag("single:")),
            map(
                alt((delimited_str, map(rest, |s: &str| s.to_string()))),
                |value| SeedDefinition::Single(value.to_string()),
            ),
        ),
        preceded(ws(tag("multi:")), multi_list),
        multi_list,
        file_or_single,
    ))(s)
}

impl FromStr for SeedDefinition {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(parse(s).expect("Failed to parse the seed definition!").1)
    }
}

#[cfg(test)]
mod test {
    use crate::seed::SeedDefinition;
    use camino::Utf8PathBuf;

    #[test]
    pub fn test() {
        assert_eq!(
            Ok(SeedDefinition::Multi(vec![
                "hello world".to_string(),
                "whats up".to_string()
            ])),
            "multi:\"hello world\", \"whats up\"".parse()
        );
        assert_eq!(
            Ok(SeedDefinition::Single("hello world".to_string())),
            "multi:\"hello world\"".parse()
        );
        assert_eq!(
            Ok(SeedDefinition::Single("whazzabeee.de".to_string())),
            "single:whazzabeee.de".parse()
        );
        assert_eq!(
            Ok(SeedDefinition::File(Utf8PathBuf::from(
                "./testdata/blacklist.txt"
            ))),
            "file:./testdata/blacklist.txt".parse()
        );

        assert_eq!(
            Ok(SeedDefinition::Multi(vec![
                "hello world".to_string(),
                "whats up".to_string()
            ])),
            "\"hello world\", \"whats up\"".parse()
        );
        assert_eq!(
            Ok(SeedDefinition::Single("hello world".to_string())),
            "\"hello world\"".parse()
        );
        assert_eq!(
            Ok(SeedDefinition::Single("whazzabeee.de".to_string())),
            "whazzabeee.de".parse()
        );
        assert_eq!(
            Ok(SeedDefinition::File(Utf8PathBuf::from(
                "./testdata/blacklist.txt"
            ))),
            "./testdata/blacklist.txt".parse()
        );
    }
}
