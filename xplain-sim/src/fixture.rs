//! Temp dirs, fixture repos and isolated git invocations.
//!
//! `standard` = git repo with HEAD = `fixtures/base`, work tree = `fixtures/work`.
//! `empty` = git repo with one empty commit. `nogit` = plain empty dir. The standard repo is built once per
//! fixture content (cached under the OS temp dir, keyed by a content hash) and copied per Sim, so a Sim costs a
//! directory copy, not four git processes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fixture {
    #[default]
    Standard,
    Empty,
    NoGit,
}

/// A temp directory removed on drop (kept when `XPLAIN_SIM_KEEP` is set).
#[derive(Debug)]
pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    pub fn new() -> TempRoot {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("xplain-sim-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        must(std::fs::create_dir_all(&base), "create temp dir");
        // Canonical like a process cwd (macOS /var -> /private/var).
        let path = std::fs::canonicalize(&base).unwrap_or(base);
        TempRoot { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Default for TempRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        if std::env::var_os("XPLAIN_SIM_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) fn must<T, E: std::fmt::Display>(r: Result<T, E>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("xplain-sim: {what}: {e}"),
    }
}

/// `git` with a hermetic environment: no user or system config, fixed identity.
pub fn git_command(dir: &Path, home: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir)
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "x")
        .env("GIT_AUTHOR_EMAIL", "a@b.c")
        .env("GIT_COMMITTER_NAME", "x")
        .env("GIT_COMMITTER_EMAIL", "a@b.c")
        .env("LC_ALL", "C")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    c
}

pub(crate) fn run_git(dir: &Path, home: &Path, args: &[&str]) -> Output {
    must(git_command(dir, home).args(args).output(), "run git")
}

fn git_ok(dir: &Path, home: &Path, args: &[&str]) {
    let o = run_git(dir, home, args);
    assert!(o.status.success(), "xplain-sim: git {args:?} failed: {}", String::from_utf8_lossy(&o.stderr));
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn copy_tree(from: &Path, to: &Path) {
    must(std::fs::create_dir_all(to), "create dir");
    let st =
        must(Command::new("cp").arg("-Rp").arg(format!("{}/.", from.display())).arg(to).status(), "run cp");
    assert!(st.success(), "xplain-sim: cp {} -> {} failed", from.display(), to.display());
}

fn hash_tree(dir: &Path, h: &mut DefaultHasher) {
    let mut names: Vec<_> =
        std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    names.sort();
    for p in names {
        p.file_name().hash(h);
        if p.is_dir() {
            hash_tree(&p, h);
        } else {
            std::fs::read(&p).unwrap_or_default().hash(h);
        }
    }
}

fn standard_template() -> &'static Path {
    static T: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let root = fixture_root();
        assert!(root.join("base").is_dir(), "xplain-sim: fixture not found at {}", root.display());
        let mut h = DefaultHasher::new();
        hash_tree(&root, &mut h);
        let key = h.finish();
        let done = std::env::temp_dir().join(format!("xplain-sim-fixture-{key:016x}"));
        if done.join(".git").is_dir() {
            return done;
        }
        let build =
            std::env::temp_dir().join(format!("xplain-sim-fixture-{key:016x}.tmp{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&build);
        must(std::fs::create_dir_all(&build), "create template dir");
        let home = build.join(".no-home");
        must(std::fs::create_dir_all(&home), "create template home");
        let repo = build.join("repo");
        copy_tree(&root.join("base"), &repo);
        commit_all(&repo, &home);
        copy_tree(&root.join("work"), &repo);
        if std::fs::rename(&repo, &done).is_err() {
            // Another process won the race: use theirs.
            let _ = std::fs::remove_dir_all(&build);
            return done;
        }
        let _ = std::fs::remove_dir_all(&build);
        done
    })
}

fn commit_all(repo: &Path, home: &Path) {
    git_ok(repo, home, &["-c", "init.defaultBranch=main", "init", "-q"]);
    git_ok(repo, home, &["add", "-A"]);
    git_ok(repo, home, &["-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "init"]);
}

/// Build `kind` into `repo` (created if missing). `home` is only for git's environment.
pub fn build_fixture(kind: Fixture, repo: &Path, home: &Path) {
    must(std::fs::create_dir_all(repo), "create repo dir");
    match kind {
        Fixture::Standard => copy_tree(standard_template(), repo),
        Fixture::Empty => commit_all(repo, home),
        Fixture::NoGit => {}
    }
}
