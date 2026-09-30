//! Scenario tests (F-MODE, F-RELOAD, F-SCOPE).
//!
//! The black box suite scripted a fake `git`; here git is real, so canned outputs are replaced by real repos and
//! the git argv is asserted on the `LoadDiff` effects core emits (`DiffSpec::git_argv` / `untracked_argv`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use xplain_core::effect::Effect;
use xplain_sim::{CellExpect as C, Fixture, Sim};

const HELP_FOOT: &str = "hjkl move  enter ask  J/K comments  ? help";

/// `const vN = 1;` lines from..=to, with `head` lines first and `over` = (n, value) replacing the `1`.
fn big(head: &[&str], from: usize, to: usize, over: &[(usize, u32)]) -> String {
    let mut out = String::new();
    for h in head {
        out.push_str(h);
        out.push('\n');
    }
    for n in from..=to {
        let v = over.iter().find(|(k, _)| *k == n).map_or(1, |(_, v)| *v);
        out.push_str(&format!("const v{n} = {v};\n"));
    }
    out
}

/// README with `# Title`, `hello`, `l3`..`l<last>`.
fn readme(last: usize) -> String {
    let mut out = String::from("# Title\nhello\n");
    for n in 3..=last {
        out.push_str(&format!("l{n}\n"));
    }
    out
}

/// argv (after `git`) of every diff load core asked for so far.
fn loads(s: &Sim) -> Vec<Vec<String>> {
    s.effects()
        .iter()
        .filter_map(|e| match e {
            Effect::LoadDiff { spec, .. } => Some(spec.git_argv()),
            _ => None,
        })
        .collect()
}

fn argv(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

#[track_caller]
fn assert_loads(s: &Sim, want: &[&[&str]]) {
    let want: Vec<Vec<String>> = want.iter().map(|w| argv(w)).collect();
    assert_eq!(loads(s), want, "LoadDiff argv sequence");
}

fn last_spec(s: &Sim) -> xplain_core::diff::DiffSpec {
    s.effects()
        .iter()
        .rev()
        .find_map(|e| match e {
            Effect::LoadDiff { spec, .. } => Some(spec.clone()),
            _ => None,
        })
        .expect("no LoadDiff effect")
}

/// Empty repo with a committed `t.txt` (old) whose worktree copy is changed (new); built while the first load is
/// held so the app sees the final state. Returns the running sim.
fn t_repo(args: &[&str], extra: impl FnOnce(&Sim)) -> Sim {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(args.iter().copied()).hold_io().build();
    s.write_file("t.txt", "old\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "t"]);
    s.write_file("t.txt", "new\n");
    extra(&s);
    s.release_io();
    s
}

const NC: &str = "[all] [mcp: off] No changes (m cycles mode, F search, q quits)";

// ---------------------------------------------------------------- F-MODE-01

/// F-MODE-01: all mode, full scope, no extra args runs `git diff --no-color --no-ext-diff -U1000000 HEAD` in cwd
/// and shows its output.
#[test]
fn f_mode_01_argv_all() {
    let mut s = Sim::builder().fixture(Fixture::Empty).hold_io().build();
    s.write_file("shim.txt", "old line\n");
    s.git(&["add", "-A"]);
    s.git(&["commit", "-qm", "shim"]);
    s.write_file("shim.txt", "new line\n");
    s.release_io();
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] \[cursor L1:C1\] shim\.txt \+1 -1$",
    );
    s.assert_row(2, "@@ -1 +1 @@");
    s.assert_row_matches(3, r"^   1      -▶old line");
    s.assert_row_matches(4, r"^        1 \+ new line");
    assert_loads(&s, &[&["diff", "--no-color", "--no-ext-diff", "-U1000000", "HEAD"]]);
}

/// F-MODE-01: `--changes-only` with `--staged` and extra args gives `git diff --no-color --no-ext-diff --cached <args>`.
#[test]
fn f_mode_01_argv_changes_flags() {
    let s = Sim::builder().fixture(Fixture::Empty).args(["--changes-only", "--staged", "main"]).build();
    s.assert_row_matches(0, r"^\[staged\] .*No changes");
    assert_loads(&s, &[&["diff", "--no-color", "--no-ext-diff", "--cached", "main"]]);
}

/// F-MODE-01: extra git args replace HEAD in all mode and are appended after the base in staged/unstaged.
#[test]
fn f_mode_01_argv_extra_args() {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(["v1", "--", "src"]).hold_io().build();
    s.git(&["tag", "v1"]);
    s.release_io();
    s.assert_row_matches(0, r"^\[all\] .*No changes");
    let all = ["diff", "--no-color", "--no-ext-diff", "-U1000000", "v1", "--", "src"];
    assert_loads(&s, &[&all]);
    assert!(!loads(&s).iter().any(|a| a.iter().any(|x| x == "HEAD")), "no HEAD with extra args");
    s.keys("m");
    s.assert_row_matches(0, r"^\[staged\] ");
    let staged = ["diff", "--no-color", "--no-ext-diff", "-U1000000", "--cached", "v1", "--", "src"];
    assert_loads(&s, &[&all, &staged]);
    s.keys("m");
    s.assert_row_matches(0, r"^\[unstaged\] ");
    // unstaged argv equals the all-mode argv with extra args, so now 2 calls
    assert_loads(&s, &[&all, &staged, &all]);
}

/// F-MODE-01: git diff argv per mode and scope: staged adds `--cached`, unstaged has no base, changes scope drops
/// `-U1000000`.
#[test]
fn f_mode_01_argv_modes() {
    let mut s = Sim::builder().fixture(Fixture::Empty).build();
    // nothing changed, so every mode shows the no-changes screen
    s.assert_row_matches(0, r"^\[all\] .*No changes");
    let all = ["diff", "--no-color", "--no-ext-diff", "-U1000000", "HEAD"];
    assert_loads(&s, &[&all]);
    s.keys("m");
    s.assert_row_matches(0, r"^\[staged\] .*No changes");
    let staged = ["diff", "--no-color", "--no-ext-diff", "-U1000000", "--cached"];
    assert_loads(&s, &[&all, &staged]);
    s.keys("m");
    s.assert_row_matches(0, r"^\[unstaged\] .*No changes");
    let unstaged = ["diff", "--no-color", "--no-ext-diff", "-U1000000"];
    assert_loads(&s, &[&all, &staged, &unstaged]);
    // c works on the no-changes screen (F-MODE-05); scope changes the argv
    s.keys("c");
    let unstaged_c = ["diff", "--no-color", "--no-ext-diff"];
    assert_loads(&s, &[&all, &staged, &unstaged, &unstaged_c]);
    s.keys("m");
    s.assert_row_matches(0, r"^\[all\] .*No changes");
    let all_c = ["diff", "--no-color", "--no-ext-diff", "HEAD"];
    assert_loads(&s, &[&all, &staged, &unstaged, &unstaged_c, &all_c]);
    s.keys("m");
    let staged_c = ["diff", "--no-color", "--no-ext-diff", "--cached"];
    assert_loads(&s, &[&all, &staged, &unstaged, &unstaged_c, &all_c, &staged_c]);
}

