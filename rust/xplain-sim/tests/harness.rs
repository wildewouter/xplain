//! Self-tests of the harness itself: held IO, manual clock, startup exits, fake CLIs, fixtures.

#![allow(clippy::expect_used)]

use serde_json::json;
use xplain_core::mcp::McpEndpoint;
use xplain_sim::{Fixture, Http, Rule, Sim};

#[test]
fn hold_io_shows_loading_until_released() {
    let mut s = Sim::builder().hold_io().build();
    s.assert_contains("Loading...");
    assert!(s.held_io() > 0 && !s.is_ready());
    s.keys("j");
    s.assert_contains("Loading...");
    s.release_io();
    assert_eq!(s.held_io(), 0);
    s.assert_row_contains(0, "[all]");
    assert!(s.is_ready());
}

#[test]
fn manual_clock_times_out_long_poll() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 1})));
    s.advance_clock(999);
    assert!(s.http_reply(&poll).is_none());
    s.advance_clock(1);
    let r = s.http_await(&poll);
    assert_eq!(r.tool_result()["status"], "no_question_yet");
    assert_eq!(s.elapsed_ms(), 1000);
}

#[test]
fn aborted_poll_frees_the_slot() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    let poll = s.http_start(Http::tool("next_question", json!({"wait_seconds": 60})));
    s.http_abort(&poll);
    assert!(s.http_reply(&poll).is_none());
    let r = s.mcp_rpc("tools/list", json!({}));
    assert_eq!(r.status, 200);
}

#[test]
fn http_checks_are_core_checks() {
    let mut s = Sim::builder().config_json(json!({"mcp": {"autostart": true}})).build();
    assert_eq!(s.http(Http::rpc("tools/list", json!({})).no_auth()).status, 401);
    assert_eq!(s.http(Http::rpc("tools/list", json!({})).token("nope")).status, 401);
    assert_eq!(s.http(Http::rpc("tools/list", json!({})).header("host", "evil.example")).status, 403);
    assert_eq!(s.http(Http::rpc("tools/list", json!({})).path("/x")).status, 404);
    assert_eq!(s.http(Http::rpc("tools/list", json!({})).method("GET")).status, 405);
    assert_eq!(s.http(Http::oversized()).status, 413);
}

#[test]
#[should_panic(expected = "connection refused")]
fn http_without_server_is_refused() {
    let mut s = Sim::builder().build();
    s.mcp_call("answer", json!({}));
}

#[test]
fn help_and_flag_errors_exit_at_startup() {
    let s = Sim::builder().args(["-h"]).build();
    assert_eq!(s.exit_code(), Some(0));
    assert!(s.stdout().starts_with("usage: xplain"));
    let s = Sim::builder().args(["--mode", "x"]).build();
    assert_eq!(s.exit_code(), Some(1));
    assert!(s.stderr().starts_with("xplain: invalid mode: x\n"));
    let s = Sim::builder().args(["--config", "${TMP}/c.json", "config", "path"]).build();
    assert_eq!(s.stdout(), format!("{}/c.json\n", s.tmp().display()));
}

#[test]
fn config_warning_goes_to_stderr() {
    let s = Sim::builder().config("{oops").build();
    assert!(s.stderr().starts_with("xplain: config: "), "{}", s.stderr());
    s.assert_row_contains(0, "[all]");
}

#[test]
fn fixtures() {
    let s = Sim::builder().fixture(Fixture::Empty).build();
    assert!(s.file_exists(".git"));
    s.assert_not_contains("README.md");
    let s = Sim::builder().fixture(Fixture::NoGit).build();
    assert!(!s.file_exists(".git"));
    s.assert_contains("Not a git repository");
    let s = Sim::builder().build();
    assert!(s.file("README.md").starts_with("# Title"));
    assert_eq!(s.git(&["rev-list", "--count", "HEAD"]).trim(), "1");
}

#[test]
fn cwd_and_size() {
    let s = Sim::builder().cwd("src").size(60, 20).build();
    assert_eq!(s.screen().len(), 20);
    assert!(s.render().rows[0].len() == 60);
    assert!(s.repo().join("src").is_dir());
    s.assert_row_contains(0, "[all]");
}

#[test]
fn ctrl_c_exits_and_later_input_is_a_test_bug() {
    let mut s = Sim::builder().build();
    s.keys("<C-c>");
    assert_eq!(s.exit_code(), Some(0));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.keys("j");
    }));
    assert!(r.is_err());
}

#[test]
fn quit_confirmation_and_resize() {
    let mut s = Sim::builder().build();
    s.resize(50, 12);
    assert_eq!(s.screen().len(), 12);
    s.keys("q");
    s.assert_contains("Quit xplain? (y/n)");
    s.keys("n");
    s.assert_not_contains("Quit xplain?");
    assert_eq!(s.exit_code(), None);
}

/// First registrable integration of the real registry: its CLI name, and the argv of its register command.
fn registrable_cli() -> (String, Vec<String>) {
    let ep = McpEndpoint { url: "http://127.0.0.1:1/mcp".into(), token: "t".repeat(20), port: 1 };
    let all = xplain_integrations::all();
    let reg = all[0].registration().expect("first agent registrable");
    let last = reg.register_commands(&ep).pop().expect("add command");
    (last.program, last.args)
}

#[test]
fn fake_cli_blocks_until_released() {
    let (prog, add_args) = registrable_cli();
    let mut s = Sim::builder()
        .shim_rules(
            &prog,
            vec![
                Rule::args([add_args[0].as_str(), "get"]).exit(1),
                Rule::args([add_args[0].as_str(), add_args[1].as_str()]).block(),
            ],
        )
        .build();
    s.keys("M<Enter>j<Enter>");
    s.keys("y");
    assert_eq!(s.blocked_calls(&prog), 1, "{}", s.dump());
    s.assert_matches(r"│> [^│]*… ");
    s.release(&prog);
    assert_eq!(s.blocked_calls(&prog), 0);
    s.assert_not_contains("… ");
    let calls = s.calls(&prog);
    assert!(calls.iter().any(|c| c.args.starts_with(&add_args[..2])), "{calls:?}");
}
