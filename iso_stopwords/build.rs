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

use isolang::Language;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use tinyjson::JsonValue;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    generate_stop_word_lists();
}

fn generate_stop_word_lists() {
    let mut data = File::options()
        .read(true)
        .open("./external/stopwords/iso/stopwords-iso.json")
        .unwrap();
    let mut content = String::new();
    data.read_to_string(&mut content).unwrap();
    drop(data);
    let parsed: JsonValue = content.parse().unwrap();
    let object: &HashMap<_, _> = parsed.get().unwrap();
    build_stop_word_library(object);
}

fn build_stop_word_library(object: &HashMap<String, JsonValue>) {
    let mut containers: HashMap<Language, Vec<String>> = HashMap::new();

    for (k, v) in object.iter() {
        let lang = Language::from_639_1(k.as_str())
            .expect(format!("Why is {k} not an iso language?").as_str());
        let values: Vec<_> = v
            .get::<Vec<_>>()
            .unwrap()
            .iter()
            .map(|value| value.get::<String>().unwrap().to_string())
            .collect();
        assert!(containers.insert(lang, values).is_none());
    }

    let path = Path::new("src/iso_stopwords.rs");
    let mut content = BufWriter::new(
        File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)
            .unwrap(),
    );
    writeln!(&mut content, "/** This file s autogenerated! */").unwrap();
    writeln!(&mut content, "use isolang;\n").unwrap();
    writeln!(&mut content, "/// Supplies default stopwords for a [lang].").unwrap();
    writeln!(
        &mut content,
        "pub fn iso_stopwords_for(lang: &isolang::Language) -> Option<&'static [&'static str]> {{"
    )
    .unwrap();
    writeln!(&mut content, "    match lang {{").unwrap();
    for k in containers.keys() {
        let c = k.to_639_3();
        let mut chars = c.chars();
        let name = match chars.next() {
            None => String::with_capacity(0),
            Some(c) => {
                let mut result = c.to_uppercase().to_string();
                result += chars.as_str();
                result
            }
        };
        writeln!(
            &mut content,
            "        isolang::Language::{} => Some(STOPWORDS_{}),",
            name,
            k.to_639_3().to_uppercase()
        )
        .unwrap();
    }
    writeln!(&mut content, "        _ => None,").unwrap();
    writeln!(&mut content, "    }}").unwrap();
    writeln!(&mut content, "}}").unwrap();

    for (k, v) in containers {
        writeln!(&mut content, "/// Stopwords for {}", k.to_name()).unwrap();
        writeln!(
            &mut content,
            "const STOPWORDS_{}: &'static [&'static str] = &[",
            k.to_639_3().to_uppercase()
        )
        .unwrap();
        for word in v {
            let word = match word.as_str() {
                "\"" => "\\\"",
                "\\" => "\\\\",
                value => value,
            };
            writeln!(&mut content, "    \"{}\",", word).unwrap();
        }
        writeln!(&mut content, "];\n").unwrap();
    }
}
