//! Ported from `e2e/scenarios/u01-cli`.
//!
//! Not portable in-process (see the port report): the alternate/main screen switch, stderr kept apart from the
//! PTY, and the git-shim screens (the sim runs real git for diff loads, so the "shimmed git prints nothing"
//! screens are replaced by checks of the emitted `LoadDiff` spec).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regex::Regex;
use xplain_core::diff::DiffSpec;
use xplain_core::effect::Effect;
use xplain_sim::{CellExpect as C, Fixture, Http, Sim};

const USAGE: &str = "usage: xplain [--cwd dir] [--config file] [--mode all|staged|unstaged | --staged | --unstaged] [git diff args...]
  --mode <m>   all (git diff HEAD, default), staged (--cached), unstaged
  --staged     same as --mode staged
  --unstaged   same as --mode unstaged
  --split      start in side-by-side view (s toggles)
  --changes-only  start with git hunks only, not the full file (c toggles)
  --theme <t>  solarized, vibrant, dull, contrast, colorblind, light (first is default) (t cycles)
  --config <f> config file (default $XPLAIN_CONFIG or ~/.config/xplain/config.json)
  -h, --help   show this help
xplain config path  print the resolved config path
extra git args replace HEAD in \"all\" mode, and are appended in the other modes.
keys: ? help, s split/unified, c full/changes, ]/[ next/prev change, m cycles mode, t cycles theme, C config, q quits
";

const THEMES: &str = "(solarized|vibrant|dull|contrast|colorblind|light)";

fn args(a: &[&str]) -> Sim {
    Sim::builder().args(a.iter().copied()).build()
}

/// Exit 1, empty stdout, stderr = `first` line + full usage.
#[track_caller]
fn assert_usage_error(s: &Sim, first: &str) {
    assert_eq!(s.exit_code(), Some(1));
    assert_eq!(s.stdout(), "");
    assert_eq!(s.stderr(), format!("{first}\n{USAGE}"));
}

fn specs(s: &Sim) -> Vec<DiffSpec> {
    s.effects()
        .iter()
        .filter_map(|e| match e {
            Effect::LoadDiff { spec, .. } => Some(spec.clone()),
            _ => None,
        })
        .collect()
}

fn argv(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| (*x).to_string()).collect()
}

// ---- F-CLI-01 ------------------------------------------------------------------------------------------------

#[test]
fn f_cli_01_help_after_bad_flag() {
    let s = args(&["--theme", "neon", "--help"]);
    assert_eq!(s.exit_code(), Some(1));
    assert_eq!(s.stdout(), "");
    assert_eq!(s.stderr(), format!("xplain: invalid theme: neon {THEMES}\n{USAGE}"));
}

#[test]
fn f_cli_01_help_anywhere() {
    let s = Sim::builder()
        .args(["--staged", "--split", "--theme", "dull", "some-rev", "--help", "other-arg"])
        .shim("git")
        .build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), USAGE);
    assert!(!s.stdout().contains('\x1b'));
    assert_eq!(s.stderr(), "");
    // no UI means no diff load, so git is never run
    assert!(specs(&s).is_empty());
    assert_eq!(s.calls("git").len(), 0);
}

#[test]
fn f_cli_01_help_before_bad_flag() {
    let s = args(&["-h", "--mode", "bogus"]);
    assert_eq!(s.exit_code(), Some(0));
    assert!(s.stdout().starts_with("usage: xplain ") && s.stdout().ends_with("q quits\n"));
    assert_eq!(s.stderr(), "");
}

#[test]
fn f_cli_01_help_verbatim() {
    let s = args(&["-h"]);
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), USAGE);
    assert_eq!(s.stderr(), "");
}

#[test]
fn f_cli_01_help() {
    let s = args(&["--help"]);
    assert_eq!(s.exit_code(), Some(0));
    assert!(s.stdout().starts_with("usage: xplain "));
    for t in ["--mode <m>", "-h, --help", "xplain config path"] {
        assert!(s.stdout().contains(t), "{t}");
    }
    assert_eq!(s.stderr(), "");
}

// ---- F-CLI-02 ------------------------------------------------------------------------------------------------

