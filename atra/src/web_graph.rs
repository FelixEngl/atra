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

use crate::runtime::{AtraHandleOption, RuntimeContext};
use crate::seed::BasicSeed;
use crate::url::{AtraOriginProvider, AtraUri, AtraUrlOrigin, UrlWithDepth};
use data_encoding::BASE32_NOPAD;
use itertools::Itertools;
use std::ffi::OsString;
use std::fmt::Write as FmtWrite;
use std::fs::File as StdFile;
use std::io;
use std::io::{BufRead, BufReader as StdBufReader, ErrorKind, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::Sender;
use ubyte::ByteUnit;

/// The entry of a webgraph.
#[derive(Debug)]
pub enum WebGraphEntry {
    /// A seed
    Seed {
        origin: AtraUrlOrigin,
        seed: AtraUri,
    },
    /// A normal link
    Link { from: AtraUri, to: AtraUri },
}

impl WebGraphEntry {
    #[inline]
    pub fn create_link(from: &UrlWithDepth, to: &UrlWithDepth) -> Self {
        Self::Link {
            from: from.url.clone(),
            to: to.url.clone(),
        }
    }

    #[inline]
    pub fn create_seed(seed: &impl BasicSeed) -> Self {
        Self::Seed {
            origin: seed.origin().to_owned(),
            seed: seed.url().url().clone(),
        }
    }

    /// A helper method for consuming lines.
    fn collect(&self, out: &mut impl EntryLineConsumer) {
        fn recognize_atra_uri(uri: &AtraUri, out: &mut impl EntryLineConsumer) -> String {
            let result = match uri.try_as_str() {
                None => {
                    let mut label = String::new();
                    label.write_str("ol:").unwrap();
                    BASE32_NOPAD.encode_append(uri.as_bytes(), &mut label);
                    out.push(format!("{label} rdfs:label \"{}\" .\n", uri.as_str()));
                    label
                }
                Some(value) => {
                    format!("<{value}>")
                }
            };
            if let Some(origin) = uri.atra_origin() {
                out.push(format!("{result} :has_origin o:{} .\n", origin));
            }
            result
        }

        match self {
            WebGraphEntry::Seed { seed, origin } => {
                let seed = recognize_atra_uri(seed, out);
                out.push(format!("o:{origin} :has_seed {seed} .\n"))
            }
            WebGraphEntry::Link { from, to } => {
                let from = recognize_atra_uri(from, out);
                let to = recognize_atra_uri(to, out);
                out.push(format!("{} :links_to {} .\n", from.as_str(), to.as_str()))
            }
        }
    }
}

/// A consumer for an entry line
trait EntryLineConsumer {
    fn push(&mut self, value: String);
}

impl EntryLineConsumer for String {
    fn push(&mut self, value: String) {
        self.write_str(&value).unwrap();
    }
}

impl EntryLineConsumer for Vec<String> {
    fn push(&mut self, value: String) {
        Vec::push(self, value)
    }
}

/// Error while working with the webgraph
#[derive(Debug, Error)]
pub enum WebGraphError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error("The file at {0:?} is not valid!")]
    InvalidFile(OsString),
    #[error("Failed to send an entry to the writer thread.")]
    SendError(WebGraphEntry),
}

/// Manages the webgraph
pub trait WebGraphManager {
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), WebGraphError>;
}

/// The default size for a links cache. Usually 10k links are cached.
pub const DEFAULT_CACHE_SIZE_WEB_GRAPH: NonZeroUsize =
    unsafe { NonZeroUsize::new_unchecked(20_000) };

/// A link net manager with a backing file.
#[derive(Debug)]
pub struct QueuingWebGraphManager {
    queue_in: Option<Sender<WebGraphEntry>>,
}

impl QueuingWebGraphManager {
    /// Creates the manager with a cache of the size [capacity] at the file [path].
    pub fn new(
        capacity: NonZeroUsize,
        path: impl AsRef<Path>,
        shutdown_and_handle: &RuntimeContext,
    ) -> Result<Self, WebGraphError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = StdFile::options()
            .write(true)
            .read(true)
            .append(true)
            .create(true)
            .open(&path)?;

        let meta = file.metadata()?;

        if !meta.is_file() {
            return Err(WebGraphError::IOError(io::Error::from(
                ErrorKind::Unsupported,
            )));
        }

        if meta.len() != 0 {
            let mut graph_prefix = false;
            let mut domain_prefix = false;
            let mut domain_label_prefix = false;
            let mut rnfs_prefix = false;
            for value in StdBufReader::new(&file).lines() {
                if let Ok(value) = value {
                    if value.starts_with("@prefix") {
                        graph_prefix = graph_prefix
                            || (value.contains(" : ") && value.contains("http://atra.de/graph#"));
                        domain_prefix = domain_prefix
                            || (value.contains(" o: ")
                                && value.contains("http://atra.de/graph/origin#"));
                        domain_label_prefix = domain_label_prefix
                            || (value.contains(" ol: ")
                                && value.contains("http://atra.de/graph/origin-label#"));
                        rnfs_prefix = rnfs_prefix
                            || (value.contains(" rdfs: ")
                                && value.contains("http://www.w3.org/2000/01/rdf-schema#"));
                    }
                }
                if graph_prefix && domain_prefix && domain_label_prefix && rnfs_prefix {
                    break;
                }
            }
            if !graph_prefix || !domain_prefix || !domain_label_prefix || !rnfs_prefix {
                log::error!(
                    "graph_prefix: {graph_prefix} domain_prefix: {domain_prefix} domain_label_prefix: {domain_label_prefix} rnfs_prefix: {rnfs_prefix}"
                );
                return Err(WebGraphError::InvalidFile(
                    path.as_ref().as_os_str().to_os_string(),
                ));
            }
        } else {
            writeln!(&mut file, "@prefix : <http://atra.de/graph#> .").unwrap();
            writeln!(&mut file, "@prefix o: <http://atra.de/graph/origin#> .").unwrap();
            writeln!(
                &mut file,
                "@prefix ol: <http://atra.de/graph/origin-label#> ."
            )
            .unwrap();
            writeln!(
                &mut file,
                "@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> ."
            )
            .unwrap();
        }

