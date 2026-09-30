//! Spec coverage gate: every in-scope feature ID in `spec/SPEC.md` ("## Coverage index",
//! Test column `yes`) needs a Rust test, or an entry in `app_pending.txt`.
//!
//! A test covers an ID when its fn is named `f_<group>_<nn>_...` (maps to `F-<GROUP>-<NN>`)
//! or when a `// covers: F-X-NN[, F-Y-NN]` comment marker is present.
//! `app_pending.txt`: one `F-X-NN reason` per line, `#` comments allowed. IDs not yet covered
//! by any Rust test. Entries that already have a test fail.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn in_scope_ids() -> BTreeSet<String> {
    let spec = fs::read_to_string(root().join("spec/SPEC.md")).expect("read spec/SPEC.md");
    let table = spec.split("## Coverage index").nth(1).expect("Coverage index section");
    let mut ids = BTreeSet::new();
    for line in table.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 4 || !cells[1].starts_with("F-") {
            continue;
        }
        if cells[3] == "yes" {
            ids.insert(cells[1].to_string());
        } else {
            assert!(cells[3].starts_with("no ("), "unknown Test value {:?} for {}", cells[3], cells[1]);
        }
    }
    assert!(!ids.is_empty(), "no in-scope IDs parsed");
    ids
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).expect("read_dir") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs")
            && p.file_name().is_some_and(|n| n != "spec_coverage.rs")
        {
            out.push(p);
        }
    }
}

/// ID -> one example location of a covering test.
fn tested_ids() -> BTreeMap<String, String> {
    let mut files = Vec::new();
    rust_files(&root(), &mut files);
    let mut found = BTreeMap::new();
    for f in files {
        let text = fs::read_to_string(&f).expect("read rs file");
        for (i, line) in text.lines().enumerate() {
            let t = line.trim();
            let loc = format!("{}:{}", f.display(), i + 1);
            if let Some(rest) = t.strip_prefix("fn f_").or_else(|| t.strip_prefix("pub fn f_")) {
                let mut parts = rest.split('_');
                let (group, num) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                if !group.is_empty()
                    && group.chars().all(|c| c.is_ascii_lowercase())
                    && num.len() >= 2
                    && num.chars().all(|c| c.is_ascii_digit())
                {
                    let id = format!("F-{}-{}", group.to_uppercase(), num);
                    found.entry(id).or_insert(loc.clone());
                }
            }
            if let Some(rest) = t.strip_prefix("// covers:") {
                for id in rest.split(|c: char| c == ',' || c.is_whitespace()) {
                    if id.starts_with("F-") {
                        found.entry(id.to_string()).or_insert(loc.clone());
                    }
                }
            }
        }
    }
    found
}

fn pending() -> BTreeMap<String, String> {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/app_pending.txt"))
        .expect("read app_pending.txt");
    let mut m = BTreeMap::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (id, reason) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
        assert!(!reason.trim().is_empty(), "{id}: allowlist entry needs a reason");
        assert!(m.insert(id.to_string(), reason.trim().to_string()).is_none(), "{id}: duplicate");
    }
    m
}

#[test]
fn every_in_scope_feature_has_a_test_or_pending_entry() {
    let scope = in_scope_ids();
    let tested = tested_ids();
    let pending = pending();

    let missing: Vec<&String> =
        scope.iter().filter(|id| !tested.contains_key(*id) && !pending.contains_key(*id)).collect();
    assert!(missing.is_empty(), "in-scope IDs with no test and not in app_pending.txt: {missing:?}");

    let stale: Vec<String> = pending
        .keys()
        .filter(|id| tested.contains_key(*id))
        .map(|id| format!("{id} (tested at {})", tested[id]))
        .collect();
    assert!(stale.is_empty(), "app_pending.txt entries that now have a test, remove them: {stale:?}");

    let unknown: Vec<&String> = pending.keys().filter(|id| !scope.contains(*id)).collect();
    assert!(unknown.is_empty(), "app_pending.txt entries not in scope: {unknown:?}");
}
