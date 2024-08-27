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

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// Config of the session, basically paths etc.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename(serialize = "Paths"))]
pub struct PathsConfig {
    /// The root path where the application runs
    #[serde(default = "_default_root_folder")]
    pub root: Utf8PathBuf,
    directories: Directories,
    files: Files
}

fn _default_root_folder() -> Utf8PathBuf { "./atra_data".parse::<Utf8PathBuf>().unwrap() }


impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            root: _default_root_folder(),
            files: Files::default(),
            directories: Directories::default()
        }
    }
}

impl PathsConfig {
    pub fn new(root: impl AsRef<Utf8Path>, directories: Directories, files: Files) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            directories,
            files
        }
    }
}


macro_rules! path_constructors {
    ($self: ident.($($root: ident => $name: ident = $path1: ident.$path2: ident;)+)) => {
        $(
            pub fn $name(&$self) -> Utf8PathBuf {
                $self.$root.join(&$self.$path1.$path2)
            }

            paste::paste! {
                pub fn [<$name _name>](&$self) -> Option<&str> {
                    $self.$path1.$path2.file_name()
                }
            }
        )+
    };
}

impl PathsConfig {

    pub fn root_path(&self) -> &Utf8Path  {
        self.root.as_path()
    }


    path_constructors! {
        self.(
            root => dir_database = directories.database;
            root => file_queue = files.queue;
            root => file_blacklist = files.blacklist;
            root => file_web_graph = files.web_graph;
            root => dir_big_files = directories.big_files;
        )
    }

}



#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Directories {
    /// Path to the database directory
    #[serde(default = "_default_database_dir")]
    pub database: Utf8PathBuf,
    /// Path to the big files directory
    #[serde(default = "_default_big_files_dir")]
    pub big_files: Utf8PathBuf,
}

impl Directories {
    pub fn new(database: impl AsRef<Utf8Path>, big_files: impl AsRef<Utf8Path>) -> Self {
        Self {
            database: database.as_ref().to_path_buf(),
            big_files: big_files.as_ref().to_path_buf()
        }
    }
}

impl Default for Directories {
    fn default() -> Self {
        Self {
            database: _default_database_dir(),
            big_files: _default_big_files_dir()
        }
    }
}

fn _default_database_dir() -> Utf8PathBuf { "./rocksdb".parse::<Utf8PathBuf>().unwrap() }
fn _default_big_files_dir() -> Utf8PathBuf { "./big_files".parse::<Utf8PathBuf>().unwrap() }

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Files {
    #[serde(default = "_default_queue_file")]
    pub queue: Utf8PathBuf,
    #[serde(default = "_default_blacklist_file")]
    pub blacklist: Utf8PathBuf,
    #[serde(default = "_default_web_graph_file")]
    pub web_graph: Utf8PathBuf
}

impl Files {
    pub fn new(queue: impl AsRef<Utf8Path>, blacklist: impl AsRef<Utf8Path>, web_graph: impl AsRef<Utf8Path>) -> Self {
        Self {
            queue: queue.as_ref().to_path_buf(),
            blacklist: blacklist.as_ref().to_path_buf(),
            web_graph: web_graph.as_ref().to_path_buf()
        }
    }
}

impl Default for Files {
    fn default() -> Self {
        Self {
            queue: _default_queue_file(),
            blacklist: _default_blacklist_file(),
            web_graph: _default_web_graph_file()
        }
    }
}


fn _default_queue_file() -> Utf8PathBuf { "./queue.tmp".parse::<Utf8PathBuf>().unwrap() }
fn _default_blacklist_file() -> Utf8PathBuf { "./blacklist.txt".parse::<Utf8PathBuf>().unwrap() }
fn _default_web_graph_file() -> Utf8PathBuf { "./web_graph.rdf".parse::<Utf8PathBuf>().unwrap() }


#[cfg(test)]
mod test {
    use crate::core::config::paths::PathsConfig;

    #[test]
    fn can_make_init(){
        let config = PathsConfig::default();
        println!("{}", config.dir_big_files())
    }
}