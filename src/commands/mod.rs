use serde::{Deserialize, Serialize};
use toml_edit;

pub mod clean;
pub mod fetch;
pub mod init;
pub mod sync;

/// this type is used to deserialize `.gitrepos` files.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlConfig {
    version: Option<String>,
    default_branch: Option<String>,
    default_remote: Option<String>,
    repos: Option<Vec<TomlRepo>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlRepo {
    local: Option<String>,
    remote: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    commit: Option<String>,
}

// serialzie config file .gitrepos
impl TomlConfig {
    pub fn serialize(&self) -> String {
        let toml = toml_edit::ser::to_item(self).unwrap();
        let mut out = String::new();

        out.push_str("# This file is automatically @generated by mgit.\n");
        out.push_str("# Editing it as you wish.\n");

        // version = "x.y.z"
        if let Some(item) = toml.get("version") {
            out.push_str(&format!("version = {}\n", item));
        }

        // default-branch = "your_branch"
        if let Some(item) = toml.get("default-branch") {
            out.push_str(&format!("default-branch = {}\n", item));
        }

        // default-remote = "your_remote"
        if let Some(item) = toml.get("default-remote") {
            out.push_str(&format!("default-remote = {}\n", item));
        }

        out.push_str("\n");

        // [[repos]]
        if let Some(repos) = toml.get("repos") {
            let list = repos.as_array().expect("repos must be an array");

            for entry in list {
                out.push_str("[[repos]]\n");
                let table = entry.as_inline_table().expect("repo must be table");

                // local = "your/local/path"
                if let Some(item) = table.get("local") {
                    out.push_str(&format!("local = {}\n", item));
                }

                // remote = "your://remote/url"
                if let Some(item) = table.get("remote") {
                    out.push_str(&format!("remote = {}\n", item));
                }

                // branch = "your_branch"
                if let Some(item) = table.get("branch") {
                    out.push_str(&format!("branch = {}\n", item));
                }

                // tag = "your_tag"
                if let Some(item) = table.get("tag") {
                    out.push_str(&format!("tag = {}\n", item));
                }

                // commit = "your_tag"
                if let Some(item) = table.get("commit") {
                    out.push_str(&format!("commit = {}\n", item));
                }

                out.push_str("\n");
            }
        }

        out
    }
}

// TODO
// pub fn load_config(path: &Path) -> Option<TomlConfig> {
//     let pb = path.to_path_buf();

//     // check if .mgit/ exists
//     let user_dir = pb.join(".mgit");
//     if user_dir.is_dir() == false {
//         return None;
//     }

//     None
// }
