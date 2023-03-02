use crate::common::{execute_cargo_cmd, execute_cmd, failed_message};
use std::env;
use std::path::PathBuf;

mod common;

/// 测试内容：
///     1、运行命令 mgit init <path>
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、branch
///     3、根目录不是仓库
///     4. 只有同级仓库目录
///
/// 测试目录结构:
///   test_snapshot_init
///     ├─foobar-1 (.git)
///     └─foobar-2 (.git)
#[test]
fn cli_init_simple() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/test_init_simple");

    create_repos_tree1(&path);

    for repo_path in ["foobar-1", "foobar-2"] {
        execute_cmd(&path.join(repo_path), "git", &["fetch", "--all"]).unwrap();
        execute_cmd(
            &path.join(repo_path),
            "git",
            &["branch", "-u", "origin/master"],
        )
        .expect(failed_message::GIT_BRANCH);
    }

    let input_path = path.clone().into_os_string().into_string().unwrap();
    // execute cli init function with path
    execute_cargo_cmd("mgit", &["init", &input_path]);

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(input_path + "/.gitrepos").unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

/// 测试内容：
///     1、运行命令 mgit init <path> --force
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、branch
///     3、根目录是仓库
///     4. 只有同级仓库目录
///
/// 测试目录结构:
///   cli_init_force1 (.git)
///     ├─foobar-1 (.git)
///     └─foobar-2 (.git)
#[test]
fn cli_init_force1() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/test_init_force1");

    create_repos_tree2(&path);

    for repo_path in ["", "foobar-1", "foobar-2"] {
        execute_cmd(&path.join(repo_path), "git", &["fetch", "--all"])
            .expect(failed_message::GIT_FETCH);
        execute_cmd(
            &path.join(repo_path),
            "git",
            &["branch", "-u", "origin/master"],
        )
        .expect(failed_message::GIT_BRANCH);
    }

    let input_path = path.clone().into_os_string().into_string().unwrap();
    // execute cli init function with path
    execute_cargo_cmd("mgit", &["init", &input_path, "--force"]);

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(input_path + "/.gitrepos").unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "."
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

/// 测试内容：
///     1、运行命令 mgit init <path> --force
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、branch
///     3、根目录不是仓库
///     4. 具有父子级仓库目录
///
/// 测试目录结构:
///   cli_init_force2 (.git)
///     ├─foobar-1 (.git)
///     │  ├──foobar-1-1 (.git)
///     │  └──foobar-1-2 (.git)
///     └─foobar-2 (.git)
///        ├──foobar-2-1 (.git)
///        └──foobar-2-2 (.git)
#[test]
fn cli_init_force2() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/cli_init_force2");
    std::fs::create_dir_all(path.clone()).unwrap();

    create_repos_tree3(&path);
    let repo_paths = [
        "",
        "foobar-1",
        "foobar-1/foobar-1-1",
        "foobar-1/foobar-1-2",
        "foobar-2",
        "foobar-2/foobar-2-1",
        "foobar-2/foobar-2-2",
    ];
    for repo_path in repo_paths {
        execute_cmd(&path.join(repo_path), "git", &["fetch", "--all"])
            .expect(failed_message::GIT_FETCH);
        execute_cmd(
            &path.join(repo_path),
            "git",
            &["branch", "-u", "origin/master"],
        )
        .expect(failed_message::GIT_BRANCH);
    }
    let input_path = path.clone().into_os_string().into_string().unwrap();
    // execute cli init function with path
    execute_cargo_cmd("mgit", &["init", &input_path, "--force"]);

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(input_path + "/.gitrepos").unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "."
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-1/foobar-1-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-1/foobar-1-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-2/foobar-2-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-2/foobar-2-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

/// 测试内容：
///     1、运行命令 mgit snapshot <path>
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、commit
///     3、根目录不是仓库
#[test]
fn cli_snapshot_simple() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/test_snapshot_simple");

    create_repos_tree1(&path);

    let input_path = path.clone().into_os_string().into_string().unwrap();
    // execute cli init function with path
    execute_cargo_cmd("mgit", &["snapshot", &input_path]);

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(input_path + "/.gitrepos").unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "8d90314117b4cb86abb6c4d55130437c6d87a30d"

[[repos]]
local = "foobar-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "8d90314117b4cb86abb6c4d55130437c6d87a30d"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

/// 测试内容：
///     1、运行命令 mgit snapshot <path> --branch
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、branch
///     3、根目录不是仓库
#[test]
fn cli_snapshot_branch() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/test_snapshot_branch");

    create_repos_tree1(&path);
    for repo_path in ["foobar-1", "foobar-2"] {
        execute_cmd(&path.join(repo_path), "git", &["fetch", "--all"])
            .expect(failed_message::GIT_FETCH);
        execute_cmd(
            &path.join(repo_path),
            "git",
            &["branch", "-u", "origin/master"],
        )
        .expect(failed_message::GIT_BRANCH);
    }
    let input_path = path.clone().into_os_string().into_string().unwrap();
    // execute cli init function with path
    execute_cargo_cmd("mgit", &["snapshot", &input_path, "--branch"]);

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(input_path + "/.gitrepos").unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"

[[repos]]
local = "foobar-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
branch = "master"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

