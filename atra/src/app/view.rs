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

mod db_view;

use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use camino::Utf8PathBuf;
use console::{style, Term};
use dialoguer::{Select, theme};
use itertools::{Either, Itertools};
use crate::contexts::local::LocalContext;
use crate::contexts::traits::{SupportsLinkState, SupportsUrlQueue};
use crate::crawl::{SlimCrawlResult, StoredDataHint};
use crate::link_state::{LinkStateLike, LinkStateManager};
use crate::url::AtraUri;
use crate::warc_ext::WarcSkipInstruction;
use rocksdb::{Error, IteratorMode};
use strum::{Display, VariantArray};
use time::OffsetDateTime;
use crate::app::view::db_view::{ControlledIterator, SlimEntry};
use crate::data::RawVecData;
use crate::format::supported::InterpretedProcessibleFileFormat;

#[derive(Debug, Display, VariantArray)]
enum Targets {
    #[strum(to_string = "See the stats.")]
    Stats,
    #[strum(to_string = "See some entries.")]
    Entries,
    #[strum(to_string = "Quit")]
    Quit
}


#[derive(Debug)]
struct SelectableEntry(
    usize,
    SlimEntry
);

impl Display for SelectableEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}: {}", self.0, self.1.0.as_ref().0.as_str()).as_str())
    }
}

#[derive(Debug, Display)]
enum SelectDialougeEntry {
    #[strum(to_string = "{0}")]
    Select(SelectableEntry),
    Next,
    Previous,
    Quit
}

