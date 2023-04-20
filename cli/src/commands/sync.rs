use anyhow::Context;
use atomic_counter::{AtomicCounter, RelaxedCounter};

use clap::{error::ErrorKind, ArgMatches, CommandFactory};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::{iter::ParallelIterator, prelude::IntoParallelRefIterator};
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{clean, fetch, Cli, RemoteRef, ResetType, StashMode};
use crate::{
    commands::track::set_tracking_remote_branch,
    config::{
        repo::{cmp_local_remote, exclude_ignore, TomlRepo},
        repos::{load_config, TomlConfig},
    },
    git,
    utils::logger,
};

pub(crate) fn exec(args: &ArgMatches) {
    // get input path
    let input_path = match args.get_one::<String>("path") {
        Some(path) => PathBuf::from(path),
        None => env::current_dir().unwrap(),
    };

    logger::command_start("sync repos", &input_path);

    let thread_count = args.get_one::<usize>("thread").unwrap_or(&4);
    let hard = args.get_one::<bool>("hard").unwrap_or(&false);
    let stash = args.get_one::<bool>("stash").unwrap_or(&false);
    let silent = args.get_one::<bool>("silent").unwrap_or(&false);
    let no_track = args.get_one::<bool>("no_track").unwrap_or(&false);
    let no_checkout = args.get_one::<bool>("no_checkout").unwrap_or(&false);
    let depth = args.get_one::<usize>("depth");

    let ignore = match args.get_many::<String>("ignore") {
        Some(r) => {
            let ignore = r.collect::<Vec<&String>>();
            Some(ignore)
        }
        _ => None,
    };

    let stash_mode = match (stash, hard) {
        (false, false) => StashMode::Normal,
        (true, false) => StashMode::Stash,
        (false, true) => StashMode::Hard,
        _ => Cli::command()
            .error(
                ErrorKind::ArgumentConflict,
                "'--stash' and '--hard' can't be used together.",
            )
            .exit(),
    };

    // set config file path
    let config_file = match args.get_one::<PathBuf>("config") {
        Some(r) => r.to_owned(),
        _ => input_path.join(".gitrepos"),
    };

    // check if .gitrepos exists
    if !config_file.is_file() {
        logger::config_file_not_found();
        return;
    }

    // load config file(like .gitrepos)
    let Some(toml_config) = load_config(&config_file) else{
        logger::new("load config file failed!");
        return;
    };

    inner_exec(
        input_path,
        toml_config,
        *thread_count,
        stash_mode,
        *silent,
        *no_track,
        *no_checkout,
        depth,
        ignore,
    );
}

