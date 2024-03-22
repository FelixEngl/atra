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

use camino::Utf8PathBuf as PathBuf;
use ini::Ini;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use crate::core::ini_ext::{FromIni, IniExt, IntoIni, SectionSetterExt};

/// Config of the session, basically paths etc.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathsConfig {

    /// The root path where the application runs
    pub root_folder: String,
    pub db_dir_name: Option<String>,
    pub queue_file_name: Option<String>,
    pub blacklist_name: Option<String>,
    pub big_file_dir_name: Option<String>,
    pub web_graph_file_name: Option<String>
}

const DEFAULT_ATRA_DATA_ROOT: &str = "atra_data";


impl FromIni for PathsConfig {
    fn from_ini(ini: &Ini) -> Self {
        Self {
            root_folder: ini.get_or(Some("Paths"), "root", DEFAULT_ATRA_DATA_ROOT.to_string()),
            db_dir_name: ini.get(Some("Paths"), "db_dir_name"),
            queue_file_name: ini.get(Some("Paths"), "queue_file_name"),
            blacklist_name: ini.get(Some("Paths"), "blacklist_name"),
            big_file_dir_name: ini.get(Some("Paths"), "big_file_dir_name"),
            web_graph_file_name: ini.get(Some("Paths"), "web_graph_file_name")
        }
    }
}

impl IntoIni for PathsConfig {
    fn insert_into(&self, ini: &mut Ini) {
        ini.with_section(Some("Paths"))
            .set("root", &self.root_folder)
            .set_optional("db_dir_name", self.db_dir_name.as_ref())
            .set_optional("queue_file_name", self.queue_file_name.as_ref())
            .set_optional("blacklist_name", self.blacklist_name.as_ref())
            .set_optional("big_file_dir_name", self.big_file_dir_name.as_ref())
            .set_optional("web_graph_file_name", self.web_graph_file_name.as_ref());
    }
}


macro_rules! path_constructors {
    ($self: ident: $($name: ident($optional_path: ident, $root: ident => $default: literal);)+) => {
        $(
            pub fn $name(&$self) -> PathBuf {
                let mut new = PathBuf::new();
                if let Some(ref path) = $self.$optional_path {
                    new.push(&$self.$root);
                    new.push(path);
                } else {
                    new.push(&$self.$root);
                    new.push($default);
                }
                new
            }

            paste::paste! {
                pub fn [<$name _name>](&$self) -> Cow<str> {
                    if let Some(ref path) = $self.$optional_path {
                        Cow::Borrowed(path)
                    } else {
                        Cow::Borrowed($default)
                    }
                }
            }
        )+
    };
}

impl PathsConfig {

    pub fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.root_folder)
    }

    /// Allows to update the session config with another one
    #[allow(dead_code)]
    pub fn override_with(self, other: Self) -> Self {
        Self {
            root_folder: if other.root_folder == DEFAULT_ATRA_DATA_ROOT { self.root_folder } else { other.root_folder },
            db_dir_name: other.db_dir_name.or(self.db_dir_name),
            blacklist_name: other.blacklist_name.or(self.blacklist_name),
            queue_file_name: other.queue_file_name.or(self.queue_file_name),
            big_file_dir_name: other.big_file_dir_name.or(self.big_file_dir_name),
            web_graph_file_name: other.web_graph_file_name.or(self.web_graph_file_name),
        }
    }

    path_constructors! {
        self:
        dir_database(db_dir_name, root_folder => "rocksdb");
        file_queue(queue_file_name, root_folder => "queue.tmp");
        file_blacklist(blacklist_name, root_folder => "blacklist.txt");
        dir_big_files(big_file_dir_name, root_folder => "big_files");
        file_web_graph(web_graph_file_name, root_folder => "web_graph.rdf");
    }

}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            root_folder: DEFAULT_ATRA_DATA_ROOT.to_string(),
            blacklist_name: None,
            queue_file_name: None,
            db_dir_name: None,
            big_file_dir_name: None,
            web_graph_file_name: None
        }
    }
}

#[cfg(test)]
mod test {
    use crate::core::config::paths::PathsConfig;
    use crate::core::ini_ext::IntoIni;

    #[test]
    fn can_make_init(){
        let mut config = PathsConfig::default();
        config.db_dir_name = Some(config.dir_database().to_string());
        config.queue_file_name = Some(config.file_queue().to_string());
        config.blacklist_name = Some(config.file_blacklist().to_string());
        config.big_file_dir_name = Some(config.dir_big_files().to_string());

        let ini = config.to_ini();
        ini.write_to_file("session-config.ini").unwrap();
    }
}