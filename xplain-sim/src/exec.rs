//! Synchronous real IO for the effects that touch git and the file system. Mirrors `xplain-app`
//! (`git`, `fsio`, `config_io`, `token`) without tokio, and runs git with a hermetic environment.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use xplain_core::config::{ConfigChange, ConfigSaveError, apply_patch};
use xplain_core::diff::{DiffSpec, RawDiff};
use xplain_core::errors::IoReason;
use xplain_core::integration::CommandOutput;
use xplain_core::mcp::{TokenPlan, plan_token};
use xplain_core::messages::{cannot_open_directory, cannot_run_git, cannot_write};

use crate::fixture::git_command;

const MAX_UNTRACKED: u64 = 1024 * 1024;

/// Where relative paths and the app cwd resolve to, and git's fake home.
pub(crate) struct Env<'a> {
    pub cwd: &'a Path,
    pub home: &'a Path,
}

impl Env<'_> {
    pub fn resolve(&self, p: Option<&str>) -> PathBuf {
        match p {
            Some(p) => self.cwd.join(p),
            None => self.cwd.to_path_buf(),
        }
    }

    fn git(&self, args: &[String], cwd: &Path, ok_codes: &[i32]) -> Result<String, String> {
        let out = git_command(cwd, self.home).args(args).output().map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                cannot_run_git(IoReason::NotFound)
            } else {
                format!("cannot run git: {}", IoReason::from_io_error(&e).as_str())
            }
        })?;
        let code = out.status.code().unwrap_or(-1);
        let o = CommandOutput {
            code,
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
        if ok_codes.contains(&o.code) {
            return Ok(o.stdout);
        }
        Err(if !o.stderr.is_empty() {
            o.stderr
        } else if o.code >= 0 {
            format!("git failed (exit {})", o.code)
        } else {
            "git failed".to_string()
        })
    }

    pub fn load_diff(&self, spec: &DiffSpec) -> Result<RawDiff, String> {
        let cwd = self.resolve(spec.cwd.as_deref());
        if let Some(d) = spec.cwd.as_deref() {
            let reason = match std::fs::metadata(&cwd) {
                Ok(m) if m.is_dir() => None,
                Ok(_) => Some(IoReason::NotDirectory),
                Err(e) => Some(IoReason::from_io_error(&e)),
            };
            if let Some(r) = reason {
                return Err(cannot_open_directory(d, r));
            }
        }
        let tracked = self.git(&spec.git_argv(), &cwd, &[0])?;
        let untracked = if spec.wants_untracked() { self.untracked(spec, &cwd) } else { Vec::new() };
        Ok(RawDiff { tracked, untracked })
    }

    fn untracked(&self, spec: &DiffSpec, cwd: &Path) -> Vec<String> {
        let args: Vec<String> =
            ["ls-files", "--others", "--exclude-standard", "-z"].map(String::from).to_vec();
        let Ok(listing) = self.git(&args, cwd, &[0]) else { return Vec::new() };
        listing
            .split('\0')
            .filter(|n| !n.is_empty())
            .filter_map(|name| {
                let md = std::fs::symlink_metadata(cwd.join(name)).ok()?;
                if !md.is_file() || md.len() > MAX_UNTRACKED {
                    return None;
                }
                self.git(&spec.untracked_argv(name), cwd, &[0, 1]).ok()
            })
            .collect()
    }

    pub fn list_files(&self, cwd: Option<&str>) -> Vec<String> {
        let args: Vec<String> =
            ["ls-files", "--cached", "--others", "--exclude-standard", "-z"].map(String::from).to_vec();
        match self.git(&args, &self.resolve(cwd), &[0]) {
            Ok(out) => {
                let set: BTreeSet<&str> = out.split('\0').filter(|l| !l.is_empty()).collect();
                set.into_iter().map(str::to_string).collect()
            }
            Err(_) => Vec::new(),
        }
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, IoReason> {
        std::fs::read(self.cwd.join(path)).map_err(|e| IoReason::from_io_error(&e))
    }

    pub fn write_export(&self, path: &str, contents: &str) -> Result<(), IoReason> {
        std::fs::write(self.cwd.join(path), contents).map_err(|e| IoReason::from_io_error(&e))
    }

    pub fn save_config(&self, path: &str, change: &ConfigChange) -> Result<(), ConfigSaveError> {
        let path = self.cwd.join(path);
        let existing = match std::fs::read(&path) {
            Ok(b) => Some(String::from_utf8_lossy(&b).into_owned()),
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => None,
            Err(_) => return Err(ConfigSaveError::Unreadable),
        };
        let out = apply_patch(existing.as_deref(), &change.to_patch())?;
        let io = |e: std::io::Error| ConfigSaveError::Io(IoReason::from_io_error(&e));
        if let Some(parent) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                if e.kind() == ErrorKind::AlreadyExists {
                    ConfigSaveError::Io(IoReason::NotDirectory)
                } else {
                    io(e)
                }
            })?;
        }
        atomic_write(&path, out.as_bytes(), None).map_err(io)
    }
}

fn atomic_write(path: &Path, contents: &[u8], mode: Option<u32>) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let res = (|| {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        if let Some(m) = mode {
            opts.mode(m);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(contents)?;
        f.flush()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// F-MCPSRV-01: reuse a valid token file or write a new one (dir 0700, file 0600). `random` seeds a new token.
pub(crate) fn ensure_token(state_dir: &str, random: [u8; 32]) -> Result<String, String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if state_dir.is_empty() {
        return Err(cannot_write("mcp.json", IoReason::NotFound));
    }
    let dir = Path::new(state_dir);
    let file = dir.join("mcp.json");
    let existing = std::fs::read(&file).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
    match plan_token(existing.as_deref(), random) {
        TokenPlan::Reuse(token) => Ok(token),
        TokenPlan::Write { token, file_contents } => {
            let write = || -> Result<(), IoReason> {
                let r = |e: std::io::Error| IoReason::from_io_error(&e);
                std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(r)?;
                atomic_write(&file, file_contents.as_bytes(), Some(0o600)).map_err(r)?;
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).map_err(r)
            };
            write().map_err(|reason| cannot_write(&file.display().to_string(), reason))?;
            Ok(token)
        }
    }
}