fn inner_exec(
    input_path: impl AsRef<Path>,
    toml_config: TomlConfig,
    thread_count: usize,
    stash_mode: StashMode,
    silent: bool,
    no_track: bool,
    no_checkout: bool,
    depth: Option<&usize>,
    ignore: Option<Vec<&String>>,
) {
    // remove unused repositories when use '--config' option
    // also if input_path not exists, skip this process
    if stash_mode == StashMode::Hard && input_path.as_ref().is_dir() {
        clean::exec_clean(&input_path, &toml_config);
    }

    // load .gitrepos
    let Some(mut toml_repos) = toml_config.repos else {
        return;
    };

    let input_path = input_path.as_ref();
    let default_branch = toml_config.default_branch;

    // ignore specified repositories

    exclude_ignore(&mut toml_repos, ignore);

    let repos_count = toml_repos.len();

    // multi_progress manages multiple progress bars from different threads
    // use Arc to share the MultiProgress across more than 1 thread
    let multi_progress = Arc::new(MultiProgress::new());

    // create total progress bar and set progress style
    let total_bar = multi_progress.add(ProgressBar::new(repos_count as u64));
    total_bar.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {percent}% [{bar:30.green/white}] {pos}/{len}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    total_bar.tick();

    // user a counter
    let counter = RelaxedCounter::new(1);

    // Clone Arc<MultiProgress> and spawn a thread.
    // need to do this in a thread as the `.map()` we do below also blocks.
    let multi_progress_wait = multi_progress.clone();

    // create thread pool, and set the number of thread to use by using `.num_threads(count)`
    let thread_builder = rayon::ThreadPoolBuilder::new().num_threads(thread_count);
    let Ok(thread_pool) = thread_builder.build() else
    {
           logger::new("create thread pool failed!");
        return;
    };

    // pool.install means that `.par_iter()` will use the thread pool we've built above.
    let (succ_repos, error_repos) = thread_pool.install(|| {
        let res: Vec<Result<(&TomlRepo, String), (&TomlRepo, anyhow::Error)>> = toml_repos
            .par_iter()
            .map(|toml_repo| {
                let idx = counter.inc();
                let prefix = format!("[{:02}/{:02}]", idx, repos_count);

                // create progress bar for each repo
                let progress_bar = multi_progress_wait.insert(idx, ProgressBar::new_spinner());
                progress_bar.set_style(
                    ProgressStyle::with_template("{spinner:.green.dim.bold} {msg} ")
                        .unwrap()
                        .tick_chars("/-\\| "),
                );
                progress_bar.enable_steady_tick(std::time::Duration::from_millis(500));
                let message = format!("{:>9} waiting...", &prefix);
                progress_bar.set_message(logger::truncate_spinner_msg(&message));

                // get compare stat betwwen local and specified commit/tag/branch/
                let cur_cmp_msg = match silent {
                    true => String::new(),
                    false => {
                        match cmp_local_remote(input_path, toml_repo, &default_branch, false) {
                            Ok(r) => r.unwrap(),
                            _ => String::new(),
                        }
                    }
                };

                // execute command according each repo status
                let exec_result = exec_sync_with_progress(
                    input_path,
                    toml_repo,
                    &stash_mode,
                    no_checkout,
                    depth,
                    &default_branch,
                    &prefix,
                    &progress_bar,
                );

                // handle result
                let rel_path = toml_repo.local.as_ref().unwrap();
                let result = match exec_result {
                    Ok(_) => {
                        let mut msg = logger::fmt_spinner_finished_prefix(prefix, rel_path, true);

                        // if not silent, show compare stat betweent local and remote
                        if !silent {
                            // get compare stat betwwen local and specified commit/tag/branch/
                            let cmp_res =
                                cmp_local_remote(input_path, toml_repo, &default_branch, false);

                            let mut new_cmp_msg = match cmp_res {
                                Ok(r) => r.unwrap(),
                                _ => String::new(),
                            };

                            if cur_cmp_msg != new_cmp_msg
                                && new_cmp_msg.contains("already update to date.")
                            {
                                new_cmp_msg = new_cmp_msg.replace("already update to date.", "");
                                new_cmp_msg = logger::fmt_update_to_desc(new_cmp_msg.trim());
                            }

                            msg = format!("{}: {}", msg, &new_cmp_msg)
                        };

                        // show message in progress bar
                        progress_bar.finish_with_message(logger::truncate_spinner_msg(&msg));

                        // track remote branch, return track status
                        let track_msg = match no_track {
                            true => String::new(),
                            false => match set_tracking_remote_branch(
                                input_path,
                                &toml_repo,
                                &default_branch,
                            ) {
                                Ok(r) => r,
                                _ => String::new(),
                            },
                        };

                        Ok((toml_repo, track_msg))
                    }
                    Err(e) => {
                        let message = logger::fmt_spinner_finished_prefix(
                            prefix,
                            toml_repo.local.as_ref().unwrap(),
                            false,
                        );

                        // show message in progress bar
                        progress_bar.finish_with_message(logger::truncate_spinner_msg(&message));

                        Err((toml_repo, e))
                    }
                };

                // update total progress bar
                total_bar.inc(1);

                result
            })
            .collect();

        total_bar.finish();

        // collect repos
        let mut succ_repos: Vec<(&TomlRepo, String)> = Vec::new();
        let mut error_repos: Vec<(&TomlRepo, anyhow::Error)> = Vec::new();
        for r in res {
            match r {
                Ok((toml_repo, track_msg)) => succ_repos.push((toml_repo, track_msg)),
                Err((toml_repo, e)) => error_repos.push((toml_repo, e)),
            }
        }
        (succ_repos, error_repos)
    });

    logger::new("\n");
    logger::error_statistics("sync", error_repos.len());

    // show track status
    if !silent {
        logger::new("Track status:");
        succ_repos
            .iter()
            .for_each(|(_, msg)| logger::new(format!("  {}", msg)))
    }

    // show errors
    if !error_repos.is_empty() {
        logger::new("Errors:");
        error_repos.iter().for_each(|(toml_repo, error)| {
            logger::error_detail(&toml_repo.local.as_ref().unwrap(), error);
        });
    }
}