pub fn view(
    local: LocalContext,
    internals: bool,
    extracted_links: bool,
    headers: bool,
    force_legacy: bool,
) {
    if !console::user_attended() || force_legacy {
        println!("Not a user attended terminal. Falling back to legacy.");
        view_legacy(local, internals, extracted_links, headers);
        return;
    }

    let term = Term::buffered_stdout();
    term.set_title("Atra Viewer");
    if !term.is_term() {
        println!("Not a real terminal. Falling back to legacy.");
        view_legacy(local, internals, extracted_links, headers);
        return;
    }

    // println!("Currently only legacy view is supported.");
    // view_legacy(local, internals, extracted_links, headers);

    fn print_stats(term: &Term, local: &LocalContext) {
        term.write_line("##### ATRA STATS #####").unwrap();
        term.write_line(&format!("Links in Queue:        {}", local.url_queue().len_blocking())).unwrap();
        term.write_line(&format!("Links in CrawlDB:      {}", local.crawl_db().len())).unwrap();
        term.write_line(&format!("Links in StateManager: {}", local.get_link_state_manager().len())).unwrap();
        term.write_line("Press Enter to continue...").unwrap();
        term.flush().unwrap();
        term.read_line().unwrap();
        term.clear_last_lines(6).unwrap()
    }

    #[inline(always)]
    fn retrieve_selection(local: &LocalContext, mode: IteratorMode, n: usize) -> Vec<Result<(AtraUri, SlimCrawlResult), Error>> {
        local.crawl_db()
            .iter(mode)
            .take(n)
            .map_ok(|(k, v)| {
                let k: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
                let v: SlimCrawlResult = bincode::deserialize(v.as_ref()).unwrap();
                (k, v)
            })
            .collect_vec()
    }

    fn create_select_key(value: &Result<(AtraUri, SlimCrawlResult), Error>) -> String {
        match value {
            Ok((url, _)) => {
                url.to_string()
            }
            Err(err) => {
                err.to_string().split(':').next().unwrap_or("").to_string()
            }
        }
    }

    loop {
        let selection = Select::with_theme(
            &theme::ColorfulTheme::default()
        ).with_prompt("What do you to want do?")
            .default(0)
            .clear(true)
            .report(false)
            .items(Targets::VARIANTS)
            .interact_on_opt(&term)
            .unwrap();

        match selection {
            None => {
                break
            }
            Some(value) => {
                match Targets::VARIANTS[value] {
                    Targets::Stats => print_stats(&term, &local),
                    Targets::Entries => {
                        match ControlledIterator::new(&local, 10) {
                            Ok(mut iter) => {
                                fn provide_dialouge(iter: &ControlledIterator, dialouge: &mut Vec<SelectDialougeEntry>) {
                                    dialouge.extend(
                                        iter.current().iter().enumerate().map(
                                            |(idx, value)| {
                                                SelectDialougeEntry::Select(SelectableEntry(idx, value.clone()))
                                            }
                                        )
                                    );
                                    dialouge.push(SelectDialougeEntry::Previous);
                                    dialouge.push(SelectDialougeEntry::Next);
                                    dialouge.push(SelectDialougeEntry::Quit);
                                }

                                let mut col = Vec::with_capacity(iter.selection_size() + 3);
                                provide_dialouge(&iter, &mut col);

                                loop {
                                    let selected = Select::with_theme(&theme::ColorfulTheme::default())
                                        .with_prompt("Select a target:")
                                        .default(0)
                                        .clear(true)
                                        .report(false)
                                        .items(col.as_slice())
                                        .interact_on_opt(&term)
                                        .unwrap();
                                    match selected {
                                        None => {
                                            term.write_line("You have to select something! (press any key to continue)").unwrap();
                                            term.write_line("Press Enter to continue...").unwrap();
                                            term.flush().unwrap();
                                            term.read_line().unwrap();
                                            term.clear_line().unwrap()
                                        }
                                        Some(idx) => {
                                            match col.get(idx).unwrap() {
                                                SelectDialougeEntry::Select(entry) => {
                                                    let to_view = iter.select(entry.0).unwrap();
                                                    match to_view {
                                                        None => {
                                                            term.write_line("Nothing to see... (press any key to continue)").unwrap();
                                                            term.write_line("Press Enter to continue...").unwrap();
                                                            term.flush().unwrap();
                                                            term.read_line().unwrap();
                                                        }
                                                        Some((_, uri, target)) => {
                                                            entry_dialouge(&term, uri, target, &local);
                                                        }
                                                    }
                                                }
                                                SelectDialougeEntry::Next => {
                                                    col.clear();
                                                    iter.next().unwrap();
                                                    provide_dialouge(&iter, &mut col);
                                                }
                                                SelectDialougeEntry::Previous => {
                                                    col.clear();
                                                    iter.previous().unwrap();
                                                    provide_dialouge(&iter, &mut col);
                                                }
                                                SelectDialougeEntry::Quit => {
                                                    term.clear_screen().unwrap();
                                                    break
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(value) => {
                                term.write_line(style("Failed to read entries:").red().to_string().as_str()).unwrap();
                                for value in value.into_iter() {
                                    term.write_line(style(value.to_string()).red().to_string().as_str()).unwrap();
                                }
                                break
                            }
                        }

                    }
                    Targets::Quit => {
                        break
                    }
                }
            }
        }
    }
}


#[derive(Copy, Clone, VariantArray, Display)]
enum EntryDialougeMode {
    Return,
    Export,
    OutgoingLinks,
    Headers,
    Internals,
}

fn entry_dialouge(term: &Term, uri: &AtraUri, v: &SlimCrawlResult, context: &LocalContext) {
    term.clear_screen().unwrap();
    term.write_line(format!("View of: {}", uri).as_str()).unwrap();

    term.write_line(format!("    Status Code: {}", v.meta.status_code).as_str()).unwrap();
    let lang_info = if let Some(lang) = v.meta.language {
        Cow::Owned(
            format!(
                "    Status Code: {} (confidence: {})",
                lang.lang().to_name(),
                lang.confidence()
            )
        )
    } else {
        Cow::Borrowed("    Language: -!-")
    };
    term.write_line(lang_info.as_ref()).unwrap();
    let file_info = &v.meta.file_information;
    term.write_line(format!("    Atra Filetype: {}", file_info.format).as_str()).unwrap();
    if let Some(ref mime) = file_info.mime {
        for mime in mime.iter() {
            term.write_line(format!("        Mime: {}", mime).as_str()).unwrap();
        }
    }
    if let Some(ref detected) = file_info.detected {
        term.write_line(format!("        Detected File Format: {}", detected.most_probable_file_format()).as_str()).unwrap();
    }
    term.write_line(format!("    Created At: {}", v.meta.created_at).as_str()).unwrap();
    let enc = if let Some(encoding) = v.meta.recognized_encoding {
        Cow::Owned(format!("    Encoding: {}", encoding.name()))
    } else {
        Cow::Borrowed("    Encoding: -!-")
    };
    term.write_line(enc.as_ref()).unwrap();
    let linkstate = context
        .get_link_state_manager()
        .get_link_state_sync(&v.meta.url);
    if let Ok(Some(state)) = linkstate {
        term.write_line(format!("    Linkstate:").as_str()).unwrap();
        term.write_line(format!("        State: {}", state.kind()).as_str()).unwrap();
        term.write_line(format!("        IsSeed: {}", state.is_seed()).as_str()).unwrap();
        term.write_line(format!("        Timestamp: {}", state.timestamp()).as_str()).unwrap();
        term.write_line(format!("        Recrawl: {}", state.recrawl()).as_str()).unwrap();
        term.write_line(format!("        Depth: {}", state.depth()).as_str()).unwrap();
    } else {
        println!("    Linkstate: -!-");
    }

    if let Some(ref redirect) = v.meta.final_redirect_destination {
        term.write_line(format!("        Redirect: {redirect}").as_str()).unwrap();
    }
    term.flush().unwrap();
    let mut d = DeletingTerm::new(term);
    loop {
        let selection = Select::with_theme(&theme::ColorfulTheme::default())
            .with_prompt("What to do?")
            .default(0)
            .report(false)
            .items(EntryDialougeMode::VARIANTS)
            .clear(true)
            .interact_on(&term)
            .unwrap();
        d.clear().unwrap();

        match EntryDialougeMode::VARIANTS[selection] {
            EntryDialougeMode::Return => {
                break
            }
            EntryDialougeMode::Export => {
                let retrieved = unsafe{v.get_content().expect("Failed to retrieve the data!")};
                let file_name = v.meta.url.url.file_name();
                let file_name = if let Some(file_name) = file_name {
                    file_name
                } else {
                    Cow::Owned(format!("./exported_file_{}", OffsetDateTime::now_utc().unix_timestamp().to_string()))
                };

                let file_name = match &v.meta.file_information.format {
                    InterpretedProcessibleFileFormat::HTML => {
                        if !file_name.as_ref().ends_with(".html") {
                            Cow::Owned(format!("{}.html", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::PDF => {
                        if !file_name.as_ref().ends_with(".pdf") {
                            Cow::Owned(format!("{}.pdf", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::JavaScript => {
                        if !file_name.as_ref().ends_with(".js") {
                            Cow::Owned(format!("{}.js", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::PlainText | InterpretedProcessibleFileFormat::StructuredPlainText => {
                        if !file_name.as_ref().ends_with(".txt") {
                            Cow::Owned(format!("{}.txt", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::JSON => {
                        if !file_name.as_ref().ends_with(".json") {
                            Cow::Owned(format!("{}.json", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::XML => {
                        if !file_name.as_ref().ends_with(".xml") {
                            Cow::Owned(format!("{}.xml", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::SVG => {
                        if !file_name.as_ref().ends_with(".svg") {
                            Cow::Owned(format!("{}.svg", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::RTF => {
                        if !file_name.as_ref().ends_with(".rtf") {
                            Cow::Owned(format!("{}.rtf", file_name))
                        } else {
                            file_name
                        }
                    }
                    InterpretedProcessibleFileFormat::ZIP => {
                        if !file_name.as_ref().ends_with(".zip") {
                            Cow::Owned(format!("{}.zip", file_name))
                        } else {
                            file_name
                        }
                    }
                    _ => file_name
                };
                let mut path = Utf8PathBuf::from(".");
                path.set_file_name(file_name.as_ref());
                let mut ct = 1;
                while path.exists() {
                    match path.file_name() {
                        None => {
                            path.set_file_name(
                                format!("exported_file_{}", OffsetDateTime::now_utc().unix_timestamp().to_string())
                            )
                        }
                        Some(_) => {
                            match file_name.split_once(".") {
                                None => {
                                    path.set_file_name(
                                        format!("{} ({})", file_name, ct)
                                    );
                                    ct+=1;
                                }
                                Some((a, b)) => {
                                    path.set_file_name(
                                        format!("{} ({}).{}", a, ct, b)
                                    );
                                    ct+=1;
                                }
                            }
                        }
                    }
                }
                match retrieved {
                    Either::Left(value) => {
                        match value {
                            RawVecData::None => {
                                d.write_line("Nothing to export!").unwrap();
                            }
                            RawVecData::InMemory { data } => {
                                match File::options().write(true).create_new(true).open(&path) {
                                    Ok(file) => {
                                        match BufWriter::new(file).write_all(data.as_ref()) {
                                            Ok(_) => {
                                                d.write_line(format!("Exported to {}", &path).as_str()).unwrap()
                                            }
                                            Err(err) => {d.write_line(format!("Error: {}", err).as_str()).unwrap();}
                                        }
                                    }
                                    Err(value) => {
                                        d.write_line(format!("Error: {}", value).as_str()).unwrap();
                                    }
                                }
                            }
                            RawVecData::ExternalFile { path: s_path } => {
                                match std::fs::copy(s_path, &path) {
                                    Ok(_) => {
                                        d.write_line(format!("Exported to {}", &path).as_str()).unwrap()
                                    }
                                    Err(value) => {
                                        d.write_line(format!("Error: {}", value).as_str()).unwrap();
                                    }
                                }
                            }
                        }
                    }
                    Either::Right(value) => {
                        match File::options().write(true).create_new(true).open(&path) {
                            Ok(file) => {
                                match BufWriter::new(file).write_all(value) {
                                    Ok(_) => {
                                        d.write_line(format!("Exported to {}", &path).as_str()).unwrap()
                                    }
                                    Err(err) => {d.write_line(format!("Error: {}", err).as_str()).unwrap();}
                                }
                            }
                            Err(value) => {
                                d.write_line(format!("Error: {}", value).as_str()).unwrap();
                            }
                        }
                    }
                }
            }
            EntryDialougeMode::OutgoingLinks => {
                if let Some(ref extracted_links) = v.meta.links {
                    d.write_line("    Extracted Links:").unwrap();
                    for (i, value) in extracted_links.iter().enumerate() {
                        d.write_line(format!("        {}: {}", i, value).as_str()).unwrap();
                    }
                } else {
                    d.write_line("    Extracted Links: -!-").unwrap()
                }
            }
            EntryDialougeMode::Headers => {
                if let Some(ref headers) = v.meta.headers {
                    if !headers.is_empty() {
                        d.write_line("    Headers:").unwrap();
                        for (k, v) in headers.iter() {
                            d.write_line(
                                format!(
                                    "        \"{}\": \"{}\"",
                                    k,
                                    String::from_utf8_lossy(v.as_bytes()).to_string()
                                ).as_str()
                            ).unwrap();
                        }
                    } else {
                        d.write_line("    Headers: -!-").unwrap();
                    }
                } else {
                    d.write_line("    Headers: -!-").unwrap();
                }
            }
            EntryDialougeMode::Internals => {
                d.write_line("    Internal Storage:").unwrap();
                match v.stored_data_hint {
                    StoredDataHint::External(ref value) => {
                        d.write_line(format!("        External: {} - {}", value.exists(), value).as_str()).unwrap();
                    }
                    StoredDataHint::Warc(ref value) => match value {
                        WarcSkipInstruction::Single {
                            pointer,
                            kind,
                            header_signature_octet_count,
                        } => {
                            d.write_line(format!(
                                "        Single Warc: {} - {} ({}, {}, {:?})",
                                pointer.path().exists(),
                                pointer.path(),
                                kind,
                                header_signature_octet_count,
                                pointer.pointer()
                            ).as_str()).unwrap();
                        }
                        WarcSkipInstruction::Multiple {
                            pointers,
                            header_signature_octet_count,
                            is_base64,
                        } => {
                            d.write_line(format!(
                                "        Multiple Warc: ({}, {})",
                                is_base64, header_signature_octet_count
                            ).as_str()).unwrap();
                            for pointer in pointers {
                                d.write_line(format!(
                                    "            {} - {} ({}, {}, {:?})",
                                    pointer.path().exists(),
                                    pointer.path(),
                                    is_base64,
                                    header_signature_octet_count,
                                    pointer.pointer()
                                ).as_str()).unwrap();
                            }
                        }
                    },
                    StoredDataHint::InMemory(ref value) => {
                        d.write_line(format!("        InMemory: {}", value.len()).as_str()).unwrap();
                    }
                    StoredDataHint::None => {
                        d.write_line("        None!").unwrap()
                    }
                }
            }
        }
        d.write_line("Press enter to continue...").unwrap();
        d.flush().unwrap();
        d.read_line().unwrap();
    }
}


struct DeletingTerm<'a> {
    term: &'a Term,
    ct: usize
}

impl<'a> DeletingTerm<'a> {
    pub fn new(term: &'a Term) -> Self {
        Self { term, ct: 0 }
    }

    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.ct += line.chars().filter(|value| '\n'.eq(value)).count() + 1;
        self.term.write_line(line)
    }

    pub fn read_line(&self) -> io::Result<String> {
        // self.ct += 1;
        self.term.read_line()
    }

    pub fn flush(&self) -> io::Result<()> {
        self.term.flush()
    }

    pub fn clear(&mut self) -> io::Result<()> {
        if self.ct == 0 {
            return Ok(())
        }
        self.term.clear_last_lines(self.ct)?;
        self.ct = 0;
        Ok(())
    }
}

impl<'a> Drop for DeletingTerm<'a>  {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

fn view_legacy(local: LocalContext, internals: bool, extracted_links: bool, headers: bool) {
    println!("##### ATRA STATS #####");
    println!(
        "    Links in Queue:        {}",
        local.url_queue().len_blocking()
    );
    println!("    Links in CrawlDB:      {}", local.crawl_db().len());
    println!(
        "    Links in StateManager: {}",
        local.get_link_state_manager().len()
    );
    println!("##### ATRA STATS #####");

    println!("\n\nCrawled Websides:\n");
    println!("\n-----------------------\n");
    for (k, v) in local
        .crawl_db()
        .iter(IteratorMode::Start)
        .filter_map(|value| value.ok())
        .map(|(k, v)| {
            let k: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
            let v: SlimCrawlResult = bincode::deserialize(v.as_ref()).unwrap();
            (k, v)
        })
    {
        println!("{k}");
        println!("    Meta:");
        println!("        Status Code: {}", v.meta.status_code);
        if let Some(lang) = v.meta.language {
            println!(
                "        Status Code: {} (confidence: {})",
                lang.lang().to_name(),
                lang.confidence()
            );
        } else {
            println!("        Language: -!-",);
        }
        let file_info = v.meta.file_information;
        println!("        Atra Filetype: {}", file_info.format);
        if let Some(mime) = file_info.mime {
            for mime in mime.iter() {
                println!("            Mime: {}", mime);
            }
        }
        if let Some(detected) = file_info.detected {
            println!(
                "            Detected File Format: {}",
                detected.most_probable_file_format()
            );
        }

        println!("        Created At: {}", v.meta.created_at);

        if let Some(encoding) = v.meta.recognized_encoding {
            println!("        Encoding: {}", encoding.name());
        } else {
            println!("        Encoding: -!-");
        }

        let linkstate = local
            .get_link_state_manager()
            .get_link_state_sync(&v.meta.url);
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
                        println!(
                            "            \"{}\": \"{}\"",
                            k,
                            String::from_utf8_lossy(v.as_bytes()).to_string()
                        );
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
                StoredDataHint::Warc(ref value) => match value {
                    WarcSkipInstruction::Single {
                        pointer,
                        kind,
                        header_signature_octet_count,
                    } => {
                        println!(
                            "        Single Warc: {} - {} ({}, {}, {:?})",
                            pointer.path().exists(),
                            pointer.path(),
                            kind,
                            header_signature_octet_count,
                            pointer.pointer()
                        );
                    }
                    WarcSkipInstruction::Multiple {
                        pointers,
                        header_signature_octet_count,
                        is_base64,
                    } => {
                        println!(
                            "        Multiple Warc: ({}, {})",
                            is_base64, header_signature_octet_count
                        );
                        for pointer in pointers {
                            println!(
                                "            {} - {} ({}, {}, {:?})",
                                pointer.path().exists(),
                                pointer.path(),
                                is_base64,
                                header_signature_octet_count,
                                pointer.pointer()
                            );
                        }
                    }
                },
                StoredDataHint::InMemory(ref value) => {
                    println!("        InMemory: {}", value.len());
                }
                StoredDataHint::None => {
                    println!("        None!")
                }
            }
        }

        println!("\n-----------------------\n");
    }
}
