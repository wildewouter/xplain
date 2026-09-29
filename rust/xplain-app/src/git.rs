//! Runs git for diff loading and file listing.
//!
//! Spec: F-MODE-01 (argv from `DiffSpec::git_argv`), F-MODE-02 (untracked: `git ls-files --others
//! --exclude-standard -z`, regular files only (not symlinks/dirs), size <= 1048576, unreadable skipped
//! silently, order kept, per-file `DiffSpec::untracked_argv`), F-MODE-04 (error text: git stderr
//! verbatim; `cannot run git: <reason>`; error always from the tracked `git diff`), F-CLI-06 (`--cwd`
//! checked before every load, git not spawned: `cannot open directory <dir>: <reason>`), F-SEARCH-01 /
//! F-FILES (`ListFiles`: `git ls-files --cached --others --exclude-standard`, sorted unique, failure
//! -> empty), UNSPEC-37 (git timeout 60 s).
//! Owner: component B (startup/IO).
//! Must not: parse diffs (core `parse_raw`), decide when to load, modify index/worktree.
//! `git diff` exit code 1 (with `--no-index`) is success; non-zero with empty stdout and stderr text is error.

use std::collections::BTreeSet;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use xplain_core::diff::{DiffSpec, RawDiff};
use xplain_core::errors::{IoReason, fail_msg};

const MAX_UNTRACKED: u64 = 1024 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(60);
const PARALLEL: usize = 16;