fn exec_sync_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    stash_mode: &StashMode,
    no_checkout: bool,
    depth: Option<&usize>,
    default_branch: &Option<String>,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> anyhow::Result<()> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = &input_path.join(rel_path);

    // make repo directory and skip clone the repository
    std::fs::create_dir_all(full_path)
        .with_context(|| format!("create dir {} failed.", full_path.to_str().unwrap()))?;

    let mut toml_repo = toml_repo.to_owned();
    let mut stash_mode = stash_mode.to_owned();
    let is_repo_none = git::is_repository(full_path.as_path()).is_err();
    // if repository not found, create new one
    if is_repo_none {
        // use --hard
        stash_mode = StashMode::Hard;

        // git init when dir exist
        exec_init_with_progress(input_path, &toml_repo, prefix, progress_bar)?;
        // git remote add url
        exec_add_remote_with_progress(input_path, &toml_repo, prefix, progress_bar)?;
    }

    // use default branch when branch is null
    if None == toml_repo.branch {
        toml_repo.branch = default_branch.to_owned();
    }

    // fetch
    fetch::exec_fetch_with_progress(input_path, &toml_repo, depth, prefix, progress_bar)?;

    // priority: commit/tag/branch(default-branch)
    let remote_ref = toml_repo.get_remote_ref(full_path.as_path())?;
    let remote_ref_str = match remote_ref {
        RemoteRef::Commit(commit) => commit,
        RemoteRef::Tag(tag) => tag,
        RemoteRef::Branch(branch) => branch,
    };

    // check remote-ref valid
    git::is_remote_ref_valid(full_path, &remote_ref_str)?;

    match stash_mode {
        StashMode::Normal => {
            // try stash → checkout → reset → stash pop
            if !no_checkout {
                // stash
                let stash_result =
                    exec_stash_with_progress(input_path, &toml_repo, prefix, progress_bar);
                let stash_message = stash_result.unwrap_or("stash failed.".to_string());

                // checkout
                let mut result: Result<(), anyhow::Error>;
                result = exec_checkout_with_progress(
                    input_path,
                    &toml_repo,
                    false,
                    prefix,
                    progress_bar,
                );

                if result.is_ok() {
                    // reset --hard
                    result = exec_reset_with_progress(
                        input_path,
                        &toml_repo,
                        ResetType::Hard,
                        prefix,
                        progress_bar,
                    );
                }

                // stash pop, whether checkout succ or failed, whether reset succ or failed
                if stash_message.contains("WIP") {
                    let _ =
                        exec_stash_pop_with_progress(input_path, &toml_repo, prefix, progress_bar);
                }
                result
            } else {
                // reset --soft
                exec_reset_with_progress(
                    input_path,
                    &toml_repo,
                    ResetType::Soft,
                    prefix,
                    progress_bar,
                )
            }
        }
        StashMode::Stash => {
            // stash with `--stash` option, maybe return error if need to initial commit
            let stash_result =
                exec_stash_with_progress(input_path, &toml_repo, prefix, progress_bar);

            let stash_message = stash_result.unwrap_or("stash failed.".to_string());

            // checkout
            let mut result: Result<(), anyhow::Error> = Ok(());
            let mut reset_type = ResetType::Mixed;
            if !no_checkout {
                result =
                    exec_checkout_with_progress(input_path, &toml_repo, true, prefix, progress_bar)
                        .with_context(|| stash_message.clone());

                reset_type = ResetType::Hard;
            }

            // reset --mixed
            if result.is_ok() {
                result = exec_reset_with_progress(
                    input_path,
                    &toml_repo,
                    reset_type,
                    prefix,
                    progress_bar,
                )
                .with_context(|| stash_message.clone());
            }

            // undo if checkout failed or reset failed
            if let Err(e) = result {
                // if reset failed, pop stash if stash something this time
                if stash_message.contains("WIP") {
                    let _ =
                        exec_stash_pop_with_progress(input_path, &toml_repo, prefix, progress_bar);
                }
                return Err(e);
            }
            result
        }
        StashMode::Hard => {
            // clean
            exec_clean_with_progress(input_path, &toml_repo, prefix, progress_bar)?;

            // checkout
            if !no_checkout {
                exec_checkout_with_progress(input_path, &toml_repo, true, prefix, progress_bar)?;
            }

            // reset --hard
            exec_reset_with_progress(
                input_path,
                &toml_repo,
                ResetType::Hard,
                prefix,
                progress_bar,
            )
        }
    }
}