/// 测试内容：
///     1、运行命令 mgit snapshot <path> --force --config <path>
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、commit
///     3、根目录是仓库
#[test]
fn cli_snapshot_force() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/test_snapshot_force");
    std::fs::create_dir_all(path.clone()).unwrap();

    create_repos_tree3(&path);

    let input_path = path.clone().into_os_string().into_string().unwrap();
    let config_file = input_path.clone() + "/.gitrepos";
    // execute cli init function with path
    execute_cargo_cmd(
        "mgit",
        &["snapshot", &input_path, "--force", "--config", &config_file],
    );

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(config_file).unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "."
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "8d90314117b4cb86abb6c4d55130437c6d87a30d"

[[repos]]
local = "foobar-1/foobar-1-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"

[[repos]]
local = "foobar-1/foobar-1-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"

[[repos]]
local = "foobar-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "8d90314117b4cb86abb6c4d55130437c6d87a30d"

[[repos]]
local = "foobar-2/foobar-2-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"

[[repos]]
local = "foobar-2/foobar-2-2"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

/// 测试内容：
///     1、运行命令 mgit snapshot <path> --ignore <path> --ignore <path>
///     2、抓取 path 下的所有仓库信息到配置文件 (.gitrepos)
///        仓库信息为 local、remote、commit
///     3、根目录是仓库
#[test]
fn cli_snapshot_ignore() {
    let path = env::current_dir()
        .unwrap()
        .join("target/tmp/test_snapshot_ignore");
    std::fs::create_dir_all(path.clone()).unwrap();

    create_repos_tree3(&path);

    let input_path = path.clone().into_os_string().into_string().unwrap();
    let config_file = input_path.clone() + "/.gitrepos";
    // execute cli init function with path
    execute_cargo_cmd(
        "mgit",
        &[
            "snapshot",
            &input_path,
            "--force",
            "--ignore",
            ".",
            "--ignore",
            "foobar-1/foobar-1-2",
            "--ignore",
            "foobar-2",
            "--ignore",
            "foobar-2/foobar-2-2",
        ],
    );

    // get content from .gitrepos
    let real_result = std::fs::read_to_string(config_file).unwrap();
    let expect_result = r#"
# This file is automatically @generated by mgit.
# Editing it as you wish.
default-branch = "develop"

[[repos]]
local = "foobar-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "8d90314117b4cb86abb6c4d55130437c6d87a30d"

[[repos]]
local = "foobar-1/foobar-1-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"

[[repos]]
local = "foobar-2/foobar-2-1"
remote = "https://gitee.com/ForthEspada/CS-Books.git"
commit = "1e835f92604ee5d0b37fc32ea7694d57ff19815e"
"#;

    assert_eq!(real_result.trim(), expect_result.trim());

    // clean-up
    std::fs::remove_dir_all(&path).unwrap();
}

pub fn create_repos_tree1(path: &PathBuf) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
    std::fs::create_dir_all(path.clone()).unwrap();

    let remote = "https://gitee.com/ForthEspada/CS-Books.git";

    let repo_names = ["foobar-1", "foobar-2"];

    for idx in 0..repo_names.len() {
        let dir = path.join(repo_names[idx]);
        std::fs::create_dir_all(dir.to_path_buf()).unwrap();

        // create local git repositoris
        execute_cmd(&dir, "git", &["init"]).expect(failed_message::GIT_INIT);

        // add remote
        execute_cmd(&dir, "git", &["remote", "add", "origin", remote])
            .expect(failed_message::GIT_ADD_REMOTE);

        std::fs::write(
            dir.join(".git/refs/heads/master"),
            "8d90314117b4cb86abb6c4d55130437c6d87a30d",
        )
        .unwrap();
    }
}

pub fn create_repos_tree2(path: &PathBuf) {
    create_repos_tree1(path);

    // set root git init
    execute_cmd(path, "git", &["init"]).expect(failed_message::GIT_INIT);
    let root_remote = "https://gitee.com/ForthEspada/CS-Books.git";
    execute_cmd(path, "git", &["remote", "add", "origin", root_remote])
        .expect(failed_message::GIT_ADD_REMOTE);

    std::fs::write(
        path.join(".git/refs/heads/master"),
        "1e835f92604ee5d0b37fc32ea7694d57ff19815e",
    )
    .unwrap();
}

pub fn create_repos_tree3(path: &PathBuf) {
    // set root git init
    create_repos_tree1(path);

    let remote = "https://gitee.com/ForthEspada/CS-Books.git";

    // get all dir
    for it in std::fs::read_dir(path).unwrap() {
        let dir_entry = match it {
            Ok(dir) => dir,
            Err(e) => {
                println!("{}", e);
                continue;
            }
        };
        let entry_path = &dir_entry.path();
        let entry_name = &entry_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // init repo randomly
        for idx in 1..3 {
            let repo_name = format!("{}-{}", entry_name, idx);
            let dir = entry_path.join(&repo_name);

            std::fs::create_dir_all(dir.to_path_buf()).unwrap();
            // create local git repositoris
            execute_cmd(&dir, "git", &["init"]).expect(failed_message::GIT_INIT);

            // add remote
            execute_cmd(&dir, "git", &["remote", "add", "origin", remote])
                .expect(failed_message::GIT_ADD_REMOTE);

            std::fs::write(
                dir.join(".git/refs/heads/master"),
                "1e835f92604ee5d0b37fc32ea7694d57ff19815e",
            )
            .unwrap();
        }
    }

    // set root git init
    execute_cmd(path, "git", &["init"]).expect(failed_message::GIT_INIT);
    execute_cmd(path, "git", &["remote", "add", "origin", remote])
        .expect(failed_message::GIT_ADD_REMOTE);
    std::fs::write(
        path.join(".git/refs/heads/master"),
        "1e835f92604ee5d0b37fc32ea7694d57ff19815e",
    )
    .unwrap();
}