// ---------------------------------------------------------------- F-MODE-02

/// F-MODE-02: untracked files only in all mode without extra args: not in staged/unstaged, not with extra args.
#[test]
fn f_mode_02_not_other_modes() {
    let mut s = Sim::builder().args(["HEAD"]).build();
    // all mode with extra args: x.bin (untracked) not listed
    s.assert_row_matches(0, r"^\[all\] .*\[1/3\] .*README\.md ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[3/3\] .*src/c\.tsx ");
    s.keys("mm");
    s.assert_row_matches(0, r"^\[unstaged\] .*\[1/3\] .*README\.md ");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[3/3\] .*src/c\.tsx ");
}

/// F-MODE-02: untracked file shown in default all mode but not with `--unstaged`, and no untracked listing then.
#[test]
fn f_mode_02_unstaged_flag() {
    let mut s = t_repo(&["--unstaged"], |s| s.write_file("u.txt", "you\n"));
    s.assert_row_matches(0, r"^\[unstaged\] .*\[1/1\] .*t\.txt \+1 -1$");
    let spec = last_spec(&s);
    assert!(!spec.wants_untracked(), "no ls-files / --no-index in unstaged mode");
    // cycle unstaged -> all; now untracked are listed
    s.keys("m");
    s.assert_row_matches(0, r"^\[all\] ");
    let spec = last_spec(&s);
    assert!(spec.wants_untracked());
    assert_eq!(
        spec.untracked_argv("u.txt"),
        argv(&["diff", "--no-index", "--no-color", "--no-ext-diff", "-U1000000", "--", "/dev/null", "u.txt"])
    );
    s.assert_row_matches(0, r"\[1/2\] ");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/2\] \[cursor L1:C1\] u\.txt \+1 -0$");
}

/// F-MODE-02: untracked listed via `ls-files --others --exclude-standard -z`, each diffed via `--no-index` against
/// /dev/null.
#[test]
fn f_mode_02_untracked_argv() {
    let mut s = t_repo(&[], |s| {
        s.write_file("b.txt", "bee\n");
        s.write_file("a.txt", "ay\n");
    });
    s.assert_row_matches(0, r"\[1/3\] \[cursor L1:C1\] t\.txt \+1 -1$");
    let spec = last_spec(&s);
    assert!(spec.wants_untracked());
    assert_eq!(
        spec.untracked_argv("a.txt"),
        argv(&["diff", "--no-index", "--no-color", "--no-ext-diff", "-U1000000", "--", "/dev/null", "a.txt"])
    );
    s.keys("<Tab>");
    // real ls-files order is sorted: a before b, after the tracked file
    s.assert_row_matches(0, r"\[2/3\] \[cursor L1:C1\] a\.txt \+1 -0$");
    s.assert_row_matches(3, r"^        1 \+▶ay");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[3/3\] \[cursor L1:C1\] b\.txt \+1 -0$");
    s.assert_row_matches(3, r"^        1 \+▶bee");
    // changes scope drops -U1000000 from the untracked diff too
    s.keys("c");
    s.assert_row_matches(0, r"^\[all\] \[changes\] .*\[3/3\] .*b\.txt \+1 -0$");
    assert_eq!(
        last_spec(&s).untracked_argv("a.txt"),
        argv(&["diff", "--no-index", "--no-color", "--no-ext-diff", "--", "/dev/null", "a.txt"])
    );
}

/// F-MODE-02: all mode lists untracked regular files <= 1 MiB after tracked files; big files, symlinks, dirs,
/// unreadable skipped; index untouched.
#[test]
fn f_mode_02_untracked_real() {
    let mut s = Sim::builder()
        .size(120, 30)
        .file("0first.txt", "zero\n")
        .file("sub/n.txt", "n1\nn2\n")
        .file("secret.txt", "s\n")
        .file("nested/inner.txt", "i\n")
        .build();
    // at start secret.txt is readable and nested/ is a plain dir, so nested/inner.txt is listed
    s.assert_row_matches(0, r"\[1/8\] \[cursor L2:C1\] README\.md \+2 -1$");
    // 1024 lines of 1023 a + newline = 1048576 bytes
    let line = format!("{}\n", "a".repeat(1023));
    s.write_file("fit.txt", &line.repeat(1024));
    s.write_file("huge.txt", &line.repeat(1024));
    s.append_file("huge.txt", "b");
    std::os::unix::fs::symlink("README.md", s.repo().join("link.md")).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(s.repo().join("secret.txt"), std::fs::Permissions::from_mode(0o0)).unwrap();
    }
    s.git(&["init", "-q", "nested"]);
    // fit.txt is exactly 1048576 bytes (shown), huge.txt 1048577 (skipped)
    s.keys("r");
    s.keys("f");
    // tracked first in git order, then untracked in ls-files order; skipped files absent
    s.assert_contains(" Files (1/7)");
    s.assert_matches(
        r"M README\.md \+2 -1 \*.*\n.*M src/big\.ts \+1 -1.*\n.*M src/c\.tsx \+1 -1.*\n.*A 0first\.txt \+1 -0.*\n.*A fit\.txt \+1024 -0.*\n.*A sub/n\.txt \+2 -0.*\n.*A x\.bin \+1 -0",
    );
    for t in ["huge.txt", "link.md", "secret.txt", "nested"] {
        s.assert_not_contains(t);
    }
    s.keys("<Esc><Tab><Tab><Tab>");
    // untracked file shown as a fully added file
    s.assert_row_matches(0, r"\[4/7\] \[cursor L1:C1\] 0first\.txt \+1 -0$");
    s.assert_row_matches(2, r"^@@ -0,0 \+1 @@");
    s.assert_row_matches(3, r"^        1 \+▶zero");
    s.keys("<Tab><Tab>");
    s.assert_row_matches(0, r"\[6/7\] \[cursor L1:C1\] sub/n\.txt \+2 -0$");
    s.assert_row_matches(2, r"^@@ -0,0 \+1,2 @@");
    s.assert_row_matches(3, r"^        1 \+▶n1");
    s.assert_row_matches(4, r"^        2 \+ n2");
    // untracked files were not added to the index
    assert_eq!(s.git_code(&["ls-files", "--error-unmatch", "0first.txt"]), 1);
    assert_eq!(s.git_code(&["diff", "--cached", "--quiet"]), 0);
}

