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

use std::ffi::OsString;
use std::fs::File as StdFile;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use std::io;
use std::io::{BufRead, Write, BufReader as StdBufReader, ErrorKind};
use std::num::NonZeroUsize;
use std::path::Path;
use case_insensitive_string::CaseInsensitiveString;
use thiserror::Error;
use tokio::sync::mpsc::{Sender};
use tokio::sync::mpsc::error::{SendError};
use ubyte::ByteUnit;
use crate::core::crawl::seed::CrawlSeed;
use crate::core::UrlWithDepth;
use crate::util::RuntimeContext;

#[derive(Debug)]
pub enum WebGraphEntry {
    Seed {
        domain: CaseInsensitiveString,
        seed: String
    },
    Link {
        from: String,
        to: String
    }
}

impl WebGraphEntry {
    pub fn create_link(from: &UrlWithDepth, to: &UrlWithDepth) -> Self {
        Self::Link {
            from: from.url.to_string(),
            to: to.url.to_string()
        }
    }

    pub fn create_seed(seed: &impl CrawlSeed) -> Self {
        Self::Seed {
            seed: seed.url().url.to_string(),
            domain: seed.domain().clone()
        }
    }

    fn as_internal_notation3(&self) -> String {
        match self {
            WebGraphEntry::Seed { domain, seed } => {
                format!("\"{}\" :has_seed <{seed}> .", domain.as_ref())
            }
            WebGraphEntry::Link { from, to } => {
                format!("<{from}> :links_to <{to}> .")
            }
        }
    }

    fn to_internal_notation3(self) -> String {
        match self {
            WebGraphEntry::Seed { domain, seed } => {
                format!("\"{}\" :has_seed <{seed}> .", domain.as_ref())
            }
            WebGraphEntry::Link { from, to } => {
                format!("<{from}> :links_to <{to}> .")
            }
        }
    }
}



#[derive(Debug, Error)]
pub enum LinkNetError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error("The file at {0:?} is not valid!")]
    InvalidFile(OsString),
    #[error("Failed to send an entry to the writer thread.")]
    SendError(WebGraphEntry),
}

/// Manages the webgraph
pub trait WebGraphManager {
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), LinkNetError>;
}

/// The default size for a links cache. Usually 10k links are cached.
pub const DEFAULT_CACHE_SIZE_WEB_GRAPH: NonZeroUsize = unsafe{NonZeroUsize::new_unchecked(20_000)};

/// A link net manager with a backing file.
#[derive(Debug)]
pub struct QueuingWebGraphManager {
    queue_in: Sender<WebGraphEntry>,
}

impl QueuingWebGraphManager {
    /// Creates the manager with a cache of the size [capacity] at the file [path].
    pub fn new(
        capacity: NonZeroUsize,
        path: impl AsRef<Path>,
        shutdown_and_handle: &RuntimeContext,
    ) -> Result<Self, LinkNetError> {

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
            return Err(LinkNetError::IOError(io::Error::from(ErrorKind::Unsupported)))
        }

        let needs_header = meta.len() == 0;

        if !needs_header {
            if !StdBufReader::new(&file).lines()
                .any(|value|
                    match value {
                        Ok(value) => {
                            value.starts_with("@prefix") && value.contains("http://atra.de/")
                        }
                        Err(_) => {false}
                    }
                )
            {
                return Err(LinkNetError::InvalidFile(path.as_ref().as_os_str().to_os_string()))
            }
        } else  {
            writeln!(&mut file, "@prefix : <http://atra.de/>").unwrap();
        }


        let mut writer = BufWriter::with_capacity(ByteUnit::Kilobyte(32).as_u64() as usize, File::from_std(file));
        let (queue_in, mut queue_out) = tokio::sync::mpsc::channel::<WebGraphEntry>(capacity.get());
        let guard = shutdown_and_handle.shutdown_guard().clone();

        // todo: may need scaling
        shutdown_and_handle.handle().io_or_main_or_current().spawn(
            async move {
                log::debug!("WebGraphWriter: Start writer thread");
                let _guard = guard;
                while let Some(value) = queue_out.recv().await {
                    log::trace!("WebGraphWriter:Write {:?}", value);
                    match writer.write_all(value.to_internal_notation3().as_bytes()).await {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("WebGraphWriter: encountered a problem:{err}")
                        }
                    }
                    match writer.write_u8(b'\n').await {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("WebGraphWriter: encountered a problem:{err}")
                        }
                    }
                }

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
            }
        );

        Ok(Self { queue_in })
    }

}

impl WebGraphManager for QueuingWebGraphManager {
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), LinkNetError> {
        match self.queue_in.send(link_net_entry).await {
            Ok(_) => {return Ok(())}
            Err(SendError(value)) => {
                log::error!("Failed to write {} to the external file", value.as_internal_notation3());
                return Err(LinkNetError::SendError(value))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use std::sync::Arc;
    use log4rs::append::console::ConsoleAppender;
    use log4rs::Config;
    use log4rs::config::{Appender, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use log::LevelFilter;
    use tokio::sync::{Barrier};
    use crate::core::web_graph::{WebGraphEntry, WebGraphManager, QueuingWebGraphManager};
    use tokio::task::{JoinSet};
    use crate::core::runtime::OptionalAtraHandle;
    use crate::core::shutdown::{graceful_shutdown, UnsafeShutdownGuard};
    use crate::util::RuntimeContext;

    #[tokio::test]
    async fn can_write_propery(){
        scopeguard::defer! {
            let _ = std::fs::remove_file(Path::new("./atra_data/example.n3"));
        }

        let (_, b, mut guard) = graceful_shutdown();

        let console_logger = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l}{I} - {d} - {m}{n}")))
            .build();

        let config = Config::builder()
            .appender(Appender::builder().build("out", Box::new(console_logger)))
            .logger(Logger::builder().build("atra", LevelFilter::Trace))
            .build(Root::builder().appender("out").build(LevelFilter::Warn))
            .unwrap();

        let _ = log4rs::init_config(config).unwrap();

        let writer = Arc::new(QueuingWebGraphManager::new(
            10.try_into().unwrap(),
            "./atra_data/example.n3",
            &RuntimeContext::new(
                UnsafeShutdownGuard::Guarded(b.into_inner().1),
                OptionalAtraHandle::None
            )
        ).unwrap());
        let barrier = Arc::new(Barrier::new(20));
        let mut handles = JoinSet::new();
        for i in 0..20 {
            let c = barrier.clone();
            let w = writer.clone();
            let entry = WebGraphEntry::Link {
                from: format!("http://www.test.de/{i}"),
                to: format!("http://www.test.de/{}", i+1),
            };
            handles.spawn(
                async move {
                    let wait_result = c.wait().await;
                    w.add(entry).await.unwrap();
                    wait_result
                }
            );
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
        guard.wait().await;
        let read = std::fs::read_to_string(Path::new("./atra_data/example.n3")).unwrap();
        println!("N3-File:\n\n{read}")
    }
}