        let mut writer = BufWriter::with_capacity(
            ByteUnit::Kilobyte(32).as_u64() as usize,
            File::from_std(file),
        );

        if let Ok(handle) = shutdown_and_handle.handle().try_io_or_main_or_current() {
            log::debug!("Found Runtime. Setting up writer.");
            let (queue_in, mut queue_out) =
                tokio::sync::mpsc::channel::<WebGraphEntry>(capacity.get());
            let guard = shutdown_and_handle.shutdown_guard().guard();

            async fn write_buffer(entry: &mut Vec<String>, writer: &mut BufWriter<File>) {
                for value in entry.drain(..).unique() {
                    if let Err(err) = writer.write_all(value.as_bytes()).await {
                        log::error!("WebGraphWriter: encountered a problem:{err}")
                    }
                }
            }

            // todo: may need scaling
            handle.spawn(async move {
                let _guard = guard;
                log::debug!("WebGraphWriter: Start writer thread");

                let mut buffer = Vec::with_capacity(32);
                let mut entry_buffer = Vec::new();

                while queue_out.recv_many(&mut buffer, 32).await > 0 {
                    log::trace!("WebGraphWriter:Write {} entries", buffer.len());
                    for value in &buffer {
                        value.collect(&mut entry_buffer);
                    }
                    buffer.clear();
                    write_buffer(&mut entry_buffer, &mut writer).await;
                }

                debug_assert!(buffer.is_empty());
                debug_assert!(entry_buffer.is_empty());

                match writer.flush().await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("WebGraphWriter: Failed to flush data: {err}");
                    }
                }
                let file = writer.into_inner();
                match file.sync_all().await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("WebGraphWriter: Failed to sync to file: {err}");
                    }
                }
                log::debug!("WebGraphWriter: Stopping writer thread");
            });

            Ok(Self {
                queue_in: Some(queue_in),
            })
        } else {
            log::debug!("No Runtime found. Piping entries to nirvana.");
            Ok(Self { queue_in: None })
        }
    }
}

impl WebGraphManager for QueuingWebGraphManager {
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), WebGraphError> {
        if let Some(ref sender) = self.queue_in {
            match sender.send(link_net_entry).await {
                Ok(_) => Ok(()),
                Err(SendError(value)) => {
                    log::error!("Failed to write {:?} to the external file", value);
                    Err(WebGraphError::SendError(value))
                }
            }
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::runtime::{
        GracefulShutdownWithGuard, OptionalAtraHandle, RuntimeContext,
    };
    use crate::url::AtraUri;
    use crate::web_graph::{QueuingWebGraphManager, WebGraphEntry, WebGraphManager};
    use log::LevelFilter;
    use log4rs::append::console::ConsoleAppender;
    use log4rs::config::{Appender, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use log4rs::Config;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn can_write_propery() {
        scopeguard::defer! {
            let _ = std::fs::remove_file(Path::new("./atra_data/example.ttl"));
        }

        let console_logger = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l}{I} - {d} - {m}{n}")))
            .build();

        let config = Config::builder()
            .appender(Appender::builder().build("out", Box::new(console_logger)))
            .logger(Logger::builder().build("atra", LevelFilter::Trace))
            .build(Root::builder().appender("out").build(LevelFilter::Warn))
            .unwrap();

        let _ = log4rs::init_config(config).unwrap();

        let guard = GracefulShutdownWithGuard::new();
        let s = guard.get().clone();
        let writer = Arc::new(
            QueuingWebGraphManager::new(
                10.try_into().unwrap(),
                "./atra_data/example.ttl",
                &RuntimeContext::new(guard, OptionalAtraHandle::None),
            )
            .unwrap(),
        );
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = JoinSet::new();
        for i in 0..20 {
            let c = barrier.clone();
            let w = writer.clone();
            let entry = WebGraphEntry::Link {
                from: (format!("http://www.test.de/{i}")
                    .parse::<AtraUri>()
                    .unwrap()),
                to: (format!("http://www.test.de/{}", i + 1)
                    .parse::<AtraUri>()
                    .unwrap()),
            };
            handles.spawn(async move {
                let wait_result = c.wait().await;
                w.add(entry).await.unwrap();
                wait_result
            });
        }
        while let Some(result) = handles.join_next().await {
            match result {
                Ok(ok) => {
                    println!("Worked: {}", ok.is_leader())
                }
                Err(err) => {
                    println!("JoinError! {err:?}")
                }
            }
        }
        drop(writer);
        log::info!("Waiting!");
        s.wait().await;
        let read = std::fs::read_to_string(Path::new("./atra_data/example.ttl")).unwrap();
        println!("Turtle-File:\n\n{read}")
    }
}
