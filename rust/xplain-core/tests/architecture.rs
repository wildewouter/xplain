//! Architecture guard: agent-specific names live only in xplain-integrations (behind `AgentIntegration`).
//! Scans every `.rs` / `.toml` file of xplain-core and xplain-app for agent names, case-insensitive.

use std::fs;
use std::path::{Path, PathBuf};

// Spelled in pieces so this file does not match itself.
const AGENTS: [[&str; 2]; 4] = [["clau", "de"], ["cod", "ex"], ["copi", "lot"], ["open", "code"]];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("entry in {}: {e}", dir.display()))?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out)?;
        } else if path.extension().is_some_and(|x| x == "rs" || x == "toml") {
            out.push(path);
        }
    }
    Ok(())
}

#[test]
fn core_and_app_never_name_an_agent() -> Result<(), String> {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let this = core.join("tests").join("architecture.rs");
    let mut files = Vec::new();
    for dir in [core.to_path_buf(), core.join("..").join("xplain-app")] {
        walk(&dir, &mut files)?;
    }
    let names: Vec<String> = AGENTS.iter().map(|p| p.concat()).collect();
    let mut hits = Vec::new();
    for f in files.iter().filter(|f| **f != this) {
        let text = fs::read_to_string(f).map_err(|e| format!("read {}: {e}", f.display()))?;
        for (i, line) in text.lines().enumerate() {
            let low = line.to_lowercase();
            if let Some(n) = names.iter().find(|n| low.contains(n.as_str())) {
                hits.push(format!("{}:{}: `{n}`: {}", f.display(), i + 1, line.trim()));
            }
        }
    }
    if files.len() < 10 {
        return Err(format!("scan found only {} files: wrong root?", files.len()));
    }
    if hits.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "agent names outside xplain-integrations (use generic wording, see xplain-integrations):\n{}",
            hits.join("\n")
        ))
    }
}