// ---------------------------------------------------------------- F-MODE-03

/// F-MODE-03: `m` is ignored in browse; mode unchanged after going back to the diff.
#[test]
fn f_mode_03_browse_ignored() {
    let mut s = Sim::builder().build();
    s.keys("F");
    s.keys("src/a.ts<Enter>");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] src/a\.ts");
    s.keys("m");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] src/a\.ts");
    s.assert_row_matches(2, r"^   1  ▶export const a = 2;");
    s.keys("<Esc>");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md +2 -1");
    // m still works in the diff view afterwards
    s.keys("m");
    s.assert_row_matches(0, r"^\[staged\] ");
}

/// F-MODE-03: `m` cycles all -> staged -> unstaged -> all; chip updates, file index back to 1, diff reloaded from
/// the new source.
#[test]
fn f_mode_03_cycle() {
    let mut s = Sim::builder().build();
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md +2 -1");
    s.git(&["add", "src/c.tsx"]);
    s.keys("<Tab><Tab>");
    s.assert_row_matches(0, r"^\[all\] .*\[3/4\] \[cursor L1:C1\] src/c\.tsx \+1 -1$");
    s.keys("m");
    // staged diff has only the staged file
    s.assert_row(0, "[staged] [full] [unified] [solarized] [mcp: off] [1/1] [cursor L1:C1] src/c.tsx +1 -1");
    s.assert_row_matches(2, r"^@@ -1 \+1 @@");
    s.assert_row_matches(3, r"^   1      -▶export const C = \(\) => <div>hi</div>;");
    s.keys("m");
    // unstaged: c.tsx fully staged and untracked x.bin not listed; index 1 of the new list
    s.assert_row(
        0,
        "[unstaged] [full] [unified] [solarized] [mcp: off] [1/2] [cursor L2:C1] README.md +2 -1",
    );
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"^\[unstaged\] .*\[2/2\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
    s.keys("m");
    // back to all; index 1 again (README.md), position per F-NAV-08 on the other path
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md +2 -1");
    s.assert_row_matches(-1, r"^\(1-5/5\) ");
}

/// F-MODE-03: `m` from file 2 jumps to index 1 of the new list.
#[test]
fn f_mode_03_other_index_path() {
    let mut s = Sim::builder().size(120, 20).build();
    s.git(&["add", "src/big.ts"]);
    s.keys("<Tab>");
    s.assert_row_matches(0, r"^\[all\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
    s.assert_row_matches(-1, r"^\(28-44/62\) ");
    s.keys("5j3l");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L34:C4\] src/big\.ts ");
    s.keys("m");
    // same path as before m (src/big.ts, was index 2, staged list has it at index 1): cursor kept (F-RELOAD-03),
    // top reset to 0 then follow (F-NAV-10) puts the cursor 2 rows above the bottom
    s.assert_row(
        0,
        "[staged] [full] [unified] [solarized] [mcp: off] [1/1] [cursor L34:C4] src/big.ts +1 -1",
    );
    s.assert_row_matches(-1, r"^\(22-38/62\) ");
}

/// F-MODE-03: `m` landing on the same path keeps cursor line/col; viewport reset to top then follow (F-NAV-10)
/// instead of F-NAV-08.
#[test]
fn f_mode_03_same_path_cursor_kept() {
    let mut s = Sim::builder().size(120, 20).file("README.md", &readme(60)).hold_io().build();
    s.git(&["add", "README.md"]);
    s.release_io();
    // H=17, m=2; README rows = hunk, 2 context, 58 adds; start on first add (L3)
    s.assert_row_matches(0, r"^\[all\] .*\[1/4\] \[cursor L3:C1\] README\.md \+58 -0$");
    s.assert_row_matches(-1, r"^\(1-17/61\) ");
    s.keys("40j10k2l");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L33:C3\] README\.md ");
    s.assert_row_matches(-1, r"^\(30-46/61\) ");
    s.keys("m");
    // staged list = [README.md], same path as before m -> cursor kept (L33 col 3); top reset to 0 then follow
    // puts the cursor 2 rows above the bottom
    s.assert_row(
        0,
        "[staged] [full] [unified] [solarized] [mcp: off] [1/1] [cursor L33:C3] README.md +58 -0",
    );
    s.assert_row_matches(-1, r"^\(20-36/61\) ");
    s.assert_row_matches(16, r"^       33 \+▶l33");
    s.keys("m");
    // unstaged list = [src/big.ts, src/c.tsx]; index 1 is another path -> F-NAV-08
    s.assert_row(
        0,
        "[unstaged] [full] [unified] [solarized] [mcp: off] [1/2] [cursor L30:C1] src/big.ts +1 -1",
    );
    s.assert_row_matches(-1, r"^\(28-44/62\) ");
}

// ---------------------------------------------------------------- F-MODE-04

const AMBIGUOUS: &str =
    "fatal: ambiguous argument 'nosuchrev': unknown revision or path not in the working tree.";

/// F-MODE-04: bad git args: whole screen is git stderr in red, no header/footer; failing `r` keeps it; succeeding
/// `r` restores the UI.
#[test]
fn f_mode_04_bad_args() {
    let mut s = Sim::builder().size(120, 20).args(["nosuchrev"]).build();
    s.assert_row(0, AMBIGUOUS);
    s.assert_row(1, "Use '--' to separate paths from revisions, like this:");
    s.assert_row(2, "'git <command> [<revision>...] -- [<file>...]'");
    s.assert_row(-1, "");
    for t in ["[all]", "─", "hjkl move", "(0-0/0)", "No changes"] {
        s.assert_not_contains(t);
    }
    s.assert_cell(s.find_in_row(0, "fatal: ambiguous").unwrap().x, 0, C::new().fg("1"));
    s.assert_cell(s.find_in_row(1, "Use ").unwrap().x, 1, C::new().fg("1"));
    // r retries; still failing -> error screen stays
    s.keys("r");
    s.assert_row(0, AMBIGUOUS);
    for t in ["[all]", "hjkl move", "reloaded"] {
        s.assert_not_contains(t);
    }
    s.git(&["branch", "nosuchrev"]);
    // now the rev exists; r succeeds and the normal UI returns (extra args -> no untracked x.bin)
    s.keys("r");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/3] [cursor L2:C1] README.md +2 -1");
    s.assert_row_matches(2, r"^@@ -1,2 \+1,3 @@");
    s.assert_row_matches(-1, r"\(1-5/5\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("fatal");
}

