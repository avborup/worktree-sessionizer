use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};

use tempfile::TempDir;
use wt::{CreateWorktree, RemoveOptions, Wt, git};

fn git_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(["-C", &dir.to_string_lossy()])
        .args(["-c", "user.email=wt@test", "-c", "user.name=wt"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("running git")
}

fn git_ok(dir: &Path, args: &[&str]) -> String {
    let out = git_in(dir, args);
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    git_in(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .status
    .success()
}

/// A temp `WT_HOME` whose config points at a temp repo with one commit, an
/// ignored `node_modules/x` (cow cloned) and `.eslintcache` (symlinked).
///
/// Project `app` is the normal one. Project `broken` symlinks the tracked
/// `README.md`, which always fails once the worktree exists, to exercise
/// rollback.
struct Fixture {
    _tmp: TempDir,
    home: PathBuf,
    repo: PathBuf,
    root: PathBuf,
    wt: Wt,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().join("home");
        let repo = tmp.path().join("repo");
        let root = tmp.path().join("worktrees");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(repo.join("node_modules")).unwrap();

        git_ok(&repo, &["init", "-q", "-b", "main"]);
        // Branching from a local branch then records an upstream, so
        // `create` has one to unset.
        git_ok(&repo, &["config", "branch.autoSetupMerge", "always"]);
        std::fs::write(repo.join("README.md"), "hi").unwrap();
        std::fs::write(repo.join(".gitignore"), "node_modules/\n.eslintcache\n").unwrap();
        std::fs::write(repo.join("node_modules/x"), "dep").unwrap();
        std::fs::write(repo.join(".eslintcache"), "cache").unwrap();
        git_ok(&repo, &["add", "-A"]);
        git_ok(&repo, &["commit", "-qm", "init"]);

        let config = format!(
            "worktrees_root = '{root}'\n\
             prune_after_days = 30\n\
             \n\
             [[projects]]\n\
             name = 'app'\n\
             repo_path = '{repo}'\n\
             cow_clone = ['node_modules']\n\
             symlink = ['.eslintcache']\n\
             \n\
             [[projects]]\n\
             name = 'broken'\n\
             repo_path = '{repo}'\n\
             symlink = ['README.md']\n",
            root = root.display(),
            repo = repo.display(),
        );
        std::fs::write(home.join("config.toml"), config).unwrap();

        let wt = Wt::at(&home);
        Self {
            _tmp: tmp,
            home,
            repo,
            root,
            wt,
        }
    }

    fn state_toml(&self) -> Option<String> {
        std::fs::read_to_string(self.home.join("state.toml")).ok()
    }

    fn worktree_count(&self) -> usize {
        git_ok(&self.repo, &["worktree", "list", "--porcelain"])
            .lines()
            .filter(|l| l.starts_with("worktree "))
            .count()
    }
}

fn req(project: &str, branch: &str, base: Option<&str>) -> CreateWorktree {
    CreateWorktree {
        project: project.to_string(),
        branch: branch.to_string(),
        base: base.map(str::to_string),
        session_name: format!("s-{}", branch.replace('/', "-")),
    }
}

#[test]
fn create_new_branch_from_base() {
    let f = Fixture::new();

    let entry = f.wt.create(&req("app", "feat/x", Some("main"))).unwrap();

    assert_eq!(entry.path, f.root.join("app").join("feat-x"));
    assert_eq!(entry.project, "app");
    assert_eq!(entry.branch, "feat/x");
    assert_eq!(entry.session_name, "s-feat-x");
    assert_eq!(entry.base_branch.as_deref(), Some("main"));
    assert_eq!(
        git_ok(&entry.path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "feat/x"
    );

    let modules = std::fs::symlink_metadata(entry.path.join("node_modules")).unwrap();
    assert!(modules.is_dir(), "node_modules cloned, not linked");
    assert_eq!(
        std::fs::read_to_string(entry.path.join("node_modules/x")).unwrap(),
        "dep"
    );
    let cache = std::fs::symlink_metadata(entry.path.join(".eslintcache")).unwrap();
    assert!(cache.file_type().is_symlink(), ".eslintcache symlinked");

    let upstream = git_in(
        &entry.path,
        &["rev-parse", "--abbrev-ref", "feat/x@{upstream}"],
    );
    assert!(!upstream.status.success(), "upstream unset");

    let state = f.wt.state().unwrap();
    assert_eq!(state.worktrees.len(), 1);
    assert_eq!(state.worktrees[0].id, entry.id);
    assert_eq!(state.worktrees[0].path, entry.path);
}

#[test]
fn create_checks_out_existing_branch() {
    let f = Fixture::new();
    git_ok(&f.repo, &["checkout", "-q", "-b", "existing"]);
    std::fs::write(f.repo.join("more.txt"), "more").unwrap();
    git_ok(&f.repo, &["add", "-A"]);
    git_ok(&f.repo, &["commit", "-qm", "more"]);
    git_ok(&f.repo, &["checkout", "-q", "main"]);
    let head = git_ok(&f.repo, &["rev-parse", "existing"]);

    let entry = f.wt.create(&req("app", "existing", None)).unwrap();

    assert_eq!(entry.path, f.root.join("app").join("existing"));
    assert_eq!(entry.base_branch, None);
    assert_eq!(git_ok(&entry.path, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git_ok(&entry.path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "existing"
    );
    assert_eq!(f.wt.state().unwrap().worktrees.len(), 1);
}

#[test]
fn create_failure_leaves_nothing() {
    let f = Fixture::new();
    f.wt.create(&req("app", "feat/ok", Some("main"))).unwrap();
    let before = f.state_toml();

    let err =
        f.wt.create(&req("app", "feat/z", Some("nope")))
            .expect_err("unknown base must fail");

    assert!(
        format!("{err:#}").contains("nope"),
        "error names the base: {err:#}"
    );
    assert!(
        !f.root.join("app").join("feat-z").exists(),
        "no worktree dir"
    );
    assert!(!branch_exists(&f.repo, "feat/z"), "no branch");
    assert_eq!(f.state_toml(), before, "state unchanged");
    assert_eq!(f.worktree_count(), 2, "only the repo and feat/ok");
}

#[test]
fn create_rolls_back_new_branch_when_setup_fails() {
    let f = Fixture::new();

    f.wt.create(&req("broken", "feat/r", Some("main")))
        .expect_err("symlinking over a tracked file must fail");

    assert!(
        !f.root.join("broken").join("feat-r").exists(),
        "worktree dir removed"
    );
    assert!(
        !branch_exists(&f.repo, "feat/r"),
        "the branch it created is deleted"
    );
    assert_eq!(f.worktree_count(), 1, "worktree unregistered from git");
    assert_eq!(f.state_toml(), None, "nothing saved");
}

#[test]
fn create_rollback_keeps_existing_branch() {
    let f = Fixture::new();
    git_ok(&f.repo, &["branch", "keep"]);

    f.wt.create(&req("broken", "keep", None))
        .expect_err("symlinking over a tracked file must fail");

    assert!(
        !f.root.join("broken").join("keep").exists(),
        "worktree dir removed"
    );
    assert!(
        branch_exists(&f.repo, "keep"),
        "a branch it did not create stays"
    );
    assert_eq!(f.worktree_count(), 1, "worktree unregistered from git");
    assert_eq!(f.state_toml(), None, "nothing saved");
}

#[test]
fn create_refuses_existing_path() {
    let f = Fixture::new();
    let target = f.root.join("app").join("feat-y");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("mine.txt"), "keep me").unwrap();

    let err =
        f.wt.create(&req("app", "feat/y", Some("main")))
            .expect_err("existing path must be refused");

    assert!(err.to_string().contains("already exists"), "{err:#}");
    assert_eq!(
        std::fs::read_to_string(target.join("mine.txt")).unwrap(),
        "keep me"
    );
    assert!(!branch_exists(&f.repo, "feat/y"), "no branch");
    assert_eq!(f.state_toml(), None);
}

#[test]
fn create_refuses_unknown_project() {
    let f = Fixture::new();

    let err =
        f.wt.create(&req("ghost", "feat/g", Some("main")))
            .expect_err("unknown project must be refused");

    assert!(err.to_string().contains("ghost"), "{err:#}");
    assert!(!branch_exists(&f.repo, "feat/g"));
    assert_eq!(f.state_toml(), None);
}

#[test]
fn remove_keeps_branch_unless_asked() {
    let f = Fixture::new();
    let kept = f.wt.create(&req("app", "feat/a", Some("main"))).unwrap();
    let dropped = f.wt.create(&req("app", "feat/b", Some("main"))).unwrap();

    f.wt.remove(&kept.id, RemoveOptions::default()).unwrap();

    assert!(!kept.path.exists(), "worktree dir removed");
    assert!(branch_exists(&f.repo, "feat/a"), "branch kept by default");
    let ids: Vec<_> =
        f.wt.state()
            .unwrap()
            .worktrees
            .into_iter()
            .map(|e| e.id)
            .collect();
    assert_eq!(ids, vec![dropped.id.clone()]);

    f.wt.remove(
        &dropped.id,
        RemoveOptions {
            delete_branch: true,
            force: false,
        },
    )
    .unwrap();

    assert!(!dropped.path.exists());
    assert!(
        !branch_exists(&f.repo, "feat/b"),
        "branch deleted when asked"
    );
    assert!(f.wt.state().unwrap().worktrees.is_empty());
}

#[test]
fn remove_refuses_dirty_unless_force() {
    let f = Fixture::new();
    let entry = f.wt.create(&req("app", "feat/d", Some("main"))).unwrap();
    std::fs::write(entry.path.join("scratch.txt"), "untracked").unwrap();

    f.wt.remove(&entry.id, RemoveOptions::default())
        .expect_err("dirty worktree must not be removed");

    assert!(
        entry.path.join("scratch.txt").exists(),
        "worktree left in place"
    );
    assert_eq!(f.wt.state().unwrap().worktrees.len(), 1, "entry kept");

    f.wt.remove(
        &entry.id,
        RemoveOptions {
            delete_branch: false,
            force: true,
        },
    )
    .unwrap();

    assert!(!entry.path.exists());
    assert!(f.wt.state().unwrap().worktrees.is_empty());
}

#[test]
fn remove_refuses_unknown_id() {
    let f = Fixture::new();

    let err =
        f.wt.remove("01NOPE", RemoveOptions::default())
            .expect_err("unknown id must fail");

    assert!(err.to_string().contains("01NOPE"), "{err:#}");
}

#[test]
fn touch_bumps_last_used_at() {
    let f = Fixture::new();
    let entry = f.wt.create(&req("app", "feat/t", Some("main"))).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));

    f.wt.touch(&entry.id).unwrap();

    let saved = &f.wt.state().unwrap().worktrees[0];
    assert!(saved.last_used_at > entry.last_used_at);
    assert_eq!(saved.created_at, entry.created_at);
}

#[test]
fn concurrent_creates_keep_both_entries() {
    let f = Fixture::new();
    let barrier = Arc::new(Barrier::new(2));

    let handles: Vec<_> = ["feat/one", "feat/two"]
        .into_iter()
        .map(|branch| {
            let home = f.home.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let wt = Wt::at(home);
                barrier.wait();
                wt.create(&req("app", branch, Some("main")))
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    let mut branches: Vec<_> =
        f.wt.state()
            .unwrap()
            .worktrees
            .into_iter()
            .map(|e| e.branch)
            .collect();
    branches.sort();
    assert_eq!(branches, vec!["feat/one", "feat/two"]);
}

#[test]
fn mutate_persists_under_the_home() {
    let f = Fixture::new();
    let entry = f.wt.create(&req("app", "feat/m", Some("main"))).unwrap();

    let removed =
        f.wt.mutate(|state| {
            state.remove(&entry.id);
            Ok(state.worktrees.len())
        })
        .unwrap();

    assert_eq!(removed, 0);
    assert!(f.wt.state().unwrap().worktrees.is_empty());
    assert_eq!(f.wt.state_path(), f.home.join("state.toml"));
    assert!(
        f.home.join("state.lock").exists(),
        "lock lives next to state"
    );
}

#[test]
fn from_env_uses_wt_home() {
    let tmp = tempfile::tempdir().unwrap();
    // No other test reads WT_HOME: they all use `Wt::at`.
    unsafe { std::env::set_var("WT_HOME", tmp.path()) };
    let wt = Wt::from_env();
    unsafe { std::env::remove_var("WT_HOME") };

    assert_eq!(wt.home(), tmp.path());
    assert_eq!(wt.state_path(), tmp.path().join("state.toml"));
    assert_ne!(
        Wt::from_env().home(),
        tmp.path(),
        "falls back without WT_HOME"
    );
}

#[test]
fn git_helpers() {
    let f = Fixture::new();
    assert!(git::branch_exists(&f.repo, "main").unwrap());
    assert!(!git::branch_exists(&f.repo, "nope").unwrap());

    assert!(
        !git::is_dirty(&f.repo).unwrap(),
        "ignored files don't count"
    );
    std::fs::write(f.repo.join("new.txt"), "untracked").unwrap();
    assert!(git::is_dirty(&f.repo).unwrap(), "untracked files count");

    assert_eq!(
        git::default_branch(&f.repo).unwrap(),
        "main",
        "no origin falls back"
    );

    let tmp = tempfile::tempdir().unwrap();
    let upstream = tmp.path().join("upstream");
    std::fs::create_dir(&upstream).unwrap();
    git_ok(&upstream, &["init", "-q", "-b", "trunk"]);
    git_ok(&upstream, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let clone = tmp.path().join("clone");
    git_ok(
        tmp.path(),
        &[
            "clone",
            "-q",
            &upstream.to_string_lossy(),
            &clone.to_string_lossy(),
        ],
    );
    assert_eq!(git::default_branch(&clone).unwrap(), "trunk");
}
