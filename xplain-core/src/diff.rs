//! Diff model and unified-diff parser.
//!
//! Spec: F-EDGE-01..06, F-MODE-01 (argv builder), F-MODE-02 (assembly of untracked entries),
//! F-SCOPE-01. Owner: core lead (diff component).
//! Must not: run git or read files. The runtime runs git (argv from [`DiffSpec::git_argv`]) and hands
//! back raw text in [`RawDiff`]; parsing and status derivation happen here.

use crate::options::DiffMode;

/// Everything the runtime needs to load a diff (payload of `Effect::LoadDiff`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSpec {
    /// `--cwd` verbatim; `None` = process cwd. Runtime checks it exists/is dir before spawning git (F-CLI-06).
    pub cwd: Option<String>,
    pub mode: DiffMode,
    /// Full-file scope adds `-U1000000`.
    pub full: bool,
    pub git_args: Vec<String>,
}

impl DiffSpec {
    /// Untracked files are listed and appended only in mode `all` with no extra args (F-MODE-02).
    pub fn wants_untracked(&self) -> bool {
        self.mode == DiffMode::All && self.git_args.is_empty()
    }

    /// Argv after `git`: `diff --no-color --no-ext-diff [-U1000000] <base> <extra>` (F-MODE-01).
    pub fn git_argv(&self) -> Vec<String> {
        let mut v = self.base_argv();
        match self.mode {
            DiffMode::Staged => v.push("--cached".into()),
            DiffMode::Unstaged => {}
            DiffMode::All => {
                if self.git_args.is_empty() {
                    v.push("HEAD".into());
                }
            }
        }
        v.extend(self.git_args.iter().cloned());
        v
    }

    fn base_argv(&self) -> Vec<String> {
        let mut v: Vec<String> = ["diff", "--no-color", "--no-ext-diff"].map(String::from).to_vec();
        if self.full {
            v.push("-U1000000".into());
        }
        v
    }

    /// Argv after `git` for one untracked file: `diff --no-index --no-color --no-ext-diff [-U1000000] -- /dev/null <name>`.
    pub fn untracked_argv(&self, name: &str) -> Vec<String> {
        let mut v: Vec<String> = vec!["diff".into(), "--no-index".into()];
        v.extend(self.base_argv().into_iter().skip(1));
        v.extend(["--".into(), "/dev/null".into(), name.to_string()]);
        v
    }
}

/// Raw git output as delivered by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDiff {
    /// stdout of the tracked `git diff` command.
    pub tracked: String,
    /// stdout of each `git diff --no-index` untracked-file command, in `ls-files` order. Runtime already
    /// skipped symlinks, dirs, unreadable and files > 1048576 bytes.
    pub untracked: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    /// Raw text without the leading mark; tabs kept (expanded at render time).
    pub text: String,
    /// True for the synthetic ` No newline at end of file` row (F-EDGE-06); not counted in adds/dels.
    pub no_newline_marker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// Full `@@ ... @@ ...` line.
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Why a file has no hunks (single dim note row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// `Renamed, no content changes`
    RenamedNoChanges,
    /// `Binary file`
    Binary,
    /// `No textual changes`
    NoTextualChanges,
}

/// Picker status letter (F-EDGE-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Added,
    Deleted,
    Modified,
    Renamed,
}

