//! Ported from `e2e/scenarios/u14-mcpui` (MCP modal, integration registration, copy texts).

use serde_json::json;
use xplain_sim::{CellExpect as C, Http, Rule, Sim, SimBuilder};

const PORT: &str = "47615";
const TOKEN: &str = "tok-0123456789abcdefXYZ";

/// Builder with the MCP port pinned (the e2e runner passes a free `${PORT}`; the sim never listens).
fn base() -> SimBuilder {
    Sim::builder().env("XPLAIN_MCP_PORT", PORT)
}

/// Same, with a pre-written token file.
fn with_token() -> SimBuilder {
    base().file("${STATE}/xplain/mcp.json", &format!(r#"{{"token": "{TOKEN}"}}"#))
}

fn port(s: &str) -> String {
    s.replace("${PORT}", PORT)
}

fn get(exit: i32) -> Rule {
    Rule::args(["mcp", "get"]).exit(exit)
}

/// Every pattern matches the screen.
#[track_caller]
fn all(s: &Sim, patterns: &[&str]) {
    for p in patterns {
        s.assert_matches(&port(p));
    }
}

#[track_caller]
fn none(s: &Sim, texts: &[&str]) {
    for t in texts {
        s.assert_not_contains(t);
    }
}

/// Calls of `name` whose argv equals `args`.
fn n_exact(s: &Sim, name: &str, args: &[&str]) -> usize {
    s.calls(name).iter().filter(|c| c.args == args).count()
}

/// Calls of `name` whose argv starts with `prefix`.
fn n_prefix(s: &Sim, name: &str, prefix: &[&str]) -> usize {
    s.calls(name).iter().filter(|c| c.starts_with(prefix)).count()
}

#[track_caller]
fn calls_exact(s: &Sim, name: &str, args: &[&str], count: usize) {
    assert_eq!(n_exact(s, name, args), count, "{name} {args:?}: {:?}", s.calls(name));
}

#[track_caller]
fn calls_prefix(s: &Sim, name: &str, prefix: &[&str], count: usize) {
    assert_eq!(n_prefix(s, name, prefix), count, "{name} {prefix:?}: {:?}", s.calls(name));
}

// ---------------------------------------------------------------------------------------------------------------
// F-INTEG-01: argv per CLI

#[test]
fn f_integ_01_claude_argv() {
    let mut s = with_token().shim_rules("claude", vec![get(1)]).build();
    s.keys("M<Enter>");
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 1);
    let repo = s.repo().to_string_lossy().into_owned();
    assert!(s.calls("claude").iter().all(|c| c.cwd == repo), "cwd: {:?}", s.calls("claude"));
    calls_prefix(&s, "claude", &["mcp", "add"], 0);
    s.keys("j<Enter>y");
    calls_exact(&s, "claude", &["mcp", "remove", "xplain", "-s", "local"], 1);
    let add = [
        "mcp".to_string(),
        "add".into(),
        "xplain".into(),
        format!("http://127.0.0.1:{PORT}/mcp"),
        "--transport".into(),
        "http".into(),
        "--scope".into(),
        "local".into(),
        "--header".into(),
        format!("Authorization: Bearer {TOKEN}"),
    ];
    let add: Vec<&str> = add.iter().map(String::as_str).collect();
    calls_exact(&s, "claude", &add, 1);
    let c = s.calls("claude").into_iter().find(|c| c.args == add);
    assert_eq!(c.map(|c| c.cwd), Some(repo));
}

#[test]
fn f_integ_01_codex_argv() {
    let mut s = with_token().shim_rules("codex", vec![get(1)]).build();
    s.keys("M<Enter>");
    calls_exact(&s, "codex", &["mcp", "get", "xplain", "--json"], 1);
    s.keys("jj<Enter>y");
    calls_exact(&s, "codex", &["mcp", "remove", "xplain"], 1);
    // rest of add argv (token passing) UNSPEC-6
    let url = format!("http://127.0.0.1:{PORT}/mcp");
    calls_prefix(&s, "codex", &["mcp", "add", "xplain", "--url", &url], 1);
}

#[test]
fn f_integ_01_copilot_argv() {
    let mut s = with_token().shim_rules("copilot", vec![get(1)]).build();
    s.keys("M<Enter>");
    calls_exact(&s, "copilot", &["mcp", "get", "xplain", "--json"], 1);
    s.keys("jjjj<Enter>y");
    calls_exact(&s, "copilot", &["mcp", "remove", "xplain"], 1);
    let url = format!("http://127.0.0.1:{PORT}/mcp");
    let auth = format!("Authorization: Bearer {TOKEN}");
    calls_exact(
        &s,
        "copilot",
        &["mcp", "add", "xplain", &url, "--transport", "http", "--header", &auth, "--timeout", "200000"],
        1,
    );
}

/// xplain writes no agent config files itself (register all + copy OpenCode snippet).
#[test]
fn f_integ_01_no_config_files() {
    let mut s = base()
        .shim_rules("claude", vec![get(1)])
        .shim_rules("codex", vec![get(1)])
        .shim_rules("copilot", vec![get(1)])
        .build();
    s.keys("M<Enter>j<Enter>y");
    s.keys("j<Enter>y");
    s.keys("jc");
    s.keys("j<Enter>y");
    calls_prefix(&s, "claude", &["mcp", "add"], 1);
    calls_prefix(&s, "codex", &["mcp", "add"], 1);
    calls_prefix(&s, "copilot", &["mcp", "add"], 1);
    assert_eq!(s.clipboard_all().len(), 1);
    assert!(s.clipboard().is_some_and(|c| c.contains(r#""type": "remote""#)));
    for p in [
        "${HOME}/.claude.json",
        "${HOME}/.claude",
        "${HOME}/.codex",
        "${HOME}/.copilot",
        "${HOME}/.config/opencode",
        "${CONFIG}/opencode",
        "opencode.json",
        ".mcp.json",
    ] {
        assert!(!s.file_exists(p), "{p} must not exist");
    }
}

#[test]
fn f_integ_01_unregister_argv() {
    let mut s = base()
        .shim_rules("claude", vec![get(0)])
        .shim_rules("codex", vec![get(0)])
        .shim_rules("copilot", vec![get(0)])
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code registered", r"^ +│  Codex registered", r"^ +│  Copilot registered"]);
    s.keys("jdy");
    s.keys("jdy");
    s.keys("jjdy");
    calls_exact(&s, "claude", &["mcp", "remove", "xplain", "-s", "local"], 1);
    calls_exact(&s, "codex", &["mcp", "remove", "xplain"], 1);
    calls_exact(&s, "copilot", &["mcp", "remove", "xplain"], 1);
    for n in ["claude", "codex", "copilot"] {
        calls_prefix(&s, n, &["mcp", "add"], 0);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// F-INTEG-02: registration check

/// A later successful check (R) clears the row note.
#[test]
fn f_integ_02_check_clears_note() {
    let mut s = base()
        .shim_rules("claude", vec![get(1), Rule::args(["mcp", "add"]).exit(7).stderr("nope\n")])
        .build();
    s.keys("M<Enter>j<Enter>y");
    s.assert_matches(r"^ +│> Claude Code ");
    s.assert_contains("Claude Code register failed (exit 7): nope");
    s.keys("R");
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 3);
    s.assert_matches(r"^ +│> Claude Code not registered[^\n]*│\n( +│  [^\n]*│\n){3} +│ j/k  enter register");
    s.assert_not_contains("register failed");
}

/// exit 0 without URL -> registered; non-zero -> not registered; CLI missing -> not registered, no note.
#[test]
fn f_integ_02_exit() {
    let mut s = base()
        .shim_rules("claude", vec![get(0).stdout("xplain:\n  Scope: Local config\n  Status: Connected\n")])
        .shim_rules("codex", vec![get(1).stderr("No MCP server named 'xplain' found.\n")])
        // copilot: no shim, CLI not on PATH
        .build();
    s.keys("M");
    // before any check every CLI row says not registered
    all(
        &s,
        &[
            r"^ +│  Claude Code not registered",
            r"^ +│  Codex not registered",
            r"^ +│  Copilot not registered",
        ],
    );
    s.keys("<Enter>");
    all(
        &s,
        &[
            r"^ +│  Claude Code registered",
            r"^ +│  Codex not registered",
            r"^ +│  Copilot not registered",
            r"^ +│  Copilot [^\n]*│\n +│ j/k  enter register",
        ],
    );
    none(&s, &["not found", "No MCP server"]);
    s.keys("jjjj");
    // no note on the Copilot row either
    s.assert_matches(r"^ +│> Copilot not registered[^\n]*│\n +│ j/k  enter register");
    s.assert_not_contains("not found");
}

/// JSON url field wins over a URL elsewhere in the text.
#[test]
fn f_integ_02_json_first() {
    let mut s = base()
        .shim_rules(
            "claude",
            vec![get(0).stdout(&port(
                r#"{"doc": "http://127.0.0.1:4/mcp", "url": "http://127.0.0.1:${PORT}/mcp"}"#,
            ))],
        )
        .shim_rules(
            "codex",
            vec![get(0).stdout(&port(
                r#"{"doc": "http://127.0.0.1:${PORT}/mcp", "url": "http://127.0.0.1:5/mcp"}"#,
            ))],
        )
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code registered", r"^ +│  Codex stale"]);
}

/// JSON url equal -> registered, different -> stale.
#[test]
fn f_integ_02_json_url() {
    let mut s = base()
        .shim_rules(
            "claude",
            vec![get(0).stdout(&port("xplain:\n  Type: http\n  URL: http://127.0.0.1:${PORT}/mcp\n"))],
        )
        .shim_rules(
            "codex",
            vec![get(0).stdout(&port(r#"{"name": "xplain", "url": "http://127.0.0.1:${PORT}/mcp"}"#))],
        )
        .shim_rules("copilot", vec![get(0).stdout(r#"{"name": "xplain", "url": "http://127.0.0.1:1/mcp"}"#)])
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code registered", r"^ +│  Codex registered", r"^ +│  Copilot stale"]);
}

/// xplain.url and mcpServers.xplain.url; different -> stale.
#[test]
fn f_integ_02_nested_url() {
    let mut s = base()
        .shim_rules(
            "claude",
            vec![get(0).stdout(r#"{"xplain": {"type": "http", "url": "http://127.0.0.1:2/mcp"}}"#)],
        )
        .shim_rules(
            "codex",
            vec![
                get(0)
                    .stdout(&port(r#"{"mcpServers": {"xplain": {"url": "http://127.0.0.1:${PORT}/mcp"}}}"#)),
            ],
        )
        .shim_rules(
            "copilot",
            vec![
                get(0)
                    .stdout(&port(r#"{"mcpServers": {"xplain": {"url": "http://localhost:${PORT}/mcp"}}}"#)),
            ],
        )
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code stale", r"^ +│  Codex registered", r"^ +│  Copilot stale"]);
}

/// Check re-runs on R (status follows CLI) and on each server start.
#[test]
fn f_integ_02_recheck() {
    let mut s = base().shim_rules("copilot", vec![get(0)]).build();
    s.keys("M<Enter>");
    s.assert_matches(r"^ +│  Copilot registered");
    s.set_shim("copilot", vec![get(0).stdout(r#"{"url": "http://127.0.0.1:6/mcp"}"#)]);
    s.keys("R");
    s.assert_matches(r"^ +│  Copilot stale");
    s.set_shim("copilot", vec![get(1)]);
    s.keys("<Enter><Enter>");
    all(&s, &[r"^ +│> ● on ", r"^ +│  Copilot not registered"]);
    calls_exact(&s, "copilot", &["mcp", "get", "xplain", "--json"], 3);
}

/// First http(s) URL in text, ends at whitespace, quote or comma.
#[test]
fn f_integ_02_text_url() {
    let mut s = base()
        .shim_rules(
            "claude",
            vec![get(0).stdout(&port(
                "server xplain url='http://127.0.0.1:${PORT}/mcp', other http://127.0.0.1:3/mcp\n",
            ))],
        )
        .shim_rules(
            "codex",
            vec![get(0).stdout(&port("xplain -> \"http://127.0.0.1:${PORT}/mcp\" (http)\n"))],
        )
        .shim_rules("copilot", vec![get(0).stdout(&port("xplain https://127.0.0.1:${PORT}/mcp,enabled\n"))])
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code registered", r"^ +│  Codex registered", r"^ +│  Copilot stale"]);
}

// ---------------------------------------------------------------------------------------------------------------
// F-INTEG-03: register

/// The row shows the busy marker "… " while the register CLI runs, gone when it is done.
#[test]
fn f_integ_03_busy() {
    let mut s = base().shim_rules("claude", vec![get(1), Rule::args(["mcp", "add"]).exit(0).block()]).build();
    s.keys("M<Enter>j<Enter>");
    s.assert_matches(r"^ +│> Claude Code not registered");
    s.assert_not_contains("… ");
    // fake claude blocks in `mcp add`; the row is seen mid-run
    s.keys("y");
    assert_eq!(s.blocked_calls("claude"), 1);
    s.assert_matches(r"^ +│> Claude Code … not registered");
    s.release("claude");
    calls_prefix(&s, "claude", &["mcp", "add"], 1);
    s.assert_matches(r"^ +│> Claude Code not registered");
    s.assert_contains("Registered. Restart or resume the session");
    s.assert_not_contains("… ");
}

/// Note hint + watch prompt advice; check re-runs, note kept.
#[test]
fn f_integ_03_claude_ok() {
    let mut s = base().shim_rules("claude", vec![get(1)]).build();
    s.keys("M<Enter>j");
    s.assert_matches(r"^ +│> Claude Code not registered");
    s.assert_not_contains("Registered.");
    s.keys("<Enter>");
    // fake claude reports the registration from now on
    s.set_shim("claude", vec![get(0)]);
    s.keys("y");
    calls_prefix(&s, "claude", &["mcp", "add"], 1);
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 2);
    all(
        &s,
        &[
            r"^ +│> Claude Code registered",
            r"Registered\. Restart or resume the session \(claude --resume\),[\s│]+then[\s│]+paste[\s│]+the[\s│]+watch[\s│]+prompt\.;[\s│]+restart",
        ],
    );
    // note wraps over 2 modal rows (max 2, rest cut); regex allows row breaks between words
}

#[test]
fn f_integ_03_copilot_ok() {
    let mut s = base().shim_rules("copilot", vec![get(1)]).build();
    s.keys("M<Enter>jjjj<Enter>y");
    calls_prefix(&s, "copilot", &["mcp", "add"], 1);
    s.assert_matches(
        r"Registered\.[\s│]+Restart[\s│]+copilot[\s│]+\(or[\s│]+use[\s│]+/mcp\),[\s│]+then[\s│]+paste[\s│]+the[\s│]+watch[\s│]+prompt\.;[\s│]+restart",
    );
    // note wrap width not in spec: words may break across the 2 note rows
}

/// Register non-zero exit, no output: no ": " suffix.
#[test]
fn f_integ_03_exit_empty() {
    let mut s = base().shim_rules("claude", vec![get(1), Rule::args(["mcp", "add"]).exit(5)]).build();
    s.keys("M<Enter>j<Enter>y");
    s.assert_matches(r"^ +│ Claude Code register failed \(exit 5\) +│$");
}

/// "(exit <code>): " + first 3 stderr lines joined.
#[test]
fn f_integ_03_exit_stderr() {
    let mut s = base()
        .shim_rules(
            "claude",
            vec![
                get(1),
                Rule::args(["mcp", "add"])
                    .exit(3)
                    .stdout("ignored stdout\n")
                    .stderr("err one\nerr two\nerr three\nerr four\n"),
            ],
        )
        .build();
    s.keys("M<Enter>j<Enter>y");
    s.assert_matches(
        r"^ +│ Claude Code register failed \(exit 3\):[\s│]+err[\s│]+one[\s│]+err[\s│]+two[\s│]+err[\s│]+three +│$",
    );
    none(&s, &["err four", "ignored stdout", "Registered."]);
    s.assert_text_cell("Claude Code register failed", 0, C::new().fg("#cb4b16"));
}

/// Register non-zero exit, empty stderr: stdout lines used.
#[test]
fn f_integ_03_exit_stdout() {
    let mut s = base()
        .shim_rules("copilot", vec![get(1), Rule::args(["mcp", "add"]).exit(2).stdout("out one\nout two\n")])
        .build();
    s.keys("M<Enter>jjjj<Enter>y");
    s.assert_matches(r"^ +│ Copilot register failed \(exit 2\): out one out two +│$");
    // spec 'first 3 lines ..., trimmed, joined by space' is ambiguous about per-line trimming; not asserted
}

/// Token masked as *** in register error message.
#[test]
fn f_integ_03_mask() {
    let mut s = with_token()
        .shim_rules(
            "claude",
            vec![get(1), Rule::args(["mcp", "add"]).exit(1).stderr(&format!("bad header Bearer {TOKEN}\n"))],
        )
        .build();
    s.keys("M<Enter>j<Enter>y");
    let want = format!("Authorization: Bearer {TOKEN}");
    let n = s.calls("claude").iter().filter(|c| c.args.contains(&want)).count();
    assert_eq!(n, 1, "{:?}", s.calls("claude"));
    s.assert_matches(
        r"^ +│ Claude Code register failed \(exit 1\):[\s│]+bad[\s│]+header[\s│]+Bearer[\s│]+\*\*\* +│$",
    );
    s.assert_not_contains(TOKEN);
}

/// Register with CLI missing: "<label> CLI not found".
#[test]
fn f_integ_03_not_found() {
    let mut s = base().build();
    s.keys("M<Enter>jj");
    s.assert_matches(r"^ +│> Codex not registered");
    s.assert_not_contains("CLI not found");
    s.keys("<Enter>y");
    s.assert_matches(r"^ +│> Codex not registered[^\n]*│\n( +│[^\n]*│\n)*? +│ Codex CLI not found +│$");
}

/// Register result note shown on that row when selected; kept after re-check.
#[test]
fn f_integ_03_note_row() {
    let mut s = base()
        .shim_rules("claude", vec![get(1), Rule::args(["mcp", "add"]).exit(7).stderr("nope\n")])
        .build();
    s.keys("M<Enter>j<Enter>y");
    s.assert_contains("Claude Code register failed (exit 7): nope");
    s.keys("j");
    s.assert_matches(r"^ +│> Codex ");
    s.assert_not_contains("register failed");
    s.keys("k");
    s.assert_matches(r"^ +│> Claude Code ");
    s.assert_contains("Claude Code register failed (exit 7): nope");
    // note survived the check that re-ran right after the register
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 2);
    s.keys("k");
    s.assert_matches(r"^ +│> ● on");
    s.assert_not_contains("register failed");
    // busy marker "… " during the CLI run: f_integ_03_busy
    // 20 s timeout path ("<label> register failed: <message>") not covered: timing based
}

/// Register runs remove first, its failure ignored, then add.
#[test]
fn f_integ_03_remove_ignored() {
    let mut s = base()
        .shim_rules(
            "claude",
            vec![
                get(1),
                Rule::args(["mcp", "remove"]).exit(1).stderr("No MCP server found with name: xplain\n"),
            ],
        )
        .build();
    s.keys("M<Enter>j<Enter>y");
    calls_prefix(&s, "claude", &["mcp", "remove"], 1);
    calls_prefix(&s, "claude", &["mcp", "add"], 1);
    s.assert_contains("Registered. Restart or resume the session");
    none(&s, &["failed", "No MCP server found"]);
    // remove runs before add, and the check re-runs after them; exactly these 4 calls
    let calls = s.calls("claude");
    assert_eq!(calls.len(), 4, "{calls:?}");
    let seq: [&[&str]; 4] = [&["mcp", "get"], &["mcp", "remove"], &["mcp", "add"], &["mcp", "get"]];
    for (c, p) in calls.iter().zip(seq) {
        assert!(c.starts_with(p), "sequence: {calls:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// F-INTEG-04: unregister

/// Non-zero exit: "<label> unregister failed (exit <code>): <output>", token masked.
#[test]
fn f_integ_04_fail() {
    let mut s = with_token()
        .shim_rules(
            "claude",
            vec![get(0), Rule::args(["mcp", "remove"]).exit(2).stderr(&format!("boom {TOKEN}\n"))],
        )
        .build();
    s.keys("M<Enter>jdy");
    calls_exact(&s, "claude", &["mcp", "remove", "xplain", "-s", "local"], 1);
    s.assert_matches(r"^ +│ Claude Code unregister failed \(exit 2\):[\s│]+boom[\s│]+\*\*\* +│$");
    none(&s, &[TOKEN, "Removed xplain"]);
}

/// Unregister works with MCP off when row known registered.
#[test]
fn f_integ_04_mcp_off() {
    let mut s = base().shim_rules("codex", vec![get(0)]).build();
    s.keys("M<Enter><Enter>");
    all(&s, &[r"^ +│> ○ off", r"^ +│  Codex registered"]);
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("jjdy");
    calls_exact(&s, "codex", &["mcp", "remove", "xplain"], 1);
    s.assert_matches(r"^ +│ Removed xplain from Codex +│$");
    // row status afterwards: UNSPEC-22
}

/// Unregister with CLI gone: "<label> CLI not found".
#[test]
fn f_integ_04_not_found() {
    let mut s = base().shim_rules("codex", vec![get(0)]).build();
    s.keys("M<Enter>jj");
    s.assert_matches(r"^ +│> Codex registered");
    s.remove_shim("codex");
    s.keys("dy");
    s.assert_matches(r"^ +│ Codex CLI not found +│$");
    s.assert_not_contains("Removed xplain");
}

/// Unregister ok: remove run once, note "Removed xplain from <label>".
#[test]
fn f_integ_04_ok() {
    let mut s = base().shim_rules("copilot", vec![get(0)]).build();
    s.keys("M<Enter>jjjj");
    s.assert_matches(r"^ +│> Copilot registered");
    s.assert_not_contains("Removed xplain");
    s.keys("d");
    calls_prefix(&s, "copilot", &["mcp", "remove"], 0);
    s.keys("y");
    calls_exact(&s, "copilot", &["mcp", "remove", "xplain"], 1);
    calls_prefix(&s, "copilot", &["mcp", "add"], 0);
    s.assert_matches(r"^ +│ Removed xplain from Copilot +│$");
    s.assert_text_cell("Removed xplain from Copilot", 0, C::new().fg("#cb4b16"));
}

// ---------------------------------------------------------------------------------------------------------------
// F-INTEG-05: copy register command

#[test]
fn f_integ_05_claude() {
    let mut s = with_token().build();
    s.keys("M<Enter>jc");
    assert_eq!(s.clipboard_all().len(), 1);
    let want = format!(
        "claude mcp add xplain http://127.0.0.1:{PORT}/mcp --transport http --scope local --header \"Authorization: Bearer {TOKEN}\"\n\
         \n\
         Then restart/resume the session: claude --resume\n\
         Allow the tools: claude --allowedTools \"mcp__xplain\"  (or add permission rule mcp__xplain)\n\
         Optional: set env CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS=0 to avoid auto-backgrounding long calls."
    );
    assert_eq!(s.clipboard(), Some(want.as_str()));
    s.assert_contains("register command (copied):");
    s.assert_contains("claude mcp add xplain");
    s.assert_not_contains(TOKEN);
}

/// Text UNSPEC-6; token not shown.
#[test]
fn f_integ_05_codex() {
    let mut s = with_token().build();
    s.keys("M<Enter>jj");
    assert_eq!(s.clipboard_all().len(), 0);
    s.keys("c");
    assert_eq!(s.clipboard_all().len(), 1);
    s.assert_contains("register command (copied):");
    s.assert_not_contains(TOKEN);
}

#[test]
fn f_integ_05_copilot() {
    let mut s = with_token().build();
    s.keys("M<Enter>jjjjc");
    assert_eq!(s.clipboard_all().len(), 1);
    let want = format!(
        "copilot mcp add xplain http://127.0.0.1:{PORT}/mcp --transport http --header \"Authorization: Bearer {TOKEN}\" --timeout 200000\n\
         \n\
         Then restart copilot (or use /mcp).\n\
         Allow the tools: copilot --allow-tool='xplain'  (or approve once when asked).\n\
         Session-only alternative: copilot --additional-mcp-config @file --allow-tool='xplain'"
    );
    assert_eq!(s.clipboard(), Some(want.as_str()));
    s.assert_contains("register command (copied):");
    s.assert_contains("Bearer ***");
    s.assert_not_contains(TOKEN);
}

#[test]
fn f_integ_05_opencode() {
    let mut s = with_token().build();
    s.keys("M<Enter>jjjc");
    assert_eq!(s.clipboard_all().len(), 1);
    let want = format!(
        "Add to opencode.json (project) or ~/.config/opencode/opencode.json:\n\
         \n\
         \"mcp\": {{\n\
         \x20 \"xplain\": {{\n\
         \x20   \"type\": \"remote\",\n\
         \x20   \"url\": \"http://127.0.0.1:{PORT}/mcp\",\n\
         \x20   \"enabled\": true,\n\
         \x20   \"oauth\": false,\n\
         \x20   \"timeout\": 120000,\n\
         \x20   \"headers\": {{\"Authorization\": \"Bearer {TOKEN}\"}}\n\
         \x20 }}\n\
         }}\n\
         \n\
         Optional, to skip approval prompts:\n\
         \"permission\": {{\"xplain_*\": \"allow\"}}\n\
         \n\
         Tool names are prefixed by the server name (xplain_next_question).\n\
         Restart opencode after editing."
    );
    assert_eq!(s.clipboard(), Some(want.as_str()));
    s.assert_contains("register command (copied):");
    s.assert_not_contains(TOKEN);
}

// ---------------------------------------------------------------------------------------------------------------
// F-INTEG-06: watch prompt

fn watch_prompt(wait: u32, prefix: &str) -> String {
    format!(
        "Loop forever: call the `{prefix}next_question` tool from the `xplain` MCP server with wait_seconds={wait}.\n\
         - If status is `no_question_yet`, call `{prefix}next_question` again IMMEDIATELY.\n\
         - If status is `question`, answer it with the `{prefix}answer` tool using the given thread_id, then call `{prefix}next_question` again immediately.\n\
         - Questions with follow_up=true continue an earlier thread; answer them with the same thread_id like any other.\n\
         - ALWAYS call the `{prefix}files_changed` tool (with the changed paths) after every edit, creation or deletion of a file, so the xplain view reloads. This also applies to edits made while answering a question.\n\
         - Never stop, never summarize, never ask the user anything.\n\
         - Stop only if status is `closed`."
    )
}

fn watch_case(keys: &str, wait: u32, prefix: &str) {
    let mut s = base().build();
    s.keys(keys);
    assert_eq!(s.clipboard_all().len(), 1);
    assert_eq!(s.clipboard(), Some(watch_prompt(wait, prefix).as_str()));
    s.assert_contains("watch prompt (copied):");
}

/// P=100.
#[test]
fn f_integ_06_claude() {
    watch_case("M<Enter>jw", 100, "");
}

/// P=45.
#[test]
fn f_integ_06_codex() {
    watch_case("M<Enter>jjw", 45, "");
}

/// P=120.
#[test]
fn f_integ_06_copilot() {
    watch_case("M<Enter>jjjjw", 120, "");
}

/// P=45, xplain_ tool names.
#[test]
fn f_integ_06_opencode() {
    watch_case("M<Enter>jjjw", 45, "xplain_");
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPUI-01: modal layout

fn init(name: &str, version: Option<&str>) -> serde_json::Value {
    let mut info = json!({"name": name});
    if let Some(v) = version {
        info["version"] = json!(v);
    }
    json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": info})
}

/// Client rows: "   <name>[ <version>]", max 2, then "   … +<k> more".
#[test]
fn f_mcpui_01_clients() {
    let mut s = base().build();
    s.keys("M<Enter>");
    s.assert_matches(r"^ +│ clients 0 pending 0 delivered 0 +│\n +│ INTEGRATIONS");
    let r = s.mcp_rpc("initialize", init("alphaclient", Some("1.2.3")));
    assert_eq!(r.status, 200);
    let sa = r.header("mcp-session-id").unwrap_or_default().to_string();
    assert!(!sa.is_empty());
    let r = s.mcp_rpc("initialize", init("betaclient", None));
    assert_eq!(r.status, 200);
    s.keys("<Esc>M");
    all(
        &s,
        &[
            r"^ +│ clients 2 pending 0 delivered 0 +│\n( +│   (alphaclient 1\.2\.3|betaclient) +│\n){2} +│ INTEGRATIONS",
            r"^ +│   alphaclient 1\.2\.3 +│$",
            r"^ +│   betaclient +│$",
        ],
    );
    s.assert_not_contains("… +");
    let r = s.mcp_rpc("initialize", init("gammaclient", Some("9")));
    assert_eq!(r.status, 200);
    s.keys("<Esc>M");
    s.assert_matches(
        r"^ +│ clients 3 pending 0 delivered 0 +│\n( +│   (alphaclient 1\.2\.3|betaclient|gammaclient 9) +│\n){2} +│   … \+1 more +│\n +│ INTEGRATIONS",
    );
    // a waiting long poll marks its client row with ' ⟳'
    let _poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 120})).session(&sa));
    s.keys("<Esc>M");
    all(
        &s,
        &[
            r"^ +│ clients 3 pending 0 delivered 0 +│\n( +│   (alphaclient 1\.2\.3 ⟳|betaclient|gammaclient 9) +│\n){2} +│   … \+1 more +│\n +│ INTEGRATIONS",
            r"^ +│   alphaclient 1\.2\.3 ⟳ +│$",
        ],
    );
}

/// Remove confirm rows; note hidden during confirm.
#[test]
fn f_mcpui_01_confirm_remove() {
    let mut s = base().shim_rules("claude", vec![get(0)]).build();
    s.keys("M<Enter>j<Enter>");
    s.assert_matches(r"^ +│> Claude Code registered");
    s.assert_contains("already registered (d to remove)");
    s.keys("d");
    s.assert_matches(
        r"^ +│ Remove Claude Code registration\? \(y/n\) +│\n +│ removes the xplain MCP server from Claude Code config +│\n +│ y confirm  n/esc cancel +│$",
    );
    none(&s, &["already registered", "enter register"]);
    s.assert_text_cell("Remove Claude Code", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("removes the xplain", 0, C::new().fg("#586e75"));
}

/// Register confirm rows (accent + dim), confirm hint, preview hidden.
#[test]
fn f_mcpui_01_confirm() {
    let mut s = base().build();
    s.keys("M<Enter>j");
    s.assert_matches(r"^ +│> Claude Code not registered");
    s.keys("<Enter>");
    s.assert_matches(
        r"^ +│ Register Claude Code MCP server\? \(y/n\) +│\n +│ adds the xplain MCP server to Claude Code config +│\n +│ y confirm  n/esc cancel +│$",
    );
    s.assert_not_contains("enter register");
    s.assert_text_cell("Register Claude Code", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("adds the xplain", 0, C::new().fg("#586e75"));
    s.assert_text_cell("y confirm", 0, C::new().fg("#586e75"));
    s.keys("n");
    s.assert_matches(r"^ +│ j/k  enter register  d remove  c/w copy  R refresh  esc +│$");
    none(&s, &["Register Claude Code", "y confirm"]);
}

/// Failed start: error row (dels color) under power row.
#[test]
fn f_mcpui_01_error_row() {
    let mut s = base().env("XPLAIN_MCP_PORT", "abc").build();
    s.keys("M");
    s.assert_not_contains("invalid XPLAIN_MCP_PORT");
    s.keys("<Enter>");
    s.assert_matches(
        r#"^ +│> ○ off +│\n +│ invalid XPLAIN_MCP_PORT "abc" \(0-65535\) +│\n +│ clients 0 pending 0 delivered 0 +│$"#,
    );
    s.assert_text_cell("invalid XPLAIN_MCP_PORT", 0, C::new().fg("#dc322f"));
    s.assert_row_contains(0, "[mcp: off]");
}

/// M opens MCP modal (server off): width 64, rows in order, power row selected, styles.
#[test]
fn f_mcpui_01_layout_off() {
    let mut s = base().build();
    none(&s, &["INTEGRATIONS", " MCP "]);
    s.keys("M");
    all(
        &s,
        &[
            r"^ +╭─{62}╮$",
            r"^ +│ MCP +│\n +│> ○ off +│\n +│ clients 0 pending 0 delivered 0 +│\n +│ INTEGRATIONS +│\n +│  Claude Code not registered.*│\n +│  Codex not registered.*│\n +│  OpenCode copy-paste only +│\n +│  Copilot not registered.*│\n +│ j/k  enter register  d remove  c/w copy  R refresh  esc +│\n +╰─{62}╯$",
        ],
    );
    none(&s, &["127.0.0.1", "(y/n)", "copied"]);
    // title bold
    s.assert_text_cell("MCP", 0, C::new().bold(true));
    // power row selected (solarized selBg/selFg)
    s.assert_text_cell("○ off", 0, C::new().bg("#073642").fg("#93a1a1"));
    // unselected integration row not selBg
    s.assert_text_cell("OpenCode copy-paste only", 0, C::new().bg("#002b36"));
    s.assert_text_cell("INTEGRATIONS", 0, C::new().fg("#cb4b16"));
    s.assert_text_cell("clients 0 pending", 0, C::new().fg("#586e75"));
    s.assert_text_cell("j/k  enter register", 0, C::new().fg("#586e75"));
}

/// Power row on: "● on  127.0.0.1:<port>".
#[test]
fn f_mcpui_01_layout_on() {
    let mut s = base().build();
    s.keys("M");
    s.assert_matches(r"^ +│> ○ off +│$");
    s.assert_not_contains("● on");
    s.keys("<Enter>");
    s.assert_matches(&port(
        r"^ +│ MCP +│\n +│> ● on  127\.0\.0\.1:${PORT} +│\n +│ clients 0 pending 0 delivered 0 +│\n +│ INTEGRATIONS +│$",
    ));
    s.assert_not_contains("○ off");
}

/// Box width min(cols, 64): 60 cols gives full-width box.
#[test]
fn f_mcpui_01_narrow() {
    let mut s = base().size(60, 30).build();
    s.keys("M");
    all(
        &s,
        &[
            r"^╭─{58}╮$",
            r"^│ MCP +│$",
            r"^│> ○ off +│$",
            r"^│ j/k  enter register  d remove  c/w copy  R refresh  esc +│$",
            r"^╰─{58}╯$",
        ],
    );
}

/// Long note: accent, at most 2 wrapped lines.
#[test]
fn f_mcpui_01_note_wrap() {
    let mut s = base().shim_rules("claude", vec![get(1)]).build();
    s.keys("M<Enter>j<Enter>y");
    all(
        &s,
        &[
            r"^ +│ Registered\. Restart or resume the session",
            r"^ +│  Copilot [^\n]*│\n( +│ [^\n]*│\n){1,2} +│ j/k  enter register",
        ],
    );
    s.assert_text_cell("Registered. Restart", 0, C::new().fg("#cb4b16"));
}

/// XPLAIN_MCP_PORT 65535 (max) is valid: start binds it or reports the port busy, never the invalid-port error.
#[test]
fn f_mcpui_01_port_max() {
    let mut s = base().env("XPLAIN_MCP_PORT", "65535").build();
    s.keys("M<Enter>");
    // both outcomes use port 65535
    s.assert_matches(
        r"^ +│> (● on  127\.0\.0\.1:65535 +│|○ off +│\n +│ MCP port 65535 is already in use on 127\.0\.0\.1\. )",
    );
    s.assert_not_contains("invalid XPLAIN_MCP_PORT");
}

/// XPLAIN_MCP_PORT 65536 (above max) is invalid: start fails with the invalid-port error row.
#[test]
fn f_mcpui_01_port_over_max() {
    let mut s = base().env("XPLAIN_MCP_PORT", "65536").build();
    s.keys("M<Enter>");
    s.assert_matches(
        r#"^ +│> ○ off +│\n +│ invalid XPLAIN_MCP_PORT "65536" \(0-65535\) +│\n +│ clients 0 pending 0 delivered 0 +│$"#,
    );
    s.assert_row_contains(0, "[mcp: off]");
}

/// M inside a text input is typed, no modal.
#[test]
fn f_mcpui_01_pre() {
    let mut s = base().build();
    s.keys("a");
    s.assert_contains("enter send");
    s.keys("M");
    s.assert_matches(r"│ M +│");
    s.assert_not_contains("INTEGRATIONS");
    s.keys("<Esc>M");
    s.assert_contains("INTEGRATIONS");
}

/// Preview: dim label, max 3 wrapped lines (width-4), cut with …; blank 3rd line gives " …".
#[test]
fn f_mcpui_01_preview() {
    let mut s = base().build();
    s.keys("M<Enter>jjj");
    s.assert_matches(r"^ +│> OpenCode copy-paste only");
    s.assert_not_contains("copied");
    s.keys("c");
    // first line (67 chars) wraps at 60 at last space, 3rd line blank
    s.assert_matches(
        r"^ +│> OpenCode copy-paste only +│\n +│  Copilot [^\n]*│\n +│ register command \(copied\): +│\n +│ Add to opencode\.json \(project\) or +│\n +│ ~/\.config/opencode/opencode\.json: +│\n +│ … +│\n +│ j/k  enter register",
    );
    s.assert_text_cell("register command (copied):", 0, C::new().fg("#586e75"));
    s.keys("kk");
    s.assert_matches(r"^ +│> Claude Code");
    s.assert_not_contains("copied");
    s.keys("w");
    s.assert_matches(
        r"^ +│ watch prompt \(copied\): +│\n +│ Loop forever: call the `next_question` tool from the +│\n +│ `xplain` MCP server with wait_seconds=100\. +│\n +│ - If status is `no_question_yet`, call `next_question` ag[a-z]*… +│\n +│ j/k  enter register",
    );
    s.assert_text_cell("watch prompt (copied):", 0, C::new().fg("#586e75"));
}

/// Reopen: power row selected again, no note, no preview.
#[test]
fn f_mcpui_01_reopen() {
    let mut s = base().build();
    s.keys("MR");
    s.assert_contains("start MCP first");
    s.keys("<Esc>M");
    s.assert_matches(r"^ +│> ○ off");
    s.assert_not_contains("start MCP first");
    s.keys("<Enter>jc");
    s.assert_matches(r"^ +│> Claude Code ");
    s.assert_contains("register command (copied):");
    s.keys("<Esc>M");
    all(&s, &[r"^ +│> ● on ", r"^ +│  Claude Code ", r"^ +│  Copilot [^\n]*│\n +│ j/k  enter register"]);
    s.assert_not_contains("copied");
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPUI-02: keys

/// Esc, q, M close the MCP modal (q does not quit).
#[test]
fn f_mcpui_02_close() {
    let mut s = base().build();
    s.keys("M");
    s.assert_contains("INTEGRATIONS");
    s.keys("<Esc>");
    none(&s, &["INTEGRATIONS", "Quit xplain"]);
    s.keys("M");
    s.assert_contains("INTEGRATIONS");
    s.keys("q");
    none(&s, &["INTEGRATIONS", "Quit xplain"]);
    s.keys("M");
    s.assert_contains("INTEGRATIONS");
    s.keys("M");
    none(&s, &["INTEGRATIONS", "Quit xplain"]);
    // app still running and browsable after closes
    s.assert_row_contains(0, "[mcp: off]");
}

/// Confirm pending: n/Esc cancel, y/Enter run, other keys ignored.
#[test]
fn f_mcpui_02_confirm_keys() {
    let mut s = base().shim_rules("claude", vec![get(1)]).build();
    s.keys("M<Enter>j<Enter>");
    s.assert_contains("Register Claude Code MCP server? (y/n)");
    // j, q, M, d, c ignored while confirm pending
    s.keys("jqMdc");
    s.assert_contains("Register Claude Code MCP server? (y/n)");
    s.assert_matches(r"^ +│> Claude Code ");
    assert_eq!(s.clipboard_all().len(), 0);
    s.keys("n");
    s.assert_not_contains("(y/n)");
    s.assert_matches(r"^ +│> Claude Code ");
    s.keys("<Enter>");
    s.assert_contains("(y/n)");
    s.keys("<Esc>");
    s.assert_not_contains("(y/n)");
    s.assert_contains("INTEGRATIONS");
    calls_prefix(&s, "claude", &["mcp", "add"], 0);
    s.keys("<Enter>");
    s.assert_contains("(y/n)");
    s.keys("y");
    s.assert_not_contains("(y/n)");
    s.assert_contains("Registered. Restart or resume the session");
    calls_prefix(&s, "claude", &["mcp", "add"], 1);
    s.keys("<Enter>");
    s.assert_contains("(y/n)");
    s.keys("<Enter>");
    s.assert_not_contains("(y/n)");
    calls_prefix(&s, "claude", &["mcp", "add"], 2);
}

/// c/w with MCP off: note start MCP first, nothing copied.
#[test]
fn f_mcpui_02_copy_off() {
    let mut s = base().build();
    s.keys("Mj");
    s.keys("c");
    s.assert_contains("start MCP first");
    s.assert_not_contains("copied");
    assert_eq!(s.clipboard_all().len(), 0);
    s.keys("jjw");
    s.assert_contains("start MCP first");
    s.assert_not_contains("copied");
    assert_eq!(s.clipboard_all().len(), 0);
}

/// c/w with MCP on: OSC 52 full text with token; preview masks token.
#[test]
fn f_mcpui_02_copy_on() {
    let mut s = with_token().build();
    s.keys("M<Enter>j");
    assert_eq!(s.clipboard_all().len(), 0);
    s.keys("c");
    assert_eq!(s.clipboard_all().len(), 1);
    let c = s.clipboard().unwrap_or_default();
    assert!(c.contains(&format!("claude mcp add xplain http://127.0.0.1:{PORT}/mcp")), "{c}");
    assert!(c.contains(&format!("Bearer {TOKEN}")), "{c}");
    s.assert_contains("register command (copied):");
    s.assert_contains("Bearer ***");
    s.assert_not_contains(TOKEN);
    s.keys("w");
    assert_eq!(s.clipboard_all().len(), 2);
    assert!(s.clipboard().unwrap_or_default().contains("wait_seconds=100"));
    s.assert_contains("watch prompt (copied):");
    s.assert_not_contains("register command (copied)");
}

/// d works with MCP off when row known registered.
#[test]
fn f_mcpui_02_d_mcp_off() {
    let mut s = base().shim_rules("claude", vec![get(0)]).build();
    s.keys("M<Enter>");
    s.assert_matches(r"^ +│  Claude Code registered");
    s.keys("<Enter>");
    s.assert_matches(r"^ +│> ○ off");
    s.keys("jd");
    s.assert_contains("Remove Claude Code registration? (y/n)");
    s.keys("y");
    calls_exact(&s, "claude", &["mcp", "remove", "xplain", "-s", "local"], 1);
    s.assert_contains("Removed xplain from Claude Code");
}

/// d: OpenCode copy-paste note; not registered note; registered -> remove confirm.
#[test]
fn f_mcpui_02_d() {
    let mut s = base().shim_rules("claude", vec![get(0)]).shim_rules("codex", vec![get(1)]).build();
    s.keys("M<Enter>jj");
    s.assert_matches(r"^ +│> Codex not registered");
    s.keys("d");
    s.assert_contains("not registered");
    s.assert_matches(r"^ +│ not registered +│$");
    s.assert_not_contains("(y/n)");
    s.keys("jd");
    s.assert_contains("copy-paste only: c copies the snippet");
    s.assert_not_contains("(y/n)");
    s.keys("kkd");
    s.assert_contains("Remove Claude Code registration? (y/n)");
    s.keys("n");
    calls_prefix(&s, "claude", &["mcp", "remove"], 0);
}

/// Integration Enter clears preview.
#[test]
fn f_mcpui_02_enter_clears_preview() {
    let mut s = base().shim_rules("claude", vec![get(0)]).build();
    s.keys("M<Enter>jc");
    s.assert_contains("register command (copied):");
    s.keys("<Enter>");
    s.assert_contains("already registered (d to remove)");
    s.assert_not_contains("register command (copied)");
}

/// Integration Enter with MCP off: start MCP first; OpenCode: copy-paste note; no CLI run.
#[test]
fn f_mcpui_02_enter_off() {
    let mut s = base().shim("claude").shim("codex").shim("copilot").build();
    s.keys("Mj<Enter>");
    s.assert_contains("start MCP first");
    s.assert_not_contains("(y/n)");
    s.keys("jj<Enter>");
    s.assert_contains("copy-paste only: c copies the snippet");
    s.assert_not_contains("(y/n)");
    assert_eq!(s.calls("claude").len(), 0);
}

/// Integration Enter with MCP on: registered -> already registered; not registered / stale -> confirm.
#[test]
fn f_mcpui_02_enter_on() {
    let mut s = base()
        .shim_rules("claude", vec![get(0)])
        .shim_rules("codex", vec![get(1)])
        .shim_rules("copilot", vec![get(0).stdout(r#"{"url": "http://127.0.0.1:1/mcp"}"#)])
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code registered", r"^ +│  Codex not registered", r"^ +│  Copilot stale"]);
    s.keys("j<Enter>");
    s.assert_contains("already registered (d to remove)");
    s.assert_not_contains("(y/n)");
    s.keys("j<Enter>");
    s.assert_contains("Register Codex MCP server? (y/n)");
    s.keys("n");
    s.assert_not_contains("(y/n)");
    s.keys("jj<Enter>");
    s.assert_matches(r"^ +│> Copilot stale");
    s.assert_contains("Register Copilot MCP server? (y/n)");
    s.keys("n");
    calls_prefix(&s, "copilot", &["mcp", "add"], 0);
}

/// j/k clear preview and note.
#[test]
fn f_mcpui_02_move_clears() {
    let mut s = base().build();
    s.keys("M<Enter>j");
    s.keys("c");
    s.assert_contains("register command (copied):");
    s.keys("j");
    s.assert_matches(r"^ +│> Codex ");
    s.assert_not_contains("register command (copied)");
    s.keys("j<Enter>");
    s.assert_contains("copy-paste only: c copies the snippet");
    s.keys("k");
    s.assert_matches(r"^ +│> Codex ");
    s.assert_not_contains("copy-paste only: c copies");
}

/// j/Down k/Up move over power + 4 integration rows, clamped.
#[test]
fn f_mcpui_02_move() {
    let mut s = base().build();
    s.keys("M");
    s.assert_matches(r"^ +│> ○ off");
    s.keys("k");
    s.assert_matches(r"^ +│> ○ off");
    s.assert_not_contains("> Claude");
    s.keys("j");
    all(&s, &[r"^ +│  ○ off", r"^ +│> Claude Code "]);
    s.assert_text_cell("Claude Code", 0, C::new().bg("#073642"));
    s.assert_text_cell("○ off", 0, C::new().bg("#002b36"));
    s.keys("<Down>");
    all(&s, &[r"^ +│  Claude Code ", r"^ +│> Codex "]);
    s.keys("jj");
    s.assert_matches(r"^ +│> Copilot ");
    s.keys("jj<Down>");
    all(&s, &[r"^ +│> Copilot ", r"^ +│  OpenCode "]);
    s.keys("<Up>");
    all(&s, &[r"^ +│  Copilot ", r"^ +│> OpenCode "]);
    s.keys("kkkkkk");
    all(&s, &[r"^ +│> ○ off", r"^ +│  Claude Code "]);
}

/// Space on integration row, d/c/w on power row: nothing.
#[test]
fn f_mcpui_02_noop() {
    let mut s = base().shim_rules("claude", vec![get(0)]).build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│> ● on ", r"^ +│  Claude Code registered"]);
    s.keys("dcw");
    all(&s, &[r"^ +│> ● on ", r"^ +│  Copilot [^\n]*│\n +│ j/k  enter register"]);
    none(&s, &["(y/n)", "copied", "start MCP first"]);
    assert_eq!(s.clipboard_all().len(), 0);
    s.keys("j<Space>");
    all(&s, &[r"^ +│> Claude Code registered", r"^ +│  Copilot [^\n]*│\n +│ j/k  enter register"]);
    none(&s, &["(y/n)", "already registered"]);
    s.keys("<Esc>");
    s.assert_row_contains(0, "[mcp: on]");
}

/// Power row Enter/Space toggle server; clears note.
#[test]
fn f_mcpui_02_power() {
    let mut s = base().build();
    s.keys("M");
    s.assert_matches(r"^ +│> ○ off");
    s.keys("R");
    s.assert_contains("start MCP first");
    s.keys("<Enter>");
    s.assert_matches(&port(r"^ +│> ● on  127\.0\.0\.1:${PORT}"));
    s.assert_not_contains("start MCP first");
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("<Space>");
    s.assert_matches(r"^ +│> ○ off");
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("<Space>");
    s.assert_matches(r"^ +│> ● on ");
    s.assert_row_contains(0, "[mcp: on]");
    s.keys("<Enter>");
    s.assert_matches(r"^ +│> ○ off");
    s.assert_row_contains(0, "[mcp: off]");
}

/// R: MCP off -> start MCP first; on -> re-checks registrations.
#[test]
fn f_mcpui_02_refresh() {
    let mut s = base()
        .shim_rules("claude", vec![get(1)])
        .shim_rules("codex", vec![get(1)])
        .shim_rules("copilot", vec![get(1)])
        .build();
    s.keys("M");
    s.keys("R");
    s.assert_contains("start MCP first");
    assert_eq!(s.calls("claude").len(), 0);
    s.keys("<Enter>");
    s.assert_matches(r"^ +│  Claude Code not registered");
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 1);
    // fake claude now reports a registration
    s.set_shim("claude", vec![get(0)]);
    s.keys("R");
    s.assert_matches(r"^ +│  Claude Code registered");
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 2);
    calls_exact(&s, "codex", &["mcp", "get", "xplain", "--json"], 2);
    calls_exact(&s, "copilot", &["mcp", "get", "xplain", "--json"], 2);
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPUI-03: server lifecycle

/// After stop + start: delivered back to 0.
#[test]
fn f_mcpui_03_restart_counters() {
    let mut s = base().build();
    s.keys("M<Enter><Esc>a");
    s.keys("q1 text<Enter>");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    assert_eq!(r.tool_result()["status"], "question");
    s.keys("M");
    s.assert_matches(r"^ +│ clients \d+ pending 0 delivered 1 +│$");
    s.keys("<Enter>");
    s.assert_matches(r"^ +│> ○ off");
    s.keys("<Enter>");
    all(&s, &[r"^ +│> ● on ", r"^ +│ clients 0 pending 0 delivered 0 +│$"]);
}

/// Start fail: error row message and color; header stays off.
#[test]
fn f_mcpui_03_start_fail_color() {
    let mut s = base().env("XPLAIN_MCP_PORT", "99999").build();
    s.keys("M<Enter>");
    s.assert_matches(r#"^ +│> ○ off +│\n +│ invalid XPLAIN_MCP_PORT "99999" \(0-65535\) +│$"#);
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_text_cell("invalid XPLAIN_MCP_PORT", 0, C::new().fg("#dc322f"));
    assert!(!s.file_exists("${STATE}/xplain/mcp.json"));
}

/// Start fail (token file not writable): stays off, error row kept until next start attempt.
///
/// The real scenario also stops the server and holds the port with a foreign listener to get the port-busy
/// message mid-run (`holdPort`); the sim can only fake a busy port from the start: `f_mcpui_03_start_fail_busy`.
#[test]
fn f_mcpui_03_start_fail() {
    // state dir path is a regular file: token file write fails
    let mut s = base().file("${STATE}/xplain", "not a dir").build();
    s.keys("M<Enter>");
    s.assert_matches(r"^ +│> ○ off +│\n +│ \S[^\n]*│\n +│ clients 0 pending 0 delivered 0 +│$");
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("jk<Esc>M");
    // error row survives moves and modal reopen
    s.assert_matches(r"^ +│> ○ off +│\n +│ \S[^\n]*│\n +│ clients 0 pending 0 delivered 0 +│$");
    s.remove_file("${STATE}/xplain");
    s.keys("<Enter>");
    s.assert_matches(&port(r"^ +│> ● on  127\.0\.0\.1:${PORT} +│\n +│ clients 0 pending 0 delivered 0 +│$"));
    s.assert_row_contains(0, "[mcp: on]");
    let tok = s.file("${STATE}/xplain/mcp.json");
    let tok: serde_json::Value = serde_json::from_str(&tok).unwrap_or_default();
    assert!(tok.get("token").is_some(), "{tok}");
    // stop
    s.keys("<Enter>");
    s.assert_row_contains(0, "[mcp: off]");
}

/// Port busy start: the port-busy message (variant of the tail of `f_mcpui_03_start_fail`).
#[test]
fn f_mcpui_03_start_fail_busy() {
    let p: u16 = PORT.parse().unwrap_or(47615);
    let mut s = base().busy_port(p).build();
    s.keys("M<Enter>");
    // modal width cuts the row; the visible prefix of the message is checked
    s.assert_matches(&port(
        r"^ +│> ○ off +│\n +│ MCP port ${PORT} is already in use on 127\.0\.0\.1\. Stop the othe\S*│\n +│ clients 0 pending 0 delivered 0 +│$",
    ));
    s.assert_row_contains(0, "[mcp: off]");
}

/// Start: header [mcp: on], editor defaults to ask, registration check runs.
#[test]
fn f_mcpui_03_start() {
    let mut s = base().shim("claude").shim("codex").shim("copilot").build();
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("a");
    s.assert_contains("[save] enter send");
    s.assert_not_contains("[ask]");
    s.keys("<Esc>");
    assert_eq!(s.calls("claude").len(), 0);
    s.keys("M<Enter><Esc>");
    s.assert_row_contains(0, "[mcp: on]");
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 1);
    calls_exact(&s, "codex", &["mcp", "get", "xplain", "--json"], 1);
    calls_exact(&s, "copilot", &["mcp", "get", "xplain", "--json"], 1);
    s.keys("a");
    s.assert_contains("[ask] enter send");
    s.assert_not_contains("[save]");
}

/// Integration statuses kept after stop.
#[test]
fn f_mcpui_03_statuses_kept() {
    let mut s = base()
        .shim_rules("claude", vec![get(0)])
        .shim_rules("copilot", vec![get(0).stdout(r#"{"url": "http://127.0.0.1:1/mcp"}"#)])
        .build();
    s.keys("M<Enter>");
    all(&s, &[r"^ +│  Claude Code registered", r"^ +│  Copilot stale"]);
    s.keys("<Enter>");
    all(
        &s,
        &[
            r"^ +│> ○ off",
            r"^ +│  Claude Code registered",
            r"^ +│  Copilot stale",
            r"^ +│  Codex not registered",
        ],
    );
    assert_eq!(s.calls("claude").len(), 1);
}

const CLOSED: &str = r#"{"status":"closed","note":"xplain closed the session. Stop."}"#;

/// Stop: live answers cancelled (MCP stopped), header off, clients cleared, pending 0, editor back to save.
#[test]
fn f_mcpui_03_stop() {
    let mut s = base().build();
    s.keys("M<Enter><Esc>");
    let r = s.mcp_rpc("initialize", init("stopclient", Some("1")));
    assert_eq!(r.status, 200);
    s.keys("a");
    s.assert_contains("[ask] enter send");
    s.keys("first question<Enter>");
    s.assert_contains("question sent to agent");
    let r = s.mcp_call("next_question", json!({"wait_seconds": 1}));
    let q = r.tool_result();
    assert_eq!(q["status"], "question");
    assert!(q["question"].as_str().is_some_and(|t| t.contains("first question")), "{q}");
    s.keys("jjja");
    s.keys("second question<Enter>");
    s.assert_contains("question sent to agent");
    s.keys("M");
    all(&s, &[r"│ clients \d+ pending 1 delivered 1 +│", r"│   stopclient 1 +│"]);
    s.keys("<Enter>");
    s.assert_matches(
        r"│> ○ off +│[^\n]*\n[^\n]*│ clients 0 pending 0 delivered \d+ +│[^\n]*\n[^\n]*│ INTEGRATIONS",
    );
    s.assert_not_contains("stopclient");
    s.assert_row_contains(0, "[mcp: off]");
    s.keys("<Esc>");
    s.assert_matches(
        r"answer · \S+ · cancelled[^\n]*\n[^\n]*MCP stopped[\s\S]*answer · \S+ · cancelled[^\n]*\n[^\n]*MCP stopped",
    );
    none(&s, &["waiting…", "streaming…", "agent working", "waiting for agent"]);
    s.keys("a");
    s.assert_contains("[save] enter send");
    s.assert_not_contains("[ask]");
    s.keys("<Esc>");
    // restart, two clients wait in next_question; stop answers both at once with closed, HTTP 200
    s.keys("M<Enter>");
    s.assert_row_contains(0, "[mcp: on]");
    let w1 = s.http_start(Http::tool("next_question", json!({"wait_seconds": 120})));
    s.settle();
    // a real second connection has its own source port (clients without a session are told apart by it)
    let w2 = s.http_start(Http::tool("next_question", json!({"wait_seconds": 120})).remote_port(40002));
    s.settle();
    s.keys("<Enter>");
    s.assert_row_contains(0, "[mcp: off]");
    for w in [w1, w2] {
        let r = s.http_await(&w);
        assert_eq!(r.status, 200);
        assert_eq!(r.json()["result"]["content"][0]["type"], "text");
        assert_eq!(r.json()["result"]["content"][0]["text"], CLOSED);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// F-MCPUI-04: autostart

/// Autostart failure: note "mcp autostart failed: <error>", stays off.
#[test]
fn f_mcpui_04_autostart_fail() {
    let mut s =
        base().config_json(json!({"mcp": {"autostart": true}})).env("XPLAIN_MCP_PORT", "nope").build();
    s.assert_row_contains(0, "[mcp: off]");
    s.assert_row_contains(-1, r#"mcp autostart failed: invalid XPLAIN_MCP_PORT "nope" (0-65535)"#);
    s.keys("M");
    s.assert_matches(r#"^ +│> ○ off +│\n +│ invalid XPLAIN_MCP_PORT "nope" \(0-65535\) +│$"#);
}

/// mcp.autostart false: server not started, no check.
#[test]
fn f_mcpui_04_autostart_off() {
    let s = base().config_json(json!({"mcp": {"autostart": false}})).shim("claude").build();
    s.assert_row_contains(0, "[mcp: off]");
    assert_eq!(s.calls("claude").len(), 0);
    assert!(!s.file_exists("${STATE}/xplain/mcp.json"));
}

/// mcp.autostart true: server started after first frame, check runs, token never shown.
#[test]
fn f_mcpui_04_autostart() {
    let mut s = with_token()
        .config_json(json!({"mcp": {"autostart": true}}))
        .shim_rules("claude", vec![get(0)])
        .build();
    s.assert_row_contains(0, "[mcp: on]");
    s.assert_not_contains(TOKEN);
    calls_exact(&s, "claude", &["mcp", "get", "xplain"], 1);
    let r = s.mcp_rpc("ping", json!({}));
    assert_eq!(r.status, 200);
    assert_eq!(r.rpc_result(), json!({}));
    s.keys("M");
    all(&s, &[r"^ +│> ● on  127\.0\.0\.1:${PORT} +│$", r"^ +│  Claude Code registered"]);
    s.assert_not_contains(TOKEN);
}