fn exec_init_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> anyhow::Result<()> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "initialize...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    git::init(full_path)
}

fn exec_add_remote_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> anyhow::Result<()> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "add remote...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    let url = toml_repo.remote.as_ref().unwrap();
    git::add_remote_url(full_path, url)
}

fn exec_clean_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> anyhow::Result<()> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "clean...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    git::clean(full_path)
}

fn exec_reset_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    reset_type: ResetType,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> anyhow::Result<()> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "reset...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    // priority: commit/tag/branch(default-branch)
    let remote_ref = toml_repo.get_remote_ref(full_path.as_path())?;
    let remote_ref_str = match remote_ref {
        RemoteRef::Commit(commit) => commit,
        RemoteRef::Tag(tag) => tag,
        RemoteRef::Branch(branch) => branch,
    };

    let reset_type = match reset_type {
        ResetType::Soft => "--soft",
        ResetType::Mixed => "--mixed",
        ResetType::Hard => "--hard",
    };
    git::reset(full_path, reset_type, remote_ref_str)
}

fn exec_stash_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> Result<String, anyhow::Error> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "stash...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    git::stash(full_path)
}

fn exec_stash_pop_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> Result<String, anyhow::Error> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "pop stash...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    git::stash_pop(full_path)
}

fn exec_checkout_with_progress(
    input_path: &Path,
    toml_repo: &TomlRepo,
    force: bool,
    prefix: &str,
    progress_bar: &ProgressBar,
) -> anyhow::Result<()> {
    let rel_path = toml_repo.local.as_ref().unwrap();
    let full_path = input_path.join(rel_path);

    let message = logger::fmt_spinner_desc(prefix, rel_path, "checkout...");
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    // priority: commit/tag/branch(default-branch)
    let remote_ref = toml_repo.get_remote_ref(full_path.as_path())?;
    let remote_ref_str = match remote_ref.clone() {
        RemoteRef::Commit(commit) => commit,
        RemoteRef::Tag(tag) => tag,
        RemoteRef::Branch(branch) => branch,
    };
    let branch = match remote_ref {
        RemoteRef::Commit(commit) => format!("commits/{}", &commit[..7]),
        RemoteRef::Tag(tag) => format!("tags/{}", tag),
        RemoteRef::Branch(_) => toml_repo
            .branch
            .clone()
            .unwrap_or("invalid-branch".to_string()),
    };

    // don't need to checkout if current branch is the branch
    if let Ok(currnte_branch) = git::get_current_branch(full_path.as_path()) {
        if branch == currnte_branch {
            return Ok(());
        }
    }

    let suffix = logger::fmt_checkouting(&branch);
    let message = logger::fmt_spinner_desc(prefix, rel_path, suffix);
    progress_bar.set_message(logger::truncate_spinner_msg(&message));

    // check if local branch already exists
    let branch_exist = git::local_branch_already_exist(&full_path, &branch)?;

    // create/checkout/reset branch
    let args = match (branch_exist, force) {
        (false, false) => vec!["checkout", "-B", &branch, &remote_ref_str, "--no-track"],
        (false, true) => vec![
            "checkout",
            "-B",
            &branch,
            &remote_ref_str,
            "--no-track",
            "-f",
        ],
        (true, false) => vec!["checkout", &branch],
        (true, true) => vec!["checkout", "-B", &branch, "-f"],
    };

    git::checkout(full_path, &args)
}