impl Status {
    pub fn letter(self) -> char {
        match self {
            Status::Added => 'A',
            Status::Deleted => 'D',
            Status::Modified => 'M',
            Status::Renamed => 'R',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Display path: new path (old path for deleted files), git prefix `a/`,`b/` stripped once (F-EDGE-01).
    pub path: String,
    /// Set for renames: `path` is the new path, this the old one. Display is `<old> -> <new>`.
    pub old_path: Option<String>,
    pub status: Status,
    pub adds: u32,
    pub dels: u32,
    pub hunks: Vec<Hunk>,
    /// `Some` when `hunks` is empty.
    pub note: Option<Note>,
}

impl FileDiff {
    /// `<old> -> <new>` for renames, else `path` (header and picker).
    pub fn display_path(&self) -> String {
        match &self.old_path {
            Some(old) => format!("{old} -> {}", self.path),
            None => self.path.clone(),
        }
    }
}

/// Parse tracked + untracked git output into files (tracked first, then untracked in given order).
pub fn parse_raw(raw: &RawDiff) -> Vec<FileDiff> {
    let mut files = parse_unified(&raw.tracked);
    for u in &raw.untracked {
        files.extend(parse_unified(u));
    }
    files
}

/// Parse one unified diff text possibly holding several file entries.
pub fn parse_unified(text: &str) -> Vec<FileDiff> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let starts: Vec<usize> = (0..lines.len()).filter(|&i| lines[i].starts_with("diff ")).collect();
    starts
        .iter()
        .enumerate()
        .map(|(k, &at)| {
            let end = starts.get(k + 1).copied().unwrap_or(lines.len());
            file_section(&lines[at..end])
        })
        .collect()
}

// ---------- one file section ----------

fn file_section(lines: &[&str]) -> FileDiff {
    let h = header(lines);
    let hunks = hunks(lines);
    let (mut adds, mut dels) = (0u32, 0u32);
    for l in hunks.iter().flat_map(|h| &h.lines) {
        if l.no_newline_marker {
            continue;
        }
        match l.kind {
            LineKind::Add => adds += 1,
            LineKind::Del => dels += 1,
            LineKind::Context => {}
        }
    }
    let path = h.to.clone().or_else(|| h.from.clone()).unwrap_or_else(|| "?".to_string());
    let old_path = if h.renamed { h.from.clone() } else { None };
    let note = if h.binary {
        Some(Note::Binary)
    } else if hunks.is_empty() {
        Some(if h.renamed { Note::RenamedNoChanges } else { Note::NoTextualChanges })
    } else {
        None
    };
    let status = if h.renamed {
        Status::Renamed
    } else if !hunks.is_empty() && hunks.iter().all(|x| x.header.starts_with("@@ -0,0 ")) {
        Status::Added
    } else if !hunks.is_empty() && hunks.iter().all(|x| x.header.contains(" +0,0 @@")) {
        Status::Deleted
    } else {
        Status::Modified
    };
    FileDiff { path, old_path, status, adds, dels, hunks, note }
}

struct Paths {
    from: Option<String>,
    to: Option<String>,
    binary: bool,
    renamed: bool,
}

const C_ESC: &[(u8, u8)] = &[
    (b'a', 7),
    (b'b', 8),
    (b't', 9),
    (b'n', 10),
    (b'v', 11),
    (b'f', 12),
    (b'r', 13),
    (b'"', 34),
    (b'\\', 92),
];

/// Decode a git C-quoted string starting at `s[i] == '"'`. Returns text and index after the closing quote.
fn read_quoted(s: &str, i: usize) -> (String, usize) {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut j = i + 1;
    while j < b.len() && b[j] != b'"' {
        if b[j] == b'\\' && j + 1 < b.len() {
            let oct = b.get(j + 1..j + 4).filter(|d| d.iter().all(|c| (b'0'..=b'7').contains(c)));
            if let Some(d) = oct {
                let v = d.iter().fold(0u32, |acc, c| acc * 8 + u32::from(c - b'0'));
                out.push((v & 0xff) as u8);
                j += 4;
            } else if let Some(&(_, v)) = C_ESC.iter().find(|(k, _)| *k == b[j + 1]) {
                out.push(v);
                j += 2;
            } else {
                let cl = s[j + 1..].chars().next().map_or(1, char::len_utf8);
                out.extend_from_slice(&b[j + 1..j + 1 + cl]);
                j += 1 + cl;
            }
            continue;
        }
        let cl = s[j..].chars().next().map_or(1, char::len_utf8);
        out.extend_from_slice(&b[j..j + cl]);
        j += cl;
    }
    (String::from_utf8_lossy(&out).into_owned(), j + 1)
}

/// One path argument of a header line: C-quoted or raw (git appends a TAB to names with spaces).
fn path_arg(s: &str) -> String {
    if s.starts_with('"') { read_quoted(s, 0).0 } else { s.strip_suffix('\t').unwrap_or(s).to_string() }
}

/// Git's own prefix, stripped once. `/dev/null` = no file on that side (`Some(None)`).
fn strip(p: &str, prefix: &str) -> Option<String> {
    if p == "/dev/null" { None } else { Some(p.strip_prefix(prefix).unwrap_or(p).to_string()) }
}

fn strip_opt(p: &Option<String>, prefix: &str) -> Option<String> {
    p.as_deref().and_then(|p| strip(p, prefix))
}

/// `diff --git <a/old> <b/new>`, both sides possibly C-quoted; unquoted ambiguous names split in the middle
/// when both sides are the same path.
fn git_header(rest: &str) -> (Option<String>, Option<String>) {
    if rest.starts_with('"') {
        let (a, k) = read_quoted(rest, 0);
        let tail = rest.get(k + 1..).unwrap_or("");
        return (Some(a), Some(path_arg(tail)));
    }
    if let Some(q) = rest.rfind(" \"")
        && q > 0
        && rest.ends_with('"')
    {
        return (Some(rest[..q].to_string()), Some(read_quoted(rest, q + 1).0));
    }
    let len = rest.len();
    if len >= 7 && (len - 5) & 1 == 0 {
        let n = (len - 5) / 2;
        if let (Some(x), Some(y)) = (rest.get(2..2 + n), rest.get(5 + n..))
            && x == y
            && rest.as_bytes()[2 + n] == b' '
            && let (Some(a), Some(b)) = (rest.get(..2 + n), rest.get(3 + n..))
        {
            return (Some(a.to_string()), Some(b.to_string()));
        }
    }
    match rest.find(" b/") {
        Some(m) if m > 0 => (Some(rest[..m].to_string()), Some(rest[m + 1..].to_string())),
        _ => (Some(rest.to_string()), Some(rest.to_string())),
    }
}

fn header(section: &[&str]) -> Paths {
    // `--- x`: None = not seen, Some(None) = /dev/null, Some(Some(p)) = path.
    let mut from: Option<Option<String>> = None;
    let mut to: Option<Option<String>> = None;
    let (mut rn_from, mut rn_to): (Option<String>, Option<String>) = (None, None);
    let mut hdr: (Option<String>, Option<String>) = (None, None);
    let (mut added, mut deleted, mut binary) = (false, false, false);
    for line in section {
        if line.starts_with("@@") {
            break;
        }
        if let Some(r) = line.strip_prefix("diff --git ") {
            hdr = git_header(r);
        } else if line.starts_with("diff --cc ") || line.starts_with("diff --combined ") {
            let at = line[5..].find(' ').map_or(0, |p| p + 5 + 1);
            hdr = (None, Some(path_arg(&line[at..])));
        } else if let Some(r) = line.strip_prefix("--- ") {
            from = Some(strip(&path_arg(r), "a/"));
        } else if let Some(r) = line.strip_prefix("+++ ") {
            to = Some(strip(&path_arg(r), "b/"));
        } else if let Some(r) = line.strip_prefix("rename from ").or_else(|| line.strip_prefix("copy from "))
        {
            rn_from = Some(path_arg(r));
        } else if let Some(r) = line.strip_prefix("rename to ").or_else(|| line.strip_prefix("copy to ")) {
            rn_to = Some(path_arg(r));
        } else if line.starts_with("new file mode ") {
            added = true;
        } else if line.starts_with("deleted file mode ") {
            deleted = true;
        } else if *line == "GIT binary patch"
            || line.strip_prefix("Binary files ").is_some_and(|r| r.ends_with(" differ"))
        {
            binary = true;
        }
    }
    let h_from = strip_opt(&hdr.0, "a/");
    let h_to = strip_opt(&hdr.1, "b/");
    let old_p = rn_from.or(match from {
        Some(None) => None,
        Some(Some(p)) => Some(p),
        None if added => None,
        None => h_from,
    });
    let new_p = rn_to.or(match to {
        Some(None) => None,
        Some(Some(p)) => Some(p),
        None if deleted => None,
        None => h_to,
    });
    let renamed = matches!((&old_p, &new_p), (Some(a), Some(b)) if a != b);
    Paths { from: old_p, to: new_p, binary, renamed }
}

/// `@@ -a[,b] +c[,d] @@` -> (a, b, c, d).
fn hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let range = |s: &str| -> Option<(u32, u32)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (a, b) = range(old)?;
    let (c, d) = range(new)?;
    Some((a, b, c, d))
}