/// Run `git <args>` in `cwd`. `ok_codes` are accepted exit codes. `Err` = final message text.
async fn git(args: &[String], cwd: Option<&str>, ok_codes: &[i32]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let child = cmd.spawn().map_err(|e| fail_msg("cannot run git", IoReason::from_io_error(&e)))?;
    let out = match tokio::time::timeout(GIT_TIMEOUT, child.wait_with_output()).await {
        Err(_) => return Err("git failed".to_string()),
        Ok(Err(e)) => return Err(fail_msg("cannot run git", IoReason::from_io_error(&e))),
        Ok(Ok(o)) => o,
    };
    let code = out.status.code();
    if code.is_some_and(|c| ok_codes.contains(&c)) {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Err(if !stderr.is_empty() {
        stderr
    } else if let Some(c) = code {
        format!("git failed (exit {c})")
    } else {
        "git failed".to_string()
    })
}

/// `--cwd` must be an existing directory before git is spawned (F-CLI-06).
async fn check_dir(dir: &str) -> Result<(), String> {
    let reason = match tokio::fs::metadata(dir).await {
        Ok(m) if m.is_dir() => return Ok(()),
        Ok(_) => IoReason::NotDirectory,
        Err(e) => IoReason::from_io_error(&e),
    };
    Err(fail_msg(&format!("cannot open directory {dir}"), reason))
}

/// One untracked file as an all-added diff; `None` = skipped (symlink, dir, big, unreadable, git failure).
async fn untracked_one(spec: &DiffSpec, name: &str) -> Option<String> {
    let path = match spec.cwd.as_deref() {
        Some(c) => format!("{c}/{name}"),
        None => name.to_string(),
    };
    let md = tokio::fs::symlink_metadata(&path).await.ok()?;
    if !md.is_file() || md.len() > MAX_UNTRACKED {
        return None;
    }
    git(&spec.untracked_argv(name), spec.cwd.as_deref(), &[0, 1]).await.ok()
}

async fn untracked(spec: &DiffSpec) -> Vec<String> {
    let args: Vec<String> = ["ls-files", "--others", "--exclude-standard", "-z"].map(String::from).to_vec();
    let Ok(listing) = git(&args, spec.cwd.as_deref(), &[0]).await else {
        return Vec::new();
    };
    let names: Vec<String> = listing.split('\0').filter(|n| !n.is_empty()).map(str::to_string).collect();
    let mut out = Vec::new();
    for chunk in names.chunks(PARALLEL) {
        let handles: Vec<_> = chunk
            .iter()
            .map(|n| {
                let spec = spec.clone();
                let n = n.clone();
                tokio::spawn(async move { untracked_one(&spec, &n).await })
            })
            .collect();
        for h in handles {
            if let Ok(Some(t)) = h.await {
                out.push(t);
            }
        }
    }
    out
}

/// Executes `Effect::LoadDiff`. `Err` = final error-screen text.
pub async fn load_diff(spec: &DiffSpec) -> Result<RawDiff, String> {
    if let Some(d) = spec.cwd.as_deref() {
        check_dir(d).await?;
    }
    let argv = spec.git_argv();
    let cwd = spec.cwd.as_deref();
    let (tracked, extra) = if spec.wants_untracked() {
        let (t, u) = tokio::join!(git(&argv, cwd, &[0]), untracked(spec));
        (t?, u)
    } else {
        (git(&argv, cwd, &[0]).await?, Vec::new())
    };
    Ok(RawDiff { tracked, untracked: extra })
}

/// Executes `Effect::ListFiles`; never fails (empty list on error), sorted, unique.
pub async fn list_files(cwd: Option<&str>) -> Vec<String> {
    let args: Vec<String> =
        ["ls-files", "--cached", "--others", "--exclude-standard"].map(String::from).to_vec();
    match git(&args, cwd, &[0]).await {
        Ok(out) => {
            let set: BTreeSet<&str> = out.split('\n').filter(|l| !l.is_empty()).collect();
            set.into_iter().map(str::to_string).collect()
        }
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use xplain_core::options::DiffMode;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xplain-git-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sh(d: &Path, args: &[&str]) {
        let st = std::process::Command::new("git")
            .args(["-c", "user.email=a@b", "-c", "user.name=t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(d)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    }

    fn repo(name: &str) -> PathBuf {
        let d = tmp(name);
        sh(&d, &["init", "-q"]);
        std::fs::write(d.join("a.txt"), "one\n").unwrap();
        sh(&d, &["add", "a.txt"]);
        sh(&d, &["commit", "-q", "-m", "i"]);
        d
    }

    fn spec(d: &Path, mode: DiffMode, args: &[&str]) -> DiffSpec {
        DiffSpec {
            cwd: Some(d.to_str().unwrap().to_string()),
            mode,
            full: false,
            git_args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn bad_cwd_no_spawn() {
        let d = tmp("cwd");
        let missing = d.join("nope");
        let s = spec(&missing, DiffMode::All, &[]);
        let e = load_diff(&s).await.unwrap_err();
        assert_eq!(e, format!("cannot open directory {}: not found", missing.display()));
        let f = d.join("f");
        std::fs::write(&f, "x").unwrap();
        let e = load_diff(&spec(&f, DiffMode::All, &[])).await.unwrap_err();
        assert_eq!(e, format!("cannot open directory {}: not a directory", f.display()));
    }

    #[tokio::test]
    async fn tracked_and_untracked() {
        let d = repo("load");
        std::fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(d.join("new.txt"), "hello\n").unwrap();
        std::fs::write(d.join("big.bin"), vec![b'x'; 1_048_577]).unwrap();
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub/z.txt"), "z\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("a.txt", d.join("link")).unwrap();
        let r = load_diff(&spec(&d, DiffMode::All, &[])).await.unwrap();
        assert!(r.tracked.contains("+two"));
        assert_eq!(r.untracked.len(), 2, "{:?}", r.untracked);
        assert!(r.untracked[0].contains("new.txt") && r.untracked[0].contains("+hello"));
        assert!(r.untracked[1].contains("sub/z.txt"));
        let r = load_diff(&spec(&d, DiffMode::Unstaged, &[])).await.unwrap();
        assert!(r.untracked.is_empty() && r.tracked.contains("+two"));
        let r = load_diff(&spec(&d, DiffMode::All, &["HEAD"])).await.unwrap();
        assert!(r.untracked.is_empty());
    }

    #[tokio::test]
    async fn git_error_is_stderr() {
        let d = tmp("nogit");
        let e = load_diff(&spec(&d, DiffMode::All, &[])).await.unwrap_err();
        assert!(!e.is_empty() && !e.starts_with("cannot run git"), "{e}");
        let r = repo("badarg");
        let e = load_diff(&spec(&r, DiffMode::Staged, &["no-such-ref"])).await.unwrap_err();
        assert!(e.contains("no-such-ref"), "{e}");
    }

    #[tokio::test]
    async fn list_sorted_unique() {
        let d = repo("list");
        std::fs::write(d.join("b.txt"), "b").unwrap();
        std::fs::write(d.join("a.txt"), "changed").unwrap();
        let l = list_files(d.to_str()).await;
        assert_eq!(l, ["a.txt", "b.txt"]);
        assert!(list_files(tmp("nolist").to_str()).await.is_empty());
        assert!(list_files(Some("/definitely/not/here")).await.is_empty());
    }
}