/// F-MODE-04: repo without HEAD (no commit yet) shows git's ambiguous-HEAD error as the whole screen.
#[test]
fn f_mode_04_no_head() {
    let s = Sim::builder()
        .fixture(Fixture::NoGit)
        .size(120, 20)
        .file(".git/HEAD", "ref: refs/heads/main\n")
        .file(".git/config", "[core]\n\trepositoryformatversion = 0\n\tbare = false\n")
        .file(".git/objects/.keep", "")
        .file(".git/refs/heads/.keep", "")
        .file("a.txt", "one\n")
        .build();
    s.assert_row(0, "fatal: ambiguous argument 'HEAD': unknown revision or path not in the working tree.");
    s.assert_row(1, "Use '--' to separate paths from revisions, like this:");
    for t in ["[all]", "hjkl move", "No changes", "─"] {
        s.assert_not_contains(t);
    }
    s.assert_cell(s.find_in_row(0, "fatal").unwrap().x, 0, C::new().fg("1"));
}

/// F-MODE-04: not-a-repo error screen; `r` in a repo without HEAD still fails and screen stays; after first commit
/// `r` restores the UI.
#[test]
fn f_mode_04_no_repo_no_head() {
    let mut s = Sim::builder().fixture(Fixture::NoGit).size(120, 150).build();
    // outside a repo the text is always git diff's own stderr (its --no-index usage), never the ls-files error
    s.assert_row(
        0,
        "warning: Not a git repository. Use --no-index to compare two paths outside a working tree",
    );
    s.assert_row_matches(1, r"^usage: git diff --no-index ");
    for t in ["fatal: not a git repository", "[all]", "hjkl move", "No changes", "─"] {
        s.assert_not_contains(t);
    }
    s.assert_cell(0, 0, C::new().fg("1"));
    s.git(&["init", "-q"]);
    s.keys("r");
    // repo without HEAD: load fails again, error screen stays (which error text is shown is not pinned)
    for t in ["[all]", "hjkl move", "No changes"] {
        s.assert_not_contains(t);
    }
    s.write_file("a.txt", "one\n");
    s.git(&["add", "a.txt"]);
    s.git(&["commit", "-qm", "first"]);
    s.write_file("a.txt", "one\ntwo\n");
    s.keys("r");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/1] [cursor L2:C1] a.txt +1 -0");
    s.assert_row_matches(-1, r"^reloaded \| \(1-3/3\) hjkl move  enter ask  J/K comments  \? help$");
    s.assert_not_contains("fatal");
}

/// F-MODE-04: missing `--cwd` dir (git never runs) shows the error screen in red; Ctrl+C exits 0 from it.
#[test]
fn f_mode_04_spawn_error_ctrl_c() {
    let mut s = Sim::builder().size(240, 20).args(["--cwd", "${TMP}/does-not-exist"]).build();
    let want = s.expand("cannot open directory ${TMP}/does-not-exist: not found");
    assert_eq!(s.row(0), want);
    s.assert_not_contains("[all]");
    s.assert_not_contains("hjkl move");
    s.assert_text_cell("cannot open directory", 0, C::new().fg("1"));
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

// ---------------------------------------------------------------- F-MODE-05

/// F-MODE-05: on the no-changes screen m, f, F, ?, C, M, E, r, q all work.
#[test]
fn f_mode_05_keys() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(100, 24).build();
    s.assert_row(0, NC);
    s.keys("m");
    s.assert_row(0, "[staged] [mcp: off] No changes (m cycles mode, F search, q quits)");
    s.keys("m");
    s.assert_row(0, "[unstaged] [mcp: off] No changes (m cycles mode, F search, q quits)");
    s.keys("m");
    s.assert_row(0, NC);
    s.keys("f");
    // empty file picker
    s.assert_contains(" Files (1/0)");
    s.keys("<Esc>");
    s.assert_not_contains("Files (");
    s.keys("F");
    s.assert_contains(" Search (");
    s.keys("<Esc>");
    s.assert_not_contains("Search (");
    s.keys("?");
    s.assert_contains(" Help · Diff view");
    s.keys("??");
    s.assert_not_contains(" Help · ");
    s.keys("C");
    s.assert_matches("│ Config +│");
    s.assert_contains("confirm quit");
    s.keys("<Esc>");
    s.assert_not_contains("confirm quit");
    s.keys("M");
    s.assert_matches("│ MCP +│");
    s.assert_contains("○ off");
    s.keys("<Esc>");
    s.assert_not_contains("○ off");
    s.keys("E");
    s.assert_row(-1, &format!("no comments to export | (0-0/0) {HELP_FOOT}"));
    s.keys("r");
    s.assert_row(0, NC);
    s.assert_row(-1, &format!("reloaded | (0-0/0) {HELP_FOOT}"));
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

/// F-MODE-05: no-changes header mcp chip reflects the running server.
#[test]
fn f_mode_05_mcp_chip() {
    let mut s =
        Sim::builder().fixture(Fixture::Empty).config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row(0, "[all] [mcp: on] No changes (m cycles mode, F search, q quits)");
    s.assert_row(-1, &format!("(0-0/0) {HELP_FOOT}"));
    s.keys("M<Enter><Esc>");
    // power row Enter stops the server
    s.assert_row(0, NC);
}

/// F-MODE-05: no-changes screen: header, empty viewport, no cursor tag, (0-0/0) footer; arrows/Tab/cursor moves
/// are no-ops.
#[test]
fn f_mode_05_screen() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(100, 24).build();
    let check = |s: &Sim| {
        s.assert_row(0, NC);
        s.assert_row(-1, &format!("(0-0/0) {HELP_FOOT}"));
        for r in 2..=22 {
            assert_eq!(s.row(r), "", "row {r} must be empty\n{}", s.dump());
        }
        for t in ["[cursor", "[full]", "[unified]", "[solarized]", "▶"] {
            s.assert_not_contains(t);
        }
    };
    check(&s);
    s.keys("<Tab><S-Tab><Down><Up><Left><Right>");
    check(&s);
    s.keys("jkhlgGwb5j]<PageDown><Space>");
    check(&s);
}

