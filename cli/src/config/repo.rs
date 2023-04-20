use anyhow::Context;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

use crate::{
    commands::RemoteRef,
    git,
    utils::{logger, path::display_path},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlRepo {
    pub local: Option<String>,
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub commit: Option<String>,
}

impl TomlRepo {
    pub fn get_remote_name(&self, path: impl AsRef<Path>) -> Result<String, anyhow::Error> {
        let remote_url = self
            .remote
            .as_ref()
            .with_context(|| "remote url is null.")?;
        git::find_remote_name_by_url(path, remote_url)
    }

    pub fn get_remote_ref(&self, path: &Path) -> Result<RemoteRef, anyhow::Error> {
        let remote_name = &self.get_remote_name(path)?;
        // priority: commit/tag/branch(default-branch)
        let remote_ref = {
            if let Some(commit) = &self.commit {
                RemoteRef::Commit(commit.to_string())
            } else if let Some(tag) = &self.tag {
                RemoteRef::Tag(tag.to_string())
            } else if let Some(branch) = &self.branch {
                let branch = format!("{}/{}", remote_name, branch.to_string());
                RemoteRef::Branch(branch)
            } else {
                return Err(anyhow::anyhow!("remote ref is invalid!"));
            }
        };
        Ok(remote_ref)
    }
}

pub fn exclude_ignore(toml_repos: &mut Vec<TomlRepo>, ignore: Option<Vec<&String>>) {
    if let Some(ignore_paths) = ignore {
        for ignore_path in ignore_paths {
            if let Some(idx) = toml_repos.iter().position(|r| {
                if let Some(rel_path) = r.local.as_ref() {
                    // consider "." as root path
                    display_path(rel_path) == *ignore_path
                } else {
                    false
                }
            }) {
                toml_repos.remove(idx);
            }
        }
    }
}

/// get full ahead/behind values between branches
pub fn cmp_local_remote(
    input_path: impl AsRef<Path>,
    toml_repo: &TomlRepo,
    default_branch: &Option<String>,
    use_tracking_remote: bool,
) -> Result<Option<String>, anyhow::Error> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.as_ref().join(rel_path);

    let mut toml_repo = toml_repo.to_owned();
    // use default branch when branch is null
    if None == toml_repo.branch {
        toml_repo.branch = default_branch.to_owned();
    }

    // priority: commit/tag/branch(default-branch)
    let (remote_ref_str, remote_desc) = {
        if use_tracking_remote {
            let remote_ref_str = git::get_tracking_branch(&full_path)?;
            (remote_ref_str.clone(), remote_ref_str)
        } else {
            let remote_ref = toml_repo.get_remote_ref(&full_path)?;
            let remote_ref_str = match remote_ref.clone() {
                RemoteRef::Commit(commit) => commit,
                RemoteRef::Tag(tag) => tag,
                RemoteRef::Branch(branch) => branch,
            };
            let remote_desc = match remote_ref {
                RemoteRef::Commit(commit) => (&commit[..7]).to_string(),
                RemoteRef::Tag(tag) => tag,
                RemoteRef::Branch(branch) => branch,
            };
            (remote_ref_str, remote_desc)
        }
    };

    // if specified remote commit/tag/branch is null
    if remote_desc.is_empty() {
        return Ok(Some("not tracking".to_string()));
    }

    let mut changed_files: HashSet<String> = HashSet::new();

    // get untracked files (uncommit)
    if let Ok(output) = git::get_untrack_files(&full_path) {
        for file in output.trim().lines() {
            changed_files.insert(file.to_string());
        }
    }

    // get tracked and changed files (uncommit)
    if let Ok(output) = git::get_changed_files(&full_path) {
        for file in output.trim().lines() {
            changed_files.insert(file.to_string());
        }
    }

    // get cached(staged) files (uncommit)
    if let Ok(output) = git::get_staged_files(&full_path) {
        for file in output.trim().lines() {
            changed_files.insert(file.to_string());
        }
    }

    let mut changes_desc = String::new();
    if !changed_files.is_empty() {
        // format changes tooltip
        changes_desc = logger::fmt_changes_desc(changed_files.len());
    }

    // get local branch
    let branch = git::get_current_branch(&full_path)?;

    if branch.is_empty() {
        return Ok(Some("init commit".to_string()));
    }

    // get rev-list between local branch and specified remote commit/tag/branch
    let branch_pair = format!("{}...{}", &branch, &remote_ref_str);
    let mut commit_desc = String::new();

    if let Ok(output) = git::get_rev_list_count(&full_path, branch_pair) {
        let re = Regex::new(r"(\d+)\s*(\d+)").unwrap();

        if let Some(caps) = re.captures(&output) {
            // format commit tooltip
            let (ahead, behind) = (&caps[1], &caps[2]);
            commit_desc = logger::fmt_commit_desc(ahead, behind);
        }
    } else {
        // if git rev-list find "unknown revision" error
        commit_desc = logger::fmt_unknown_revision_desc();
    }

    // show diff overview
    let desc = if commit_desc.is_empty() && changes_desc.is_empty() {
        let branch_log = git::get_branch_log(&full_path, branch);
        logger::fmt_update_to_date_desc(branch_log)
    } else {
        logger::fmt_diff_desc(remote_desc, commit_desc, changes_desc)
    };

    Ok(Some(desc))
}
