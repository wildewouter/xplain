//! Locate or download a pinned zig toolchain into target/tools/.

use crate::{Res, log};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const ZIG_VERSION: &str = "0.15.2";
const ZIG_URL: &str = "https://ziglang.org/download/0.15.2/zig-aarch64-macos-0.15.2.tar.xz";
const ZIG_SHA256: &str = "3cc2bab367e185cdfb27501c4b30b1b0653c28d9f73df8dc91488e66ece5fa6b";
const ZIG_TOP_DIR: &str = "zig-aarch64-macos-0.15.2";

fn version_of(zig: &Path) -> Option<String> {
    let out = Command::new(zig).arg("version").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Returns the zig executable to use: PATH zig if version matches, else the downloaded pin.
pub fn ensure(root: &Path) -> Res<PathBuf> {
    if version_of(Path::new("zig")).as_deref() == Some(ZIG_VERSION) {
        log(&format!("zig {ZIG_VERSION} found on PATH"));
        return Ok(PathBuf::from("zig"));
    }
    let dir = root.join("target/tools").join(format!("zig-{ZIG_VERSION}"));
    let bin = dir.join("zig");
    if version_of(&bin).as_deref() == Some(ZIG_VERSION) {
        log(&format!("zig {ZIG_VERSION} cached at {}", bin.display()));
        return Ok(bin);
    }
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err(format!(
            "no zig {ZIG_VERSION} on PATH, and the pinned download is macOS aarch64 only. \
             Install zig {ZIG_VERSION} from https://ziglang.org/download/ and put it on PATH."
        ));
    }
    let tools = root.join("target/tools");
    fs::create_dir_all(&tools).map_err(|e| format!("create {}: {e}", tools.display()))?;
    let archive = tools.join(format!("zig-{ZIG_VERSION}.tar.xz"));
    log(&format!("downloading zig {ZIG_VERSION} from {ZIG_URL}"));
    download(ZIG_URL, &archive)?;
    log("verifying sha256");
    let got = sha256_file(&archive)?;
    if got != ZIG_SHA256 {
        let _ = fs::remove_file(&archive);
        return Err(format!(
            "zig archive sha256 mismatch: expected {ZIG_SHA256}, got {got}. Removed the download; retry."
        ));
    }
    log("extracting zig");
    let _ = fs::remove_dir_all(&dir);
    let f = fs::File::open(&archive).map_err(|e| e.to_string())?;
    tar::Archive::new(xz2::read::XzDecoder::new(f))
        .unpack(&tools)
        .map_err(|e| format!("extract zig: {e}"))?;
    fs::rename(tools.join(ZIG_TOP_DIR), &dir).map_err(|e| format!("rename zig dir: {e}"))?;
    let _ = fs::remove_file(&archive);
    match version_of(&bin) {
        Some(v) if v == ZIG_VERSION => Ok(bin),
        other => Err(format!("extracted zig reports {other:?}, expected {ZIG_VERSION}")),
    }
}

fn download(url: &str, to: &Path) -> Res<()> {
    let resp = ureq::get(url).call().map_err(|e| format!("download {url}: {e}. Check network access."))?;
    let mut r = resp.into_reader();
    let mut f = fs::File::create(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| format!("download: {e}"))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        total += n as u64;
    }
    log(&format!("downloaded {} MiB", total >> 20));
    Ok(())
}

pub fn sha256_file(p: &Path) -> Res<String> {
    let mut f = fs::File::open(p).map_err(|e| format!("open {}: {e}", p.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