/// F-MODE-05: t, s, c on the no-changes screen change state silently; visible once changes exist.
#[test]
fn f_mode_05_silent_state() {
    let mut s = Sim::builder().fixture(Fixture::Empty).size(120, 24).build();
    s.assert_row(0, NC);
    s.keys("t");
    s.assert_row(0, NC);
    s.keys("s");
    s.assert_row(0, NC);
    s.keys("c");
    s.assert_row(0, NC);
    s.write_file("n.txt", "new\n");
    s.keys("r");
    // untracked file now shows; theme vibrant, split on, changes scope took effect
    s.assert_row_matches(
        0,
        r"^\[all\] \[changes\] \[split\] \[vibrant\] \[mcp: off\] \[1/1\] \[cursor [^\]]*\] n\.txt \+1 -0$",
    );
}

/// F-MODE-05: no-changes screen with split on at cols < 100: too narrow part, never p pane; p is a no-op.
#[test]
fn f_mode_05_split_narrow_p() {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(["--split"]).size(80, 24).build();
    let foot = format!("too narrow for split | (0-0/0) {HELP_FOOT}");
    s.assert_row(-1, &foot);
    s.keys("p");
    s.assert_row(0, NC);
    s.assert_row(-1, &foot);
}

/// F-MODE-05: no-changes screen with split on (effective width): footer never has p pane; p is a no-op.
#[test]
fn f_mode_05_split_p() {
    let mut s = Sim::builder().fixture(Fixture::Empty).args(["--split"]).size(120, 24).build();
    let foot = format!("(0-0/0) {HELP_FOOT}");
    s.assert_row(0, NC);
    s.assert_row(-1, &foot);
    s.assert_not_contains("p pane");
    s.assert_not_contains("[cursor");
    s.keys("p");
    // p changes nothing (no note, no pane, same footer)
    s.assert_row(0, NC);
    s.assert_row(-1, &foot);
    // s toggles split silently; footer the same with split off and on again
    s.keys("s");
    s.assert_row(-1, &foot);
    s.keys("sp");
    s.assert_row(0, NC);
    s.assert_row(-1, &foot);
}

// ---------------------------------------------------------------- F-RELOAD-01

/// F-RELOAD-01: `r` in browse re-reads the browsed file; read error ignored, old text kept.
#[test]
fn f_reload_01_browse() {
    let mut s = Sim::builder().build();
    s.keys("F");
    s.keys("src/a.ts<Enter>");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L1:C1\] src/a\.ts");
    s.assert_row_matches(2, r"^   1  ▶export const a = 2;");
    s.assert_row_matches(-1, r"^\(1-3/3\) ");
    s.write_file("src/a.ts", "export const a = 3;\nline two\nline three\nline four\n");
    s.keys("r");
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L1:C1\] src/a\.ts");
    s.assert_row_matches(2, r"^   1  ▶export const a = 3;");
    s.assert_row_matches(5, r"^   4   line four");
    s.assert_row_matches(-1, r"^reloaded \| \(1-4/4\) ");
    s.remove_file("src/a.ts");
    s.keys("jr");
    // file gone: read error ignored, old text still shown, still browsing
    s.assert_row_matches(0, r"^\[browse\] .*\[cursor L2:C1\] src/a\.ts");
    s.assert_row_matches(2, r"^   1   export const a = 3;");
    s.assert_row_matches(5, r"^   4   line four");
    s.assert_row_matches(-1, r"^reloaded \| \(1-4/4\) ");
    s.assert_not_contains("cannot read");
}

/// F-RELOAD-01: `r` picks up worktree changes; same file path kept (also when its index moves); path gone -> first
/// file.
#[test]
fn f_reload_01_changed() {
    let mut s = Sim::builder().build();
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [1/4] [cursor L2:C1] README.md +2 -1");
    s.write_file("README.md", "# Title\nhello world\nmore\nextra line\n");
    // nothing re-read until r
    s.assert_not_contains("extra line");
    s.keys("r");
    s.assert_row_matches(0, r"^\[all\] .*\[1/4\] \[cursor L2:C1\] README\.md \+3 -1$");
    s.assert_row_matches(7, r"^        4 \+ extra line");
    s.assert_row_matches(-1, r"^reloaded \| \(1-6/6\) ");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts ");
    s.git(&["checkout", "--", "README.md"]);
    s.keys("r");
    // README.md dropped out, src/big.ts still shown, now index 1
    s.assert_row_matches(0, r"^\[all\] .*\[1/3\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/3\] .*src/c\.tsx ");
    s.git(&["checkout", "--", "src/c.tsx"]);
    s.keys("r");
    // shown path gone -> first file
    s.assert_row_matches(0, r"^\[all\] .*\[1/2\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
}

/// F-RELOAD-01: `r` works while a comment is focused; focus and box stay.
#[test]
fn f_reload_01_focused_comment() {
    let mut s = Sim::builder().build();
    s.keys("a");
    s.keys("my note<Enter>");
    s.assert_contains("my note");
    s.assert_row_matches(-1, r"^question saved \(1\) \| ");
    s.keys("J");
    s.assert_row_matches(-1, r"e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$");
    s.keys("r");
    s.assert_contains("▸ sent  line L2");
    s.assert_contains("my note");
    s.assert_row_matches(
        -1,
        r"^reloaded \| \(\d+-\d+/\d+\) e edit  D delete  a ask/follow up  j/k scroll  esc back  \? help$",
    );
}

/// F-RELOAD-01: `r` with failing diff load: note is the git error, no error screen, old files kept; fixed later `r`
/// reloads.
#[test]
fn f_reload_01_load_error() {
    let mut s = Sim::builder().build();
    s.keys("<Tab>");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [2/4] [cursor L30:C1] src/big.ts +1 -1");
    // HEAD now points to a branch whose object does not exist, so `git diff HEAD` fails (bad object HEAD)
    s.write_file(".git/refs/heads/bogus", "0123456789012345678901234567890123456789\n");
    s.git(&["symbolic-ref", "HEAD", "refs/heads/bogus"]);
    s.keys("r");
    // error only as note, made one line (footer stays the last row); diff rows still shown, no error screen
    s.assert_matches(r"(?m)^  30      -▶const v30 = 1;");
    s.assert_not_contains("reloaded");
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: off] [2/4] [cursor L30:C1] src/big.ts +1 -1");
    s.assert_row_matches(-1, r"^fatal: bad object HEAD \| \(\d+-\d+/62\) hjkl move");
    // old file list still in use (checked by the header after the next successful reload)
    s.keys("<Tab>");
    s.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    s.write_file("src/c.tsx", "export const C = () => <div>changed</div>;\n");
    s.keys("r");
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] src/c\.tsx \+1 -1$");
    s.assert_row_matches(4, "changed");
    s.assert_row_matches(-1, r"^reloaded \| \(1-3/3\) ");
}

