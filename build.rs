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

use std::collections::{HashMap};
use std::env;
use std::path::Path;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use isolang::Language;
use tinyjson::JsonValue;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    generate_stop_word_lists();
}

fn generate_stop_word_lists() {
    let mut data = File::options().read(true).open("./external/stopwords/iso/stopwords-iso.json").unwrap();
    let mut content = String::new();
    data.read_to_string(&mut content).unwrap();
    drop(data);
    let parsed: JsonValue = content.parse().unwrap();
    let object: &HashMap<_, _> = parsed.get().unwrap();
    build_stop_word_library(object);
}

fn build_stop_word_library(object: &HashMap<String, JsonValue>) {


    let mut containers: HashMap<Language, Vec<String>> = HashMap::new();

    let base = Path::new("./data/stopwords/iso");
    if !base.exists() {
        std::fs::create_dir_all(base).unwrap();
    }
    println!("crate: {}", std::fs::canonicalize(base).unwrap().to_str().unwrap());
    for (k, v) in object.iter() {
        let lang = isolang::Language::from_639_1(k.as_str()).expect(format!("Why is {k} not an iso language?").as_str());
        let values: Vec<_> = v.get::<Vec<_>>().unwrap().iter().map(|value| value.get::<String>().unwrap().to_string()).collect();
        let mut file = BufWriter::new(File::options().write(true).truncate(true).create(true).open(base.join(format!("{}.txt", lang.to_639_1().unwrap()))).unwrap());
        for v in &values {
            writeln!(&mut file, "{v}").unwrap();
        }
        containers.insert(lang, values.into_iter().map(|value| value.replace("\"", "\\\"")).collect());
    }

    let mut new = phf_codegen::Map::new();
    // new.phf_path("phf");
    for (k, v) in containers {
        new.entry(format!("{}", k.to_639_3()), format!("&[\"{}\"]", v.join("\", \"")).as_str());
    }


    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("default_stopwords.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());
    writeln!(
        &mut file,
        "static DEFAULT_STOPWORDS: {} = \n{};\n",
        "phf::Map<&'static str, &'static [&'static str]>",
        new.build()
    ).unwrap();



//     writeln!(&mut file, "
//
// /// Retrieves the default stopwords for a provided [lang] in iso3 format.
// #[cfg(feature = \"tokenizing\")]
// pub fn get_default_stopwords_for(lang: &str) -> Option<&'static [&'static str]>{{
//     DEFAULT_STOPWORDS.get(&lang.to_lowercase())
// }}").unwrap();
//     writeln!(&mut file, "
//
// /// Retrieves the default stopwords for a provided [lang].
// #[cfg(feature = \"tokenizing\")]
// pub fn get_default_stopwords_for_lang(lang: &isolang::Language) -> Option<&'static [&'static str]>{{
//     get_default_stopwords_for(lang.to_639_3())
// }}").unwrap();

}