fn hunks(section: &[&str]) -> Vec<Hunk> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < section.len() {
        let Some((mut o, mut old_rem, mut n, mut new_rem)) = hunk_header(section[i]) else {
            i += 1;
            continue;
        };
        let mut h = Hunk { header: section[i].to_string(), lines: Vec::new() };
        i += 1;
        while i < section.len() {
            let l = section[i];
            let marker = l.starts_with('\\');
            if !marker && old_rem == 0 && new_rem == 0 {
                break;
            }
            let (kind, text) = match l.chars().next() {
                Some(' ') if old_rem > 0 && new_rem > 0 => (LineKind::Context, &l[1..]),
                None if old_rem > 0 && new_rem > 0 => (LineKind::Context, ""),
                Some('-') if old_rem > 0 => (LineKind::Del, &l[1..]),
                Some('+') if new_rem > 0 => (LineKind::Add, &l[1..]),
                Some('\\') => {
                    if let Some(prev) = h.lines.last() {
                        let m = DiffLine {
                            kind: prev.kind,
                            old_no: prev.old_no,
                            new_no: prev.new_no,
                            text: l[1..].to_string(),
                            no_newline_marker: true,
                        };
                        h.lines.push(m);
                    }
                    i += 1;
                    continue;
                }
                _ => break,
            };
            let line = match kind {
                LineKind::Context => {
                    let d = DiffLine {
                        kind,
                        old_no: Some(o),
                        new_no: Some(n),
                        text: text.to_string(),
                        no_newline_marker: false,
                    };
                    o += 1;
                    n += 1;
                    old_rem -= 1;
                    new_rem -= 1;
                    d
                }
                LineKind::Del => {
                    let d = DiffLine {
                        kind,
                        old_no: Some(o),
                        new_no: None,
                        text: text.to_string(),
                        no_newline_marker: false,
                    };
                    o += 1;
                    old_rem -= 1;
                    d
                }
                LineKind::Add => {
                    let d = DiffLine {
                        kind,
                        old_no: None,
                        new_no: Some(n),
                        text: text.to_string(),
                        no_newline_marker: false,
                    };
                    n += 1;
                    new_rem -= 1;
                    d
                }
            };
            h.lines.push(line);
            i += 1;
        }
        out.push(h);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(mode: DiffMode, full: bool, args: &[&str]) -> DiffSpec {
        DiffSpec { cwd: None, mode, full, git_args: args.iter().map(|s| s.to_string()).collect() }
    }
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    fn one(text: &str) -> FileDiff {
        let mut f = parse_unified(text);
        assert_eq!(f.len(), 1, "{f:?}");
        f.remove(0)
    }

    #[test]
    fn f_mode_01_argv() {
        let base = ["diff", "--no-color", "--no-ext-diff"];
        let mut e = base.to_vec();
        e.extend(["-U1000000", "HEAD"]);
        assert_eq!(spec(DiffMode::All, true, &[]).git_argv(), v(&e));
        assert_eq!(spec(DiffMode::All, false, &[]).git_argv(), v(&[base.as_slice(), &["HEAD"]].concat()));
        assert_eq!(
            spec(DiffMode::All, false, &["main", "--stat"]).git_argv(),
            v(&[base.as_slice(), &["main", "--stat"]].concat())
        );
        assert_eq!(
            spec(DiffMode::Staged, false, &[]).git_argv(),
            v(&[base.as_slice(), &["--cached"]].concat())
        );
        assert_eq!(
            spec(DiffMode::Staged, true, &["x"]).git_argv(),
            v(&[base.as_slice(), &["-U1000000", "--cached", "x"]].concat())
        );
        assert_eq!(spec(DiffMode::Unstaged, false, &[]).git_argv(), v(&base));
        assert_eq!(
            spec(DiffMode::Unstaged, true, &["a"]).git_argv(),
            v(&[base.as_slice(), &["-U1000000", "a"]].concat())
        );
    }

    #[test]
    fn f_mode_02_untracked_argv_and_gate() {
        assert_eq!(
            spec(DiffMode::All, true, &[]).untracked_argv("n f"),
            v(&["diff", "--no-index", "--no-color", "--no-ext-diff", "-U1000000", "--", "/dev/null", "n f"])
        );
        assert_eq!(
            spec(DiffMode::All, false, &[]).untracked_argv("n"),
            v(&["diff", "--no-index", "--no-color", "--no-ext-diff", "--", "/dev/null", "n"])
        );
        assert!(spec(DiffMode::All, true, &[]).wants_untracked());
        assert!(!spec(DiffMode::All, true, &["x"]).wants_untracked());
        assert!(!spec(DiffMode::Staged, true, &[]).wants_untracked());
    }

    #[test]
    fn f_scope_01_full_flag() {
        assert!(spec(DiffMode::All, true, &[]).git_argv().contains(&"-U1000000".to_string()));
        assert!(!spec(DiffMode::All, false, &[]).git_argv().contains(&"-U1000000".to_string()));
    }

    #[test]
    fn f_edge_01_prefix_strip_once() {
        let f = one(
            "diff --git a/a/x.txt b/a/x.txt\nindex 1..2 100644\n--- a/a/x.txt\n+++ b/a/x.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n",
        );
        assert_eq!(f.path, "a/x.txt");
        assert_eq!((f.adds, f.dels, f.status), (1, 1, Status::Modified));
        let f = one("diff --git a/b/a/y b/b/a/y\n--- a/b/a/y\n+++ b/b/a/y\n@@ -1 +1 @@\n-x\n+y\n");
        assert_eq!(f.path, "b/a/y");
    }

    #[test]
    fn f_edge_01_deleted_uses_old_path() {
        let f = one(
            "diff --git a/del.txt b/del.txt\ndeleted file mode 100644\nindex 286c5f5..0000000\n--- a/del.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n",
        );
        assert_eq!(f.path, "del.txt");
        assert_eq!(f.status, Status::Deleted);
        assert_eq!((f.adds, f.dels), (0, 1));
        assert!(f.note.is_none());
    }

    #[test]
    fn f_edge_01_tab_after_space_names() {
        let f = one(
            "diff --git a/my file.txt b/my file.txt\nindex 1..2 100644\n--- a/my file.txt\t\n+++ b/my file.txt\t\n@@ -1 +1 @@\n-x\n+y\n",
        );
        assert_eq!(f.path, "my file.txt");
        // header-only (no ---/+++): split in the middle
        let f = one("diff --git a/my file.txt b/my file.txt\nold mode 100644\nnew mode 100755\n");
        assert_eq!(f.path, "my file.txt");
    }

    #[test]
    fn f_edge_02_rename() {
        let f = one(
            "diff --git a/old.txt b/new.txt\nsimilarity index 100%\nrename from old.txt\nrename to new.txt\n",
        );
        assert_eq!(f.status, Status::Renamed);
        assert_eq!(f.path, "new.txt");
        assert_eq!(f.old_path.as_deref(), Some("old.txt"));
        assert_eq!(f.display_path(), "old.txt -> new.txt");
        assert_eq!(f.note, Some(Note::RenamedNoChanges));
        assert_eq!((f.adds, f.dels), (0, 0));
        assert!(f.hunks.is_empty());
    }

    #[test]
    fn f_edge_02_rename_with_changes() {
        let f = one(
            "diff --git a/o b/n\nsimilarity index 50%\nrename from o\nrename to n\n--- a/o\n+++ b/n\n@@ -1,2 +1,2 @@\n k\n-a\n+b\n",
        );
        assert_eq!(f.status, Status::Renamed);
        assert_eq!(f.display_path(), "o -> n");
        assert!(f.note.is_none());
        assert_eq!((f.adds, f.dels), (1, 1));
    }

    #[test]
    fn f_edge_03_binary() {
        let f = one(
            "diff --git a/bin.dat b/bin.dat\nindex 8352675..1592e5c 100644\nBinary files a/bin.dat and b/bin.dat differ\n",
        );
        assert_eq!(f.note, Some(Note::Binary));
        assert_eq!((f.adds, f.dels, f.status), (0, 0, Status::Modified));
        let f = one(
            "diff --git a/b b/b\nnew file mode 100644\nindex 0..1\nGIT binary patch\nliteral 3\nIcmZQzU|;|M0\n",
        );
        assert_eq!((f.note, f.status), (Some(Note::Binary), Status::Modified));
        let f = one("diff --git a/b b/b\ndeleted file mode 100644\nBinary files a/b and /dev/null differ\n");
        assert_eq!((f.note, f.status, f.path.as_str()), (Some(Note::Binary), Status::Modified, "b"));
    }

    #[test]
    fn f_edge_03_binary_detection_per_entry() {
        let fs = parse_unified(
            "diff --git a/b b/b\nBinary files a/b and b/b differ\ndiff --git a/t b/t\n--- a/t\n+++ b/t\n@@ -1 +1 @@\n-x\n+y\n",
        );
        assert_eq!(fs.len(), 2);
        assert_eq!(fs[0].note, Some(Note::Binary));
        assert_eq!(fs[1].note, None);
    }

    #[test]
    fn f_edge_04_no_textual_changes() {
        let f = one("diff --git a/e.txt b/e.txt\nold mode 100644\nnew mode 100755\n");
        assert_eq!(f.note, Some(Note::NoTextualChanges));
        assert_eq!(f.status, Status::Modified);
        let f = one("diff --git a/e b/e\nnew file mode 100644\nindex 0000000..e69de29\n");
        assert_eq!((f.note, f.path.as_str()), (Some(Note::NoTextualChanges), "e"));
        let f = one("diff --git a/e b/e\ndeleted file mode 100644\nindex e69de29..0000000\n");
        assert_eq!((f.note, f.path.as_str()), (Some(Note::NoTextualChanges), "e"));
    }

    #[test]
    fn f_edge_04_mode_change_with_content_has_hunks() {
        let f = one(
            "diff --git a/s b/s\nold mode 100644\nnew mode 100755\nindex 1..2\n--- a/s\n+++ b/s\n@@ -1 +1 @@\n-a\n+b\n",
        );
        assert!(f.note.is_none());
        assert_eq!(f.hunks.len(), 1);
    }

    #[test]
    fn f_edge_05_status_letters() {
        let a = one("diff --git a/n b/n\nnew file mode 100644\n--- /dev/null\n+++ b/n\n@@ -0,0 +1 @@\n+z\n");
        assert_eq!(a.status.letter(), 'A');
        assert_eq!(a.path, "n");
        let d =
            one("diff --git a/n b/n\ndeleted file mode 100644\n--- a/n\n+++ /dev/null\n@@ -1 +0,0 @@\n-z\n");
        assert_eq!(d.status.letter(), 'D');
        let m = one("diff --git a/n b/n\n--- a/n\n+++ b/n\n@@ -1 +1 @@\n-z\n+y\n");
        assert_eq!(m.status.letter(), 'M');
        let r = one("diff --git a/o b/n\nrename from o\nrename to n\n");
        assert_eq!(r.status.letter(), 'R');
    }

    #[test]
    fn f_edge_05_added_needs_all_hunks_from_zero() {
        let f = one("diff --git a/n b/n\n--- a/n\n+++ b/n\n@@ -0,0 +1 @@\n+a\n@@ -5 +6,2 @@\n x\n+y\n");
        // second hunk has one old line and two new lines: -5 +6,2 => ctx + add
        assert_eq!(f.status, Status::Modified);
    }

    #[test]
    fn f_edge_06_no_newline_marker() {
        let f = one(
            "diff --git a/nn.txt b/nn.txt\n--- a/nn.txt\n+++ b/nn.txt\n@@ -1,2 +1,2 @@\n a\n-b\n\\ No newline at end of file\n+c\n\\ No newline at end of file\n",
        );
        assert_eq!((f.adds, f.dels), (1, 1));
        let l = &f.hunks[0].lines;
        assert_eq!(l.len(), 5);
        assert_eq!(
            (l[2].kind, l[2].old_no, l[2].new_no, l[2].no_newline_marker),
            (LineKind::Del, Some(2), None, true)
        );
        assert_eq!(l[2].text, " No newline at end of file");
        assert_eq!((l[4].kind, l[4].new_no, l[4].no_newline_marker), (LineKind::Add, Some(2), true));
        assert_eq!(l[4].text, " No newline at end of file");
        assert!(!l[1].no_newline_marker);
    }

    #[test]
    fn f_edge_06_marker_after_context() {
        let f = one(
            "diff --git a/n b/n\n--- a/n\n+++ b/n\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n\\ No newline at end of file\n",
        );
        let l = &f.hunks[0].lines;
        assert_eq!(l[3].kind, LineKind::Context);
        assert_eq!((l[3].old_no, l[3].new_no), (Some(2), Some(2)));
        assert!(l[3].no_newline_marker);
    }

    #[test]
    fn f_edge_07_quoted_paths() {
        let f = one(
            "diff --git \"a/\\303\\251.txt\" \"b/\\303\\251.txt\"\nindex 1..2 100644\n--- \"a/\\303\\251.txt\"\n+++ \"b/\\303\\251.txt\"\n@@ -1 +1 @@\n-a\n+b\n",
        );
        assert_eq!(f.path, "é.txt");
        let f =
            one("diff --git \"a/\\303\\251.txt\" \"b/\\303\\251.txt\"\nold mode 100644\nnew mode 100755\n");
        assert_eq!(f.path, "é.txt");
        let f = one("diff --git \"a/t\\tq\\\"x\" \"b/t\\tq\\\"x\"\nnew mode 1\n");
        assert_eq!(f.path, "t\tq\"x");
        let f = one("diff --git a/é.txt b/é.txt\nold mode 1\nnew mode 2\n");
        assert_eq!(f.path, "é.txt");
    }

    #[test]
    fn f_edge_08_content_kept_raw() {
        let f = one("diff --git a/t b/t\n--- a/t\n+++ b/t\n@@ -1 +1 @@\n-\ta\u{4e2d}\n+--- x\n");
        let l = &f.hunks[0].lines;
        assert_eq!(l[0].text, "\ta\u{4e2d}");
        assert_eq!(l[1].text, "--- x");
        assert_eq!((f.adds, f.dels), (1, 1));
    }

    #[test]
    fn f_edge_line_numbers_and_headers() {
        let f = one(
            "diff --git a/t b/t\n--- a/t\n+++ b/t\n@@ -3,3 +3,4 @@ fn x()\n a\n-b\n+B\n+C\n c\n@@ -20 +21 @@\n-z\n+y\n",
        );
        assert_eq!(f.hunks[0].header, "@@ -3,3 +3,4 @@ fn x()");
        let l = &f.hunks[0].lines;
        assert_eq!((l[0].old_no, l[0].new_no), (Some(3), Some(3)));
        assert_eq!((l[1].old_no, l[1].new_no), (Some(4), None));
        assert_eq!((l[2].old_no, l[2].new_no), (None, Some(4)));
        assert_eq!((l[3].old_no, l[3].new_no), (None, Some(5)));
        assert_eq!((l[4].old_no, l[4].new_no), (Some(5), Some(6)));
        assert_eq!(f.hunks[1].lines[0].old_no, Some(20));
        assert_eq!(f.hunks[1].lines[1].new_no, Some(21));
        assert_eq!((f.adds, f.dels), (3, 2));
    }

    #[test]
    fn f_mode_02_untracked_after_tracked() {
        let raw = RawDiff {
            tracked: "diff --git a/t b/t\n--- a/t\n+++ b/t\n@@ -1 +1 @@\n-a\n+b\n".into(),
            untracked: vec![
                "diff --git a/u1 b/u1\nnew file mode 100644\nindex 0..1\n--- /dev/null\n+++ b/u1\n@@ -0,0 +1 @@\n+q\n".into(),
                "diff --git a/u2 b/u2\nnew file mode 100644\nindex 0..e69de29\n".into(),
            ],
        };
        let f = parse_raw(&raw);
        let names: Vec<_> = f.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(names, ["t", "u1", "u2"]);
        assert_eq!(f[1].status, Status::Added);
        assert_eq!(f[2].status, Status::Modified);
        assert_eq!(f[2].note, Some(Note::NoTextualChanges));
    }

    #[test]
    fn f_edge_empty_and_crlf() {
        assert!(parse_unified("").is_empty());
        assert!(parse_raw(&RawDiff::default()).is_empty());
        let f = one("diff --git a/t b/t\r\n--- a/t\r\n+++ b/t\r\n@@ -1 +1 @@\r\n-a\r\n+b\r\n");
        assert_eq!(f.hunks[0].lines[0].text, "a");
    }
}
