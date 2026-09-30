//! Release build tasks. Run as `cargo xtask <dist|install>`.
//!
//! This binary doubles as the linker shim for cargo-zigbuild: the generated `zigcc-*` wrapper
//! scripts and the `ar`/`lib` symlinks re-invoke this executable (see `shim`).
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod binfmt;
mod zig;

use binfmt::Arch;
use cargo_zigbuild::{Build, Zig};
use clap::Parser;
use flate2::{Compression, write::GzEncoder};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub type Res<T> = Result<T, String>;

const MAC_ARM: &str = "aarch64-apple-darwin";
const MAC_X86: &str = "x86_64-apple-darwin";
const MAC_UNI: &str = "universal-apple-darwin";
const LINUX_X86: &str = "x86_64-unknown-linux-musl";
const LINUX_ARM: &str = "aarch64-unknown-linux-musl";
/// Order of build and output.
const DIST_TARGETS: [&str; 5] = [MAC_ARM, MAC_X86, MAC_UNI, LINUX_X86, LINUX_ARM];
const RUSTUP_TARGETS: [&str; 4] = [MAC_ARM, MAC_X86, LINUX_X86, LINUX_ARM];

pub fn log(msg: &str) {
    eprintln!("[xtask] {msg}");
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

fn main() {
    if let Some(r) = shim() {
        if let Err(e) = r {
            eprintln!("zig shim: {e}");
            std::process::exit(1);
        }
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dist") => dist(&args[1..]),
        Some("install") => install(),
        Some("__zigbuild") => zigbuild(&args[1..]),
        _ => Err("usage: cargo xtask dist [--version <v>] | cargo xtask install".into()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// cargo-zigbuild re-executes the current exe as `ar`/`lib` (symlink argv0) or `<exe> zig cc -- args`.
fn shim() -> Option<Res<()>> {
    let mut it = std::env::args();
    let prog = it.next()?;
    let stem = Path::new(&prog).file_stem()?.to_string_lossy().to_lowercase();
    let rest: Vec<String> = it.collect();
    let zig = match stem.as_str() {
        "ar" => Zig::Ar { args: rest },
        "lib" => Zig::Lib { args: rest },
        _ if rest.first().map(String::as_str) == Some("zig") => {
            let sub = rest.get(1)?.clone();
            let mut a: Vec<String> = rest.into_iter().skip(2).collect();
            if a.first().map(String::as_str) == Some("--") {
                a.remove(0);
            }
            match sub.as_str() {
                "cc" => Zig::Cc { args: a },
                "c++" => Zig::Cxx { args: a },
                "ar" => Zig::Ar { args: a },
                "ranlib" => Zig::Ranlib { args: a },
                "lib" => Zig::Lib { args: a },
                "dlltool" => Zig::Dlltool { args: a },
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(zig.execute().map_err(|e| e.to_string()))
}

fn run(cmd: &mut Command, what: &str) -> Res<()> {
    let st = cmd.status().map_err(|e| format!("{what}: cannot start ({e}). Is it installed and on PATH?"))?;
    if st.success() { Ok(()) } else { Err(format!("{what} failed ({st})")) }
}

fn capture(cmd: &mut Command, what: &str) -> Res<String> {
    let out = cmd.output().map_err(|e| format!("{what}: cannot start ({e})"))?;
    if !out.status.success() {
        return Err(format!("{what} failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn install() -> Res<()> {
    log("cargo install --path xplain-app --locked");
    run(
        Command::new("cargo").current_dir(root()).args(["install", "--path", "xplain-app", "--locked"]),
        "cargo install",
    )
}

fn app_version() -> Res<String> {
    let find = |p: PathBuf, table: &str| -> Option<String> {
        let text = fs::read_to_string(p).ok()?;
        let mut in_table = false;
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with('[') {
                in_table = l == table;
            } else if in_table && l.starts_with("version") && l.contains('"') {
                return l.split('"').nth(1).map(str::to_string);
            }
        }
        None
    };
    find(root().join("xplain-app/Cargo.toml"), "[package]")
        .or_else(|| find(root().join("Cargo.toml"), "[workspace.package]"))
        .ok_or_else(|| "cannot read version from xplain-app/Cargo.toml; pass --version".into())
}

fn ensure_targets() -> Res<()> {
    let installed = capture(Command::new("rustup").args(["target", "list", "--installed"]), "rustup")?;
    let missing: Vec<&str> =
        RUSTUP_TARGETS.into_iter().filter(|t| !installed.lines().any(|l| l.trim() == *t)).collect();
    if missing.is_empty() {
        log("rustup targets present");
        return Ok(());
    }
    log(&format!("rustup target add {}", missing.join(" ")));
    run(Command::new("rustup").args(["target", "add"]).args(&missing), "rustup target add")
}

fn bin_path(target: &str) -> PathBuf {
    root().join("target").join(target).join("release/xplain")
}

fn cargo_build(target: &str) -> Res<()> {
    run(
        Command::new("cargo").current_dir(root()).args([
            "build",
            "--release",
            "--locked",
            "-p",
            "xplain-app",
            "--bin",
            "xplain",
            "--target",
            target,
        ]),
        &format!(
            "cargo build ({target}); on macOS this needs Xcode command line tools (xcode-select --install)"
        ),
    )
}

fn linux_build(target: &str, zig: &Path) -> Res<()> {
    // The in-process library reads the zig location from the environment, and `set_var` is unsafe,
    // so run the library from a child of this same executable with the variable set.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    run(
        Command::new(exe)
            .current_dir(root())
            .env("CARGO_ZIGBUILD_ZIG_PATH", zig)
            .args(["__zigbuild", target]),
        &format!("zigbuild ({target})"),
    )
}

/// Internal: runs the cargo-zigbuild library `Build` API for one target.
fn zigbuild(a: &[String]) -> Res<()> {
    let target = a.first().ok_or("missing target")?;
    let build = Build::parse_from([
        "zigbuild",
        "--release",
        "--locked",
        "-p",
        "xplain-app",
        "--bin",
        "xplain",
        "--target",
        target,
    ]);
    build.execute().map_err(|e| format!("{e:#}"))
}

fn lipo() -> Res<()> {
    let out = root().join("target").join(MAC_UNI).join("release");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    run(
        Command::new("lipo")
            .arg("-create")
            .arg("-output")
            .arg(out.join("xplain"))
            .arg(bin_path(MAC_ARM))
            .arg(bin_path(MAC_X86)),
        "lipo (part of Xcode command line tools)",
    )
}

fn package(dist: &Path, version: &str, target: &str) -> Res<PathBuf> {
    let name = format!("xplain-{version}-{target}");
    let out = dist.join(format!("{name}.tar.gz"));
    let f = fs::File::create(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let mut tar = tar::Builder::new(GzEncoder::new(f, Compression::best()));
    let add = |tar: &mut tar::Builder<_>, src: PathBuf, dest: &str, mode: u32| -> Res<()> {
        let data = fs::read(&src).map_err(|e| format!("read {}: {e}", src.display()))?;
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(mode);
        h.set_mtime(0);
        tar.append_data(&mut h, format!("{name}/{dest}"), data.as_slice()).map_err(|e| e.to_string())
    };
    add(&mut tar, bin_path(target), "xplain", 0o755)?;
    add(&mut tar, root().join("LICENSE"), "LICENSE", 0o644)?;
    add(&mut tar, root().join("README.md"), "README.md", 0o644)?;
    tar.into_inner().and_then(|g| g.finish()).map_err(|e| e.to_string())?;
    Ok(out)
}

fn expected_archs(target: &str) -> (&'static str, Vec<Arch>) {
    match target {
        MAC_ARM => ("macho", vec![Arch::Aarch64]),
        MAC_X86 => ("macho", vec![Arch::X86_64]),
        MAC_UNI => ("macho", vec![Arch::Aarch64, Arch::X86_64]),
        LINUX_X86 => ("elf", vec![Arch::X86_64]),
        _ => ("elf", vec![Arch::Aarch64]),
    }
}

fn check_arch(target: &str) -> Res<()> {
    let p = bin_path(target);
    let bytes = fs::read(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let got = binfmt::detect(&bytes).map_err(|e| format!("{target}: {e}"))?;
    let want = expected_archs(target);
    if got != (want.0, want.1.clone()) {
        return Err(format!("{target}: binary is {got:?}, expected {want:?}"));
    }
    log(&format!("  ok {target}: {} {:?}", got.0, got.1));
    Ok(())
}

fn smoke_run() -> Res<()> {
    let host = capture(Command::new("rustc").arg("-vV"), "rustc -vV")?
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(str::to_string))
        .unwrap_or_default();
    let target = if host == MAC_ARM || host == MAC_X86 { MAC_UNI } else { host.as_str() };
    let target = if DIST_TARGETS.contains(&target) { target } else { &host };
    if !DIST_TARGETS.contains(&target) {
        log(&format!("  skip run smoke: host {host} not a dist target"));
        return Ok(());
    }
    let out = Command::new(bin_path(target))
        .arg("--help")
        .output()
        .map_err(|e| format!("run native binary: {e}"))?;
    if !out.status.success() {
        return Err(format!("`xplain --help` exited {} for {target}", out.status));
    }
    log(&format!("  ok `xplain --help` exit 0 ({target})"));
    Ok(())
}

fn dist(args: &[String]) -> Res<()> {
    let mut version = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--version" => version = Some(it.next().ok_or("--version needs a value")?.clone()),
            other => {
                return Err(format!("unknown argument {other}. usage: cargo xtask dist [--version <v>]"));
            }
        }
    }
    let version = match version {
        Some(v) => v,
        None => app_version()?,
    };
    let t0 = Instant::now();
    log(&format!("dist xplain {version}"));

    log("step 1/6 rustup targets");
    ensure_targets()?;
    log("step 2/6 zig");
    let zig = zig::ensure(&root())?;
    log("step 3/6 macOS builds");
    cargo_build(MAC_ARM)?;
    cargo_build(MAC_X86)?;
    log("  lipo universal");
    lipo()?;
    log("step 4/6 linux builds (cargo-zigbuild library)");
    linux_build(LINUX_X86, &zig)?;
    linux_build(LINUX_ARM, &zig)?;

    log("step 5/6 smoke");
    for t in DIST_TARGETS {
        check_arch(t)?;
    }
    smoke_run()?;

    log("step 6/6 package");
    let dist_dir = root().join("dist");
    let _ = fs::remove_dir_all(&dist_dir);
    fs::create_dir_all(&dist_dir).map_err(|e| e.to_string())?;
    let mut sums = String::new();
    for t in DIST_TARGETS {
        let p = package(&dist_dir, &version, t)?;
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        sums.push_str(&format!("{}  {name}\n", zig::sha256_file(&p)?));
        log(&format!("  {name} ({} KiB)", size / 1024));
    }
    fs::write(dist_dir.join("SHA256SUMS"), sums).map_err(|e| e.to_string())?;
    log(&format!("done in {:.1}s -> {}", t0.elapsed().as_secs_f64(), dist_dir.display()));
    Ok(())
}