/// F-RELOAD-01: `r` with nothing changed: note `reloaded`, header, rows, cursor and viewport unchanged.
#[test]
fn f_reload_01_unchanged() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("<Tab>5j2l");
    let same = |s: &Sim| {
        s.assert_row(
            0,
            "[all] [full] [unified] [solarized] [mcp: off] [2/4] [cursor L34:C3] src/big.ts +1 -1",
        );
        s.assert_row_matches(2, r"^  27   27   const v27 = 1;");
        s.assert_row_matches(10, r"^  34   34  ▶const v34 = 1;");
        s.assert_row_matches(18, r"^  42   42   const v42 = 1;");
    };
    same(&s);
    s.assert_row(-1, &format!("(28-44/62) {HELP_FOOT}"));
    s.keys("r");
    same(&s);
    s.assert_row(-1, &format!("reloaded | (28-44/62) {HELP_FOOT}"));
}

// ---------------------------------------------------------------- F-RELOAD-02

/// F-RELOAD-02: agent reload with failing diff load: errors ignored, no note, no error screen, old diff kept.
#[test]
fn f_reload_02_error_silent() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: on] [1/4] [cursor L2:C1] README.md +2 -1");
    s.write_file(".git/refs/heads/bogus", "0123456789012345678901234567890123456789\n");
    s.git(&["symbolic-ref", "HEAD", "refs/heads/bogus"]);
    let r = s.mcp_call("files_changed", json!({"paths": ["README.md"]}));
    assert_eq!(r.tool_result()["ok"], true);
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: on] [1/4] [cursor L2:C1] README.md +2 -1");
    s.assert_row_matches(4, r"^   2      -▶hello");
    s.assert_row(-1, &format!("(1-5/5) {HELP_FOOT}"));
    s.assert_not_contains("fatal");
    s.assert_not_contains("reloaded");
}

/// F-RELOAD-02: MCP `files_changed` reloads the diff silently (no note), same file kept.
#[test]
fn f_reload_02_files_changed() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: on] [1/4] [cursor L2:C1] README.md +2 -1");
    s.assert_row(-1, &format!("(1-5/5) {HELP_FOOT}"));
    s.write_file("README.md", "# Title\nhello world\nmore\nagent was here\n");
    s.assert_not_contains("agent was here");
    let r = s.mcp_call("files_changed", json!({"paths": ["README.md"]}));
    assert_eq!(r.status, 200);
    assert_eq!(r.tool_result()["ok"], true);
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: on] [1/4] [cursor L2:C1] README.md +3 -1");
    s.assert_row_matches(7, r"^        4 \+ agent was here");
    s.assert_row(-1, &format!("(1-6/6) {HELP_FOOT}"));
    s.assert_not_contains("reloaded");
    // paths is optional
    s.write_file("src/c.tsx", "export const C = () => <p>x</p>;\n");
    s.keys("<S-Tab><S-Tab>");
    s.assert_row_matches(0, r"\[3/4\] .*src/c\.tsx \+1 -1$");
    s.assert_not_contains("<p>x</p>");
    let r = s.mcp_call("files_changed", json!({}));
    assert_eq!(r.tool_result()["ok"], true);
    s.assert_row_matches(0, r"\[3/4\] \[cursor L1:C1\] src/c\.tsx \+1 -1$");
    s.assert_row_matches(4, "<p>x</p>");
    s.assert_row(-1, &format!("(1-3/3) {HELP_FOOT}"));
}

/// F-RELOAD-02: successful agent reload replaces the git error screen with the normal UI (F-MODE-04).
#[test]
fn f_reload_02_restores_error_screen() {
    let mut s = Sim::builder().args(["nosuchrev"]).config_json(json!({"mcp": {"autostart": true}})).build();
    let token = s.file("${STATE}/xplain/mcp.json");
    assert!(token.contains("token"), "token file: {token}");
    s.assert_row_matches(0, "^fatal: ambiguous argument 'nosuchrev'");
    s.assert_not_contains("[all]");
    s.assert_not_contains("hjkl move");
    let r = s.mcp_call("files_changed", json!({}));
    assert_eq!(r.tool_result()["ok"], true);
    // still failing: error screen stays
    s.assert_row_matches(0, "^fatal: ambiguous argument 'nosuchrev'");
    s.git(&["branch", "nosuchrev"]);
    let r = s.mcp_call("files_changed", json!({"paths": ["README.md"]}));
    assert_eq!(r.tool_result()["ok"], true);
    s.assert_row(0, "[all] [full] [unified] [solarized] [mcp: on] [1/3] [cursor L2:C1] README.md +2 -1");
    s.assert_row(-1, &format!("(1-5/5) {HELP_FOOT}"));
    s.assert_not_contains("fatal");
}

// ---------------------------------------------------------------- F-RELOAD-03

/// F-RELOAD-03: comment boxes re-anchor to their line number after reload shifts rows.
#[test]
fn f_reload_03_comment_reanchor() {
    let mut s = Sim::builder().size(120, 24).build();
    s.keys("<Tab>15j");
    s.keys("a");
    s.keys("anchored here<Enter>");
    s.assert_matches(
        r"(?m)^  44   44  ▶const v44 = 1;[^\n]*\n╭─+╮\n│ sent  line L44 [^\n]*\n│ anchored here",
    );
    // 5 lines inserted at the top; new line 44 is now old line 39
    s.write_file("src/big.ts", &big(&["// a", "// b", "// c", "// d", "// e"], 1, 60, &[(30, 2)]));
    s.keys("r");
    s.assert_matches(
        r"(?m)^  39   44  ▶const v39 = 1;[^\n]*\n╭─+╮\n│ sent  line L44 [^\n]*\n│ anchored here",
    );
    s.assert_not_contains("  44   49   const v44 = 1;\n╭");
}