#[test]
fn f_cli_02_config_needs_value() {
    assert_usage_error(&args(&["--config"]), "xplain: --config needs a value");
}

#[test]
fn f_cli_02_cwd_needs_value() {
    assert_usage_error(&args(&["--staged", "--cwd"]), "xplain: --cwd needs a value");
}

#[test]
fn f_cli_02_mode_bad() {
    assert_usage_error(&args(&["--mode", "cached"]), "xplain: invalid mode: cached");
}

#[test]
fn f_cli_02_mode_eq_bad() {
    assert_usage_error(&args(&["--mode=Staged"]), "xplain: invalid mode: Staged");
}

#[test]
fn f_cli_02_mode_eq_empty() {
    assert_usage_error(&args(&["--mode="]), "xplain: invalid mode: ");
}

#[test]
fn f_cli_02_mode_missing() {
    assert_usage_error(&args(&["--split", "--mode"]), "xplain: invalid mode: (missing)");
}

#[test]
fn f_cli_02_theme_bad() {
    assert_usage_error(&args(&["--theme", "Dark"]), &format!("xplain: invalid theme: Dark {THEMES}"));
}

#[test]
fn f_cli_02_theme_eq_empty() {
    assert_usage_error(&args(&["--theme="]), &format!("xplain: invalid theme:  {THEMES}"));
}

#[test]
fn f_cli_02_theme_missing() {
    assert_usage_error(&args(&["--theme"]), &format!("xplain: invalid theme: (missing) {THEMES}"));
}

// ---- F-CLI-03 ------------------------------------------------------------------------------------------------

