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

// use std::fmt::{Display};
use console::Term;
// use dialoguer::{Select, theme};
use rocksdb::IteratorMode;
use crate::contexts::local::LocalContext;
use crate::contexts::traits::{SupportsLinkState, SupportsUrlQueue};
use crate::crawl::{SlimCrawlResult, StoredDataHint};
use crate::link_state::{LinkStateLike, LinkStateManager};
use crate::url::AtraUri;
use crate::warc_ext::{ WarcSkipInstruction};
// use strum::{Display, VariantArray};
// #[derive(Debug, Display, VariantArray)]
// enum Targets {
//     #[strum(to_string = "See the stats")]
//     Stats,
//     #[strum(to_string = "See some entries")]
//     Entries,
//     #[strum(to_string = "Quit")]
//     Quit
// }
//
//
// #[derive(Debug)]
// struct SelectableEntry(
//     AtraUri,
//     SlimCrawlResult
// );
//
// impl Display for SelectableEntry {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         Display::fmt(&self.0, f)
//     }
// }
//
// #[derive(Debug, Display, VariantArray)]
// enum Enty {
//     #[strum(to_string = "{0.0}")]
//     Select(SelectableEntry),
//     Next,
//     Previous,
//     Quit
// }

pub fn view(local: LocalContext, internals: bool, extracted_links: bool, headers: bool, force_legacy: bool) {
    if !console::user_attended() || force_legacy  {
        println!("Not a user attended terminal. Falling back to legacy.");
        view_legacy(local, internals, extracted_links, headers);
        return;
    }

    let term = Term::buffered_stdout();
    if term.is_term() {
        println!("Not a real terminal. Falling back to legacy.");
        view_legacy(local, internals, extracted_links, headers);
        return;
    }

    println!("Currently only legacy view is supported.");
    view_legacy(local, internals, extracted_links, headers);


    // loop {
    //     let selection = Select::with_theme(
    //         &theme::ColorfulTheme::default()
    //     ).with_prompt("What do you to do?")
    //         .default(0)
    //         .items(Targets::VARIANTS)
    //         .interact_on_opt(&term)
    //         .unwrap();
    //
    //     match selection {
    //         None => {
    //             break
    //         }
    //         Some(value) => {
    //             term.clear_screen().unwrap();
    //             match Targets::VARIANTS[value] {
    //                 Targets::Stats => {
    //                     term.write_line("##### ATRA STATS #####");
    //                     term.write_line(&format!("Links in Queue:        {}", local.url_queue().len_blocking())).unwrap();
    //                     term.write_line(&format!("Links in CrawlDB:      {}", local.crawl_db().len())).unwrap();
    //                     term.write_line(&format!("Links in StateManager: {}", local.get_link_state_manager().len())).unwrap();
    //                 }
    //                 Targets::Entries => {
    //                     let mut iter = local.crawl_db().iter(IteratorMode::Start).filter_map(
    //                         |value| value.ok()
    //                     ).map(|(k, v)| {
    //                         let k: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
    //                         let v: SlimCrawlResult = bincode::deserialize(v.as_ref()).unwrap();
    //                         (k, v)
    //                     });
    //
    //
    //
    //                     // loop {
    //                     //
    //                     //     let current =
    //                     //     Select::with_theme(&theme::ColorfulTheme::default())
    //                     //         .default(0)
    //                     //         .
    //                     // }
    //
    //
    //                 }
    //                 Targets::Quit => {
    //                     break
    //                 }
    //             }
    //         }
    //     }
    // }
}


fn view_legacy(local: LocalContext, internals: bool, extracted_links: bool, headers: bool) {
    println!("##### ATRA STATS #####");
    println!("    Links in Queue:        {}", local.url_queue().len_blocking());
    println!("    Links in CrawlDB:      {}", local.crawl_db().len());
    println!("    Links in StateManager: {}", local.get_link_state_manager().len());
    println!("##### ATRA STATS #####");

    println!("\n\nCrawled Websides:\n");
    println!("\n-----------------------\n");
    for (k, v) in local.crawl_db().iter(IteratorMode::Start).filter_map(
        |value| value.ok()
    ).map(|(k, v)| {
        let k: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
        let v: SlimCrawlResult = bincode::deserialize(v.as_ref()).unwrap();
        (k, v)
    }) {
        println!("{k}");
        println!("    Meta:");
        println!("        Status Code: {}", v.meta.status_code);
        if let Some(lang) = v.meta.language {
            println!("        Status Code: {} (confidence: {})", lang.lang().to_name(), lang.confidence());
        } else {
            println!("        Language: -!-", );
        }
        let file_info = v.meta.file_information;
        println!("        Atra Filetype: {}", file_info.format);
        if let Some(mime) = file_info.mime {
            for mime in mime.iter() {
                println!("            Mime: {}", mime);
            }
        }
        if let Some(detected) = file_info.detected {
            println!("            Detected File Format: {}", detected.most_probable_file_format());
        }

        println!("        Created At: {}", v.meta.created_at);

        if let Some(encoding) = v.meta.recognized_encoding {
            println!("        Encoding: {}", encoding.name());
        } else {
            println!("        Encoding: -!-");
        }

        let linkstate = local.get_link_state_manager().get_link_state_sync(&v.meta.url);
        if let Ok(Some(state)) = linkstate {
            println!("        Linkstate:");
            println!("            State: {}", state.kind());
            println!("            IsSeed: {}", state.is_seed());
            println!("            Timestamp: {}", state.timestamp());
            println!("            Recrawl: {}", state.recrawl());
            println!("            Depth: {}", state.depth());
        } else {
            println!("        Linkstate: -!-");
        }

        if let Some(redirect) = v.meta.final_redirect_destination {
            println!("        Redirect: {redirect}");
        }

        if headers {
            if let Some(headers) = v.meta.headers {
                if !headers.is_empty() {
                    println!("        Headers:");
                    for (k, v) in headers.iter() {
                        println!("            \"{}\": \"{}\"", k, String::from_utf8_lossy(v.as_bytes()).to_string());
                    }
                } else {
                    println!("        Headers: -!-");
                }
            } else {
                println!("        Headers: -!-");
            }
        }

        if extracted_links {
            if let Some(extracted_links) = v.meta.links {
                println!("        Extracted Links:");
                for (i, value) in extracted_links.iter().enumerate() {
                    println!("            {}: {}", i, value);
                }
            }
        }

        if internals {
            println!("    Internal Storage:");
            match v.stored_data_hint {
                StoredDataHint::External(ref value) => {
                    println!("        External: {} - {}", value.exists(), value);
                }
                StoredDataHint::Warc(ref value) => {
                    match value {
                        WarcSkipInstruction::Single { pointer, is_base64, header_signature_octet_count } => {
                            println!("        Single Warc: {} - {} ({}, {}, {:?})", pointer.path().exists(), pointer.path(), is_base64, header_signature_octet_count, pointer.pointer());
                        }
                        WarcSkipInstruction::Multiple { pointers, header_signature_octet_count, is_base64 } => {
                            println!("        Multiple Warc: ({}, {})", is_base64, header_signature_octet_count);
                            for pointer in pointers {
                                println!("            {} - {} ({}, {}, {:?})", pointer.path().exists(), pointer.path(), is_base64, header_signature_octet_count, pointer.pointer());
                            }
                        }
                    }
                }
                StoredDataHint::InMemory(ref value) => {
                    println!("        InMemory: {}", value.len());
                }
                StoredDataHint::Associated => {
                    println!("        Associated!")
                }
                StoredDataHint::None => {
                    println!("        None!")
                }
            }
        }

        println!("\n-----------------------\n");
    }
}