/// F-RELOAD-03: reload keeps the cursor on the deleted row of the same old line number (not the add row with the
/// same number).
#[test]
fn f_reload_03_deleted_flag() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("<Tab>");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
    s.assert_row_matches(5, r"^  30      -▶const v30 = 1;");
    s.assert_row_matches(-1, r"^\(28-44/62\) ");
    // extra change on line 10 adds a row above
    s.write_file("src/big.ts", &big(&[], 1, 60, &[(10, 3), (30, 2)]));
    s.keys("r");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L30:C1\] src/big\.ts \+2 -2$");
    s.assert_row_matches(6, r"^  30      -▶const v30 = 1;");
    s.assert_row_matches(7, r"^       30 \+ const v30 = 2;");
    s.assert_row_matches(-1, r"^reloaded \| \(28-44/63\) ");
}

/// F-RELOAD-03: cursor line gone after reload -> nearest new-side line number.
#[test]
fn f_reload_03_line_gone() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("<Tab>G5k");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L55:C1\] src/big\.ts \+1 -1$");
    s.assert_row_matches(-1, r"^\(46-62/62\) ");
    // file cut to 40 lines; new-side 55 gone (old 41-60 are del rows now) -> nearest new-side line 40
    s.write_file("src/big.ts", &big(&[], 1, 40, &[(30, 2)]));
    s.keys("r");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L40:C1\] src/big\.ts \+1 -21$");
    s.assert_row_matches(4, r"^  40   40  ▶const v40 = 1;");
    s.assert_row_matches(5, r"^  41      - const v41 = 1;");
    s.assert_row_matches(-1, r"^reloaded \| \(40-56/62\) ");
}

/// F-RELOAD-03: reload keeps the cursor on the same line number (and column) when rows shift; top kept then follow.
#[test]
fn f_reload_03_line_kept() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("<Tab>15j2l");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L44:C3\] src/big\.ts \+1 -1$");
    s.assert_row_matches(16, r"^  44   44  ▶const v44 = 1;");
    s.assert_row_matches(-1, r"^\(32-48/62\) ");
    // lines 1-5 deleted -> 5 del rows at top; new line 44 moves from row 46 to row 51
    s.write_file("src/big.ts", &big(&[], 6, 60, &[(30, 2)]));
    s.keys("r");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L44:C3\] src/big\.ts \+1 -6$");
    s.assert_row_matches(16, r"^  49   44  ▶const v49 = 1;");
    s.assert_row_matches(-1, r"^reloaded \| \(37-53/62\) ");
}

/// F-RELOAD-03: cursor on a row without line number (hunk header) keeps its row index, clamped to the last row.
#[test]
fn f_reload_03_no_number_row() {
    let mut s = Sim::builder()
        .size(120, 40)
        .args(["--changes-only"])
        .file("src/big.ts", &big(&[], 1, 60, &[(5, 5), (30, 2), (55, 5)]))
        .build();
    s.keys("<Tab>10G");
    s.assert_row_matches(0, r"\[2/4\] \[cursor r10:C1\] src/big\.ts \+3 -3$");
    s.assert_row_matches(11, r"^@@ -27,7 \+27,7 @@");
    // change on line 5 reverted - row index 9 is now the hunk of line 55
    s.write_file("src/big.ts", &big(&[], 1, 60, &[(30, 2), (55, 5)]));
    s.keys("r");
    s.assert_row_matches(0, r"\[2/4\] \[cursor r10:C1\] src/big\.ts \+2 -2$");
    s.assert_row_matches(2, r"^@@ -27,7 \+27,7 @@");
    s.assert_row_matches(11, r"^@@ -52,7 \+52,7 @@");
    s.assert_row_matches(-1, r"^reloaded \| \(1-18/18\) ");
    // only one hunk left (9 rows) - row index 9 clamped to the last row (line 33)
    s.write_file("src/big.ts", &big(&[], 1, 60, &[(30, 2)]));
    s.keys("r");
    s.assert_row_matches(0, r"\[2/4\] \[cursor L33:C1\] src/big\.ts \+1 -1$");
    s.assert_row_matches(10, r"^  33   33  ▶const v33 = 1;");
    s.assert_row_matches(-1, r"^reloaded \| \(1-9/9\) ");
}

/// F-RELOAD-03: split view: reload keeps pane and old-side line number when rows shift.
#[test]
fn f_reload_03_split_pane() {
    let mut s = Sim::builder().size(120, 20).args(["--split"]).build();
    s.keys("<Tab>p");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[split\] .*\[2/4\] \[cursor old L30:C1\] src/big\.ts \+1 -1$",
    );
    s.assert_row_matches(5, r"^  30 -▶const v30 = 1;.*│  30 \+▶const v30 = 2;");
    s.assert_row_matches(-1, r"^\(28-44/61\) ");
    // 5 lines inserted at the top -> old line 30 pairs with new line 35, 5 rows further down
    s.write_file("src/big.ts", &big(&["// a", "// b", "// c", "// d", "// e"], 1, 60, &[(30, 2)]));
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[split\] .*\[2/4\] \[cursor old L30:C1\] src/big\.ts \+6 -1$",
    );
    s.assert_row_matches(10, r"^  30 -▶const v30 = 1;.*│  35 \+▶const v30 = 2;");
    s.assert_row_matches(-1, r"^reloaded \| \(28-44/66\) ");
}

/// F-RELOAD-03: reload that shrinks the rows: cursor to nearest line, kept top clamped to the new bottom offset.
#[test]
fn f_reload_03_top_clamped() {
    let mut s = Sim::builder().size(120, 20).file("README.md", &readme(60)).build();
    s.keys("G");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L60:C1\] README\.md \+58 -0$");
    s.assert_row_matches(-1, r"^\(45-61/61\) ");
    s.write_file("README.md", &readme(30));
    s.keys("r");
    s.assert_row_matches(0, r"\[1/4\] \[cursor L30:C1\] README\.md \+28 -0$");
    s.assert_row_matches(-2, r"^       30 \+▶l30");
    s.assert_row_matches(-1, r"^reloaded \| \(15-31/31\) ");
}

// ---------------------------------------------------------------- F-SCOPE