#[test]
fn f_cli_03_changes_only() {
    let s = args(&["--changes-only"]);
    s.assert_row_matches(0, r"^\[all\] \[changes\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
}

#[test]
fn f_cli_03_config_eq_empty() {
    let s = Sim::builder()
        .args(["--config="])
        .env("XPLAIN_CONFIG", "${TMP}/env.json")
        .file("${TMP}/env.json", r#"{"theme": "light"}"#)
        .file("${CONFIG}/xplain/config.json", r#"{"theme": "vibrant"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[light\] \[mcp: off\] ");
}

#[test]
fn f_cli_03_config_eq() {
    let s = Sim::builder()
        .args(["--config=${TMP}/custom.json"])
        .file("${TMP}/custom.json", r#"{"theme": "colorblind", "view": {"mode": "unstaged"}}"#)
        .file("${CONFIG}/xplain/config.json", r#"{"theme": "vibrant"}"#)
        .build();
    s.assert_row_matches(0, r"^\[unstaged\] \[full\] \[unified\] \[colorblind\] \[mcp: off\] \[1/3\] ");
}

#[test]
fn f_cli_03_config_space() {
    let s = Sim::builder()
        .args(["--config", "${TMP}/custom.json"])
        .file("${TMP}/custom.json", r#"{"theme": "dull", "view": {"split": true, "full": false}}"#)
        .file("${CONFIG}/xplain/config.json", r#"{"theme": "vibrant"}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[changes\] \[split\] \[dull\] \[mcp: off\] ");
}

/// git runs in that dir (checked on the emitted spec; the sim runs real git, and `${HOME}` is no repo).
#[test]
fn f_cli_03_cwd_git() {
    let s = Sim::builder().args(["--cwd", "${HOME}"]).shim("git").build();
    let sp = specs(&s);
    assert!(!sp.is_empty());
    let home = s.home().to_str().unwrap().to_string();
    for spec in &sp {
        assert_eq!(spec.cwd.as_deref(), Some(home.as_str()));
        assert_eq!(&spec.git_argv()[..3], &argv(&["diff", "--no-color", "--no-ext-diff"])[..]);
    }
    assert_eq!(s.calls("git").len(), 0);
}

#[test]
fn f_cli_03_cwd_repo() {
    let mut s = Sim::builder().args(["--cwd", "${TMP}/other"]).size(240, 30).build();
    let other = s.expand("${TMP}/other");
    // dir does not exist yet - checked before git runs (F-CLI-06)
    s.assert_contains(&format!("cannot open directory {other}: not found"));
    s.assert_not_contains("README.md");
    s.git(&["init", "-q", &other]);
    s.write_file("${TMP}/other/notes.txt", "one\n");
    s.git(&["-C", &other, "add", "-A"]);
    s.git(&["-C", &other, "-c", "commit.gpgsign=false", "commit", "-qm", "init"]);
    s.write_file("${TMP}/other/notes.txt", "one\ntwo\n");
    s.keys("r");
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/1\] .*notes\.txt \+1 -0$",
    );
    s.assert_contains("two");
    s.assert_not_contains("README.md");
    s.assert_not_contains("cannot open directory");
    // file search lists the --cwd repo and browse reads the file from there
    s.keys("F");
    s.assert_contains("notes.txt");
    s.assert_not_contains("README.md");
    s.keys("notes<Enter>");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] .*notes\.txt$");
    s.keys("<Esc>");
    s.keys("a");
    s.keys("hi<Enter>");
    s.assert_row_contains(-1, "question saved (1)");
    s.keys("E");
    let pat = format!(r"exported 1 comment -> {}/xplain-review-\d{{8}}-\d{{6}}\.md", regex::escape(&other));
    s.assert_row_matches(-1, &pat);
    // the export file lands in --cwd, not in the process cwd
    let names: Vec<String> = s
        .list_dir("${TMP}/other")
        .into_iter()
        .filter(|n| n.starts_with("xplain-review-") && n.ends_with(".md"))
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    let body = s.file(&format!("{other}/{}", names[0]));
    assert!(body.contains("- comments: 1\n"), "{body}");
    assert!(body.contains("> hi\n"), "{body}");
    assert!(s.list_dir("${REPO}").iter().all(|n| !n.starts_with("xplain-review-")));
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_03_defaults() {
    let s = Sim::builder().build();
    s.assert_row_matches(
        0,
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] .*README\.md \+2 -1$",
    );
    assert!(!s.row(-1).contains("p pane"));
}

#[test]
fn f_cli_03_flags_override_config() {
    let mut s = Sim::builder()
        .args(["--mode", "all", "--theme", "vibrant", "--split", "--changes-only"])
        .config(
            r#"{"theme": "dull", "view": {"mode": "unstaged", "split": false, "full": true}, "app": {"confirmQuit": false}}"#,
        )
        .build();
    s.assert_row_matches(0, r"^\[all\] \[changes\] \[split\] \[vibrant\] \[mcp: off\] \[1/4\] ");
    // confirmQuit false from config (no flag) still applies - q quits at once
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
}

/// Non-flag args (also unknown --x and --) go to git in order; in all mode they replace HEAD.
#[test]
fn f_cli_03_git_args_all() {
    let s = Sim::builder().args(["v1", "--x", "--", "a b.txt"]).shim("git").build();
    let sp = specs(&s);
    assert_eq!(sp.len(), 1);
    assert_eq!(
        sp[0].git_argv(),
        argv(&["diff", "--no-color", "--no-ext-diff", "-U1000000", "v1", "--x", "--", "a b.txt"])
    );
    assert_eq!(sp[0].cwd, None);
    assert!(!sp[0].git_argv().contains(&"HEAD".to_string()));
    assert_eq!(s.calls("git").len(), 0);
}

#[test]
fn f_cli_03_git_args_staged() {
    let s = Sim::builder().args(["v1", "--staged", "--x", "--changes-only", "--", "bar"]).shim("git").build();
    let sp = specs(&s);
    assert_eq!(sp.len(), 1);
    assert_eq!(
        sp[0].git_argv(),
        argv(&["diff", "--no-color", "--no-ext-diff", "--cached", "v1", "--x", "--", "bar"])
    );
}

#[test]
fn f_cli_03_mode_eq() {
    let s = args(&["--mode=staged"]);
    s.assert_row_matches(0, r"^\[staged\] \[mcp: off\] No changes ");
    s.assert_not_contains("README.md");
}

#[test]
fn f_cli_03_mode_last_wins_shortcut() {
    let s = args(&["--unstaged", "--mode=all", "--staged"]);
    s.assert_row_matches(0, r"^\[staged\] \[mcp: off\] No changes ");
}

#[test]
fn f_cli_03_mode_last_wins() {
    let s = args(&["--staged", "--mode", "unstaged"]);
    s.assert_row_matches(0, r"^\[unstaged\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/3\] ");
}

#[test]
fn f_cli_03_mode_space() {
    let s = args(&["--mode", "unstaged"]);
    s.assert_row_matches(
        0,
        r"^\[unstaged\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/3\] .*README\.md \+2 -1$",
    );
}

#[test]
fn f_cli_03_split() {
    let s = args(&["--split"]);
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.assert_row_contains(-1, "hjkl move  enter ask  J/K comments  p pane  ? help");
}

#[test]
fn f_cli_03_staged() {
    let s = args(&["--staged"]);
    s.assert_row_matches(0, r"^\[staged\] \[mcp: off\] No changes ");
    s.assert_not_contains("README.md");
}

#[test]
fn f_cli_03_theme_eq() {
    let s = args(&["--theme=contrast"]);
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[contrast\] \[mcp: off\] ");
    let p = s.find_in_row(0, "[contrast]").expect("chip");
    s.assert_cell(p.x + 1, 0, C::new().fg("#ff00ff"));
}

#[test]
fn f_cli_03_theme_space() {
    let s = args(&["--theme", "light"]);
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[light\] \[mcp: off\] ");
    let p = s.find_in_row(0, "[light]").expect("chip");
    s.assert_cell(p.x + 1, 0, C::new().fg("#8f1f8f"));
}

#[test]
fn f_cli_03_unstaged() {
    let s = args(&["--unstaged"]);
    s.assert_row_matches(0, r"^\[unstaged\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/3\] ");
}

// ---- F-CLI-04 ------------------------------------------------------------------------------------------------

#[test]
fn f_cli_04_default() {
    let s = args(&["config", "path"]);
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
    assert_eq!(s.stderr(), "");
    assert!(!s.file_exists("${CONFIG}/xplain/config.json"));
}

#[test]
fn f_cli_04_env() {
    let s = Sim::builder().args(["config", "path"]).env("XPLAIN_CONFIG", "${TMP}/env.json").build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${TMP}/env.json\n"));
    assert_eq!(s.stderr(), "");
}

/// `config path extra` is not the subcommand: all three words go to git and the UI starts.
#[test]
fn f_cli_04_extra() {
    let mut s = Sim::builder().args(["config", "path", "extra"]).shim("git").build();
    let sp = specs(&s);
    assert_eq!(sp.len(), 1);
    assert_eq!(
        sp[0].git_argv(),
        argv(&["diff", "--no-color", "--no-ext-diff", "-U1000000", "config", "path", "extra"])
    );
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_04_flag() {
    let s = Sim::builder()
        .args(["--config", "/x/y/cfg.json", "config", "path"])
        .env("XPLAIN_CONFIG", "${TMP}/env.json")
        .build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), "/x/y/cfg.json\n");
    assert_eq!(s.stderr(), "");
}

#[test]
fn f_cli_04_flags_around() {
    let s = args(&["--staged", "config", "--theme", "dull", "--split", "path"]);
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
    assert_eq!(s.stderr(), "");
}

#[test]
fn f_cli_04_no_read() {
    let s = Sim::builder().args(["config", "path"]).config("{not json, theme: 7").build();
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
    assert_eq!(s.stderr(), "");
    assert_eq!(s.file("${CONFIG}/xplain/config.json"), "{not json, theme: 7");
}

// ---- F-CLI-05 ------------------------------------------------------------------------------------------------

/// Config warnings go to stderr, not into the UI.
#[test]
fn f_cli_05_alt_screen_warnings() {
    let mut s = Sim::builder().config(r#"{"theme": "nope", "app": {"confirmQuit": false}}"#).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.assert_not_contains("xplain: config:");
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
    assert!(s.stderr().contains(
        r#"xplain: config: invalid theme "nope" (solarized|vibrant|dull|contrast|colorblind|light); using solarized"#
    ));
}

#[test]
fn f_cli_05_ctrl_c_config_modal() {
    let mut s = Sim::builder().build();
    s.keys("C");
    s.assert_contains(" Config");
    s.assert_matches("confirm quit");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
    assert!(!s.file_exists("${CONFIG}/xplain/config.json"));
}

#[test]
fn f_cli_05_ctrl_c_diff() {
    let mut s = Sim::builder().build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
    s.assert_not_contains("Quit xplain?");
}

#[test]
fn f_cli_05_ctrl_c_editor() {
    let mut s = Sim::builder().build();
    s.keys("a");
    s.keys("hello");
    s.assert_contains("hello");
    s.assert_contains("enter send");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_05_ctrl_c_error_screen() {
    let mut s = Sim::builder().fixture(Fixture::NoGit).build();
    s.assert_not_contains("[mcp: off]");
    s.assert_not_contains("hjkl move");
    s.assert_matches(r"\S");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_05_ctrl_c_find() {
    let mut s = Sim::builder().build();
    s.keys("/ab");
    s.assert_row_contains(-1, "/ab█");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_05_ctrl_c_help() {
    let mut s = Sim::builder().build();
    s.keys("?");
    s.assert_contains("Help · Diff view");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_05_ctrl_c_no_changes() {
    let mut s = Sim::builder().fixture(Fixture::Empty).build();
    s.assert_row_matches(0, r"^\[all\] \[mcp: off\] No changes ");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_05_ctrl_c_quit_modal() {
    let mut s = Sim::builder().build();
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_05_ctrl_c_search() {
    let mut s = Sim::builder().build();
    s.keys("F");
    s.assert_contains(" Search (");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

/// Keys typed while loading are handled: q (confirm off) typed before the diff loads quits.
#[test]
fn f_cli_05_loading_keys() {
    let mut s = Sim::builder().hold_io().config(r#"{"app": {"confirmQuit": false}}"#).build();
    s.assert_contains("Loading...");
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
}

/// First frame before the diff loads is "Loading..." (dim) only; the loaded diff replaces it.
#[test]
fn f_cli_05_loading() {
    let mut s = Sim::builder().hold_io().build();
    assert!(Regex::new(r"\A\s*Loading\.\.\.\s*\z").unwrap().is_match(&s.text()), "{}", s.dump());
    assert!(s.cell_of("Loading...").dim);
    s.release_io();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.assert_not_contains("Loading");
    assert!(specs(&s).iter().any(|sp| sp.git_argv().contains(&"diff".to_string())));
}

#[test]
fn f_cli_05_warnings_stderr() {
    let mut s = Sim::builder().config(r#"{"theme": "nope", "app": {"confirmQuit": false}}"#).build();
    assert_eq!(
        s.stderr(),
        "xplain: config: invalid theme \"nope\" (solarized|vibrant|dull|contrast|colorblind|light); using solarized\n"
    );
    s.assert_row_matches(0, r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] ");
    s.assert_not_contains("xplain: config:");
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), "");
}

// ---- F-CLI-06 ------------------------------------------------------------------------------------------------

#[test]
fn f_cli_06_config_dash() {
    let s = Sim::builder()
        .args(["--config", "-c.json"])
        .file("-c.json", r#"{"theme": "light", "view": {"split": true}}"#)
        .build();
    s.assert_row_matches(0, r"^\[all\] \[full\] \[split\] \[light\] \[mcp: off\] ");
}

#[test]
fn f_cli_06_cwd_dash_h() {
    let mut s = args(&["--cwd", "-h"]);
    s.assert_contains("cannot open directory -h: not found");
    for t in ["usage: xplain", "[mcp: off]", "README.md"] {
        s.assert_not_contains(t);
    }
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_06_cwd_file() {
    let mut s = args(&["--cwd", "README.md"]);
    s.assert_row_matches(0, r"^cannot open directory README\.md: not a directory *$");
    s.assert_not_contains("[mcp: off]");
    s.assert_not_contains("hjkl move");
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_cli_06_cwd_help_config_path() {
    let s = args(&["--cwd", "--help", "config", "path"]);
    assert_eq!(s.exit_code(), Some(0));
    assert_eq!(s.stdout(), s.expand("${CONFIG}/xplain/config.json\n"));
    assert_eq!(s.stderr(), "");
}

/// `--cwd=`, `--split=x`, `--staged=x`, `--changes-only=x` are not flags: passed to git in order.
#[test]
fn f_cli_06_eq_not_flags() {
    let s = Sim::builder()
        .args(["--cwd=${HOME}", "--split=x", "--staged=x", "--changes-only=x"])
        .shim("git")
        .build();
    let sp = specs(&s);
    assert_eq!(sp.len(), 1);
    let home_arg = s.expand("--cwd=${HOME}");
    assert_eq!(
        sp[0].git_argv(),
        argv(&[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-U1000000",
            &home_arg,
            "--split=x",
            "--staged=x",
            "--changes-only=x"
        ])
    );
    assert_eq!(sp[0].cwd, None);
}

#[test]
fn f_cli_06_mode_dash() {
    assert_usage_error(&args(&["--mode", "--staged"]), "xplain: invalid mode: --staged");
}

#[test]
fn f_cli_06_theme_dash() {
    assert_usage_error(&args(&["--theme", "--split"]), &format!("xplain: invalid theme: --split {THEMES}"));
}

// ---- F-QUIT-01 -----------------------------------------------------------------------------------------------

#[test]
fn f_quit_01_browse() {
    let mut s = Sim::builder().build();
    s.keys("F");
    s.keys("package.json<Enter>");
    s.assert_row_matches(0, r"^\[browse\] \[solarized\] \[mcp: off\] .*package\.json$");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_row_matches(0, r"^\[browse\] ");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_confirm_close() {
    let mut s = Sim::builder().build();
    let head =
        r"^\[all\] \[full\] \[unified\] \[solarized\] \[mcp: off\] \[1/4\] \[cursor L2:C1\] README\.md ";
    s.assert_row_matches(0, head);
    for close in ["n", "q", "<Esc>"] {
        s.keys("q");
        s.assert_contains("Quit xplain? (y/n)");
        s.keys(close);
        s.assert_not_contains("Quit xplain?");
        s.assert_row_matches(0, r"^\[all\] .*README\.md ");
    }
    s.keys("q");
    // t (theme), s (split), j (cursor), Tab (next file), c (scope), x are ignored while the confirm is open
    s.keys("tsj<Tab>cx");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_row_matches(0, head);
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_confirm_enter() {
    let mut s = Sim::builder().build();
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("<Enter>");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_confirm_help() {
    let mut s = Sim::builder().build();
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("Help ·");
    s.keys("?");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_contains("Help · Confirm");
    s.keys("?");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_not_contains("Help ·");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_confirm_y() {
    let mut s = Sim::builder().build();
    s.assert_not_contains("Quit xplain?");
    s.assert_row_matches(0, r"^\[all\] ");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.assert_row_matches(0, r"^\[all\] ");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_mcp_running() {
    let mut s = Sim::builder().config(r#"{"mcp": {"autostart": true}}"#).build();
    s.assert_row_contains(0, "[mcp: on]");
    let _poll = s.http_start(Http::tool("next_question", serde_json::json!({"wait_seconds": 120})));
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_no_changes() {
    let mut s = Sim::builder().fixture(Fixture::Empty).build();
    s.assert_row_matches(0, r"^\[all\] \[mcp: off\] No changes ");
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("y");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_no_confirm() {
    let mut s = Sim::builder().config(r#"{"app": {"confirmQuit": false}}"#).build();
    s.assert_row_matches(0, r"^\[all\] \[full\] ");
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn f_quit_01_text_input() {
    let mut s = Sim::builder().config(r#"{"app": {"confirmQuit": false}}"#).build();
    s.keys("/");
    s.keys("q");
    s.assert_row_contains(-1, "/q█");
    s.keys("<Esc>");
    s.keys("a");
    s.keys("q");
    s.assert_contains("enter send");
    s.assert_row_matches(0, r"^\[all\] ");
    s.keys("<Esc>");
    s.assert_not_contains("enter send");
    s.keys("q");
    assert_eq!(s.exit_code(), Some(0));
}