/// F-SCOPE-01: `--changes-only` shows only the git context hunk with `[changes]` chip.
#[test]
fn f_scope_01_changes_flag() {
    let mut s = Sim::builder().size(120, 20).args(["--changes-only"]).build();
    s.keys("<Tab>");
    s.assert_row_matches(
        0,
        r"^\[all\] \[changes\] \[unified\] .*\[2/4\] \[cursor r1:C1\] src/big\.ts \+1 -1$",
    );
    s.assert_row_matches(2, r"^@@ -27,7 \+27,7 @@ const v26 = 1;");
    s.assert_row_matches(3, r"^  27   27   const v27 = 1;");
    s.assert_row_matches(6, r"^  30      - const v30 = 1;");
    s.assert_row_matches(7, r"^       30 \+ const v30 = 2;");
    s.assert_row_matches(10, r"^  33   33   const v33 = 1;");
    s.assert_row(11, "");
    s.assert_row_matches(-1, r"^\(1-9/9\) ");
    s.assert_not_contains("  26   26 ");
    s.assert_not_contains("  34   34 ");
}

/// F-SCOPE-01: default scope is full (whole file one hunk).
#[test]
fn f_scope_01_default_full() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("<Tab>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
    s.assert_row_matches(-1, r"^\(28-44/62\) ");
    s.keys("g");
    s.assert_row_matches(2, r"^@@ -1,60 \+1,60 @@");
    s.assert_row_matches(3, r"^   1    1   const v1 = 1;");
}

/// F-SCOPE-01: full scope shows the whole file as one hunk, `--changes-only` shows git's default context hunks;
/// chip `[full]`/`[changes]`.
#[test]
fn f_scope_01_hunks() {
    let mut s = Sim::builder()
        .size(120, 40)
        .args(["--changes-only"])
        .file("src/big.ts", &big(&[], 1, 60, &[(5, 5), (30, 2), (55, 5)]))
        .build();
    s.keys("<Tab>");
    // three separate changes (lines 5, 30, 55) -> three hunks with 3 context lines each
    s.assert_row_matches(
        0,
        r"^\[all\] \[changes\] \[unified\] .*\[2/4\] \[cursor r1:C1\] src/big\.ts \+3 -3$",
    );
    s.assert_row_matches(2, r"^@@ -2,7 \+2,7 @@");
    s.assert_row_matches(3, r"^   2    2  .const v2 = 1;");
    s.assert_row_matches(11, r"^@@ -27,7 \+27,7 @@");
    s.assert_row_matches(20, r"^@@ -52,7 \+52,7 @@");
    s.assert_row_matches(-1, r"^\(1-27/27\) ");
    for t in ["   1    1 ", "  60   60 ", "@@ -1,60"] {
        s.assert_not_contains(t);
    }
    s.keys("c");
    // full scope - one hunk covering all 60 lines (60 + 3 del rows + hunk row)
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[2/4\] \[cursor L5:C1\] src/big\.ts \+3 -3$");
    s.assert_row_matches(5, r"^   5      -▶const v5 = 1;");
    s.assert_row_matches(-1, r"^\(3-39/64\) ");
    s.keys("g");
    s.assert_row_matches(2, r"^@@ -1,60 \+1,60 @@");
    s.assert_row_matches(3, r"^   1    1   const v1 = 1;");
    s.assert_row_matches(-1, r"^\(1-37/64\) ");
    s.keys("G");
    s.assert_row_matches(-2, r"^  60   60  ▶const v60 = 1;");
    s.assert_row_matches(-1, r"^\(28-64/64\) ");
    s.assert_not_contains("@@ -52,7");
}

/// F-SCOPE-02: `c` is ignored in browse.
#[test]
fn f_scope_02_browse_ignored() {
    let mut s = Sim::builder().build();
    s.keys("F");
    s.keys("src/a.ts<Enter>");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L1:C1\] src/a\.ts");
    s.keys("jc");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] \[cursor L2:C1\] src/a\.ts");
    s.assert_row_matches(3, r"^   2  ▶export function f");
    s.keys("<Esc>");
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[1/4\] .*README\.md");
}

/// F-SCOPE-02: `c` keeps the shown path even when its index changes; path gone after reload -> first file.
#[test]
fn f_scope_02_path_kept() {
    let mut s = Sim::builder().build();
    s.keys("<Tab><Tab><Tab>");
    s.assert_row_matches(0, r"^\[all\] \[full\] .*\[4/4\] \[cursor L1:C1\] x\.bin \+1 -0$");
    s.git(&["checkout", "--", "README.md"]);
    s.keys("c");
    // README.md no longer changed; x.bin now at index 3 and still shown
    s.assert_row_matches(0, r"^\[all\] \[changes\] .*\[3/3\] \[cursor r1:C1\] x\.bin \+1 -0$");
    s.keys("<S-Tab>");
    s.assert_row_matches(0, r"\[2/3\] .*src/c\.tsx ");
    s.git(&["checkout", "--", "src/c.tsx"]);
    s.keys("c");
    // shown path vanished -> first file
    s.assert_row_matches(0, r"^\[all\] \[full\] .*\[1/2\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
}

/// F-SCOPE-02: `c` toggles scope, reloads, keeps the file; cursor and viewport reset per F-NAV-08 both ways.
#[test]
fn f_scope_02_toggle() {
    let mut s = Sim::builder().size(120, 20).build();
    s.keys("<Tab>3j");
    s.assert_row_matches(0, r"^\[all\] \[full\] .*\[2/4\] \[cursor L32:C1\] src/big\.ts ");
    s.assert_row_matches(-1, r"^\(28-44/62\) ");
    s.keys("c");
    // changes scope -> cursor on first row (hunk), top 0
    s.assert_row_matches(
        0,
        r"^\[all\] \[changes\] \[unified\] .*\[2/4\] \[cursor r1:C1\] src/big\.ts \+1 -1$",
    );
    s.assert_row_matches(2, r"^@@ -27,7 \+27,7 @@");
    s.assert_row_matches(-1, r"^\(1-9/9\) ");
    s.keys("4j2l");
    s.assert_row_matches(0, r"\[cursor L30:C3\] src/big\.ts");
    s.keys("c");
    // full scope -> cursor to first change row (del row of line 30), col 1, top = firstChange-3
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] .*\[2/4\] \[cursor L30:C1\] src/big\.ts \+1 -1$");
    s.assert_row_matches(5, r"^  30      -▶const v30 = 1;");
    s.assert_row_matches(-1, r"^\(28-44/62\) ");
}
