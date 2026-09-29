//! Shared helpers for the agent adapters (no agent names here either, keep it generic).
//!
//! Spec: F-INTEG-02 (check parsing, URL extraction), F-INTEG-06 (watch prompt template),
//! UNSPEC-37 (20 s timeout). Owner: integrations worker.
//! Must not: perform IO; know a specific agent; render notes (core composes and masks notes).

use xplain_core::integration::{CommandResult, CommandSpec, RegStatus};
use xplain_core::mcp::McpEndpoint;

/// Timeout for every integration CLI (UNSPEC-37).
pub const CLI_TIMEOUT_MS: u64 = 20_000;

/// `CommandSpec { program, args, cwd: None, env: [], timeout_ms: CLI_TIMEOUT_MS }`.
pub fn cli_command(program: &str, args: &[&str]) -> CommandSpec {
    CommandSpec {
        program: program.to_string(),
        args: args.iter().map(|a| (*a).to_string()).collect(),
        cwd: None,
        env: Vec::new(),
        timeout_ms: CLI_TIMEOUT_MS,
    }
}

/// `Authorization: Bearer <token>` (single argv element value, no quotes).
pub fn bearer_header(token: &str) -> String {
    format!("Authorization: Bearer {token}")
}

/// F-INTEG-06 template with `<P>` = `poll_seconds`, tool names in backticks unchanged.
pub fn watch_prompt(poll_seconds: u32) -> String {
    [
        format!(
            "Loop forever: call the `next_question` tool from the `xplain` MCP server with wait_seconds={poll_seconds}."
        ),
        "- If status is `no_question_yet`, call `next_question` again IMMEDIATELY.".to_string(),
        "- If status is `question`, answer it with the `answer` tool using the given thread_id, then call `next_question` again immediately.".to_string(),
        "- Questions with follow_up=true continue an earlier thread; answer them with the same thread_id like any other.".to_string(),
        "- ALWAYS call the `files_changed` tool (with the changed paths) after every edit, creation or deletion of a file, so the xplain view reloads. This also applies to edits made while answering a question.".to_string(),
        "- Never stop, never summarize, never ask the user anything.".to_string(),
        "- Stop only if status is `closed`.".to_string(),
    ]
    .join("\n")
}

fn json_url(v: &serde_json::Value) -> Option<String> {
    match v.get("url")? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// F-INTEG-02 URL extraction: JSON `url`, `xplain.url`, `mcpServers.xplain.url`, else first
/// `http(s)://` URL in text (ends at whitespace, `"`, `'`, `,`).
pub fn parse_url(stdout: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
        let found = json_url(&v)
            .or_else(|| v.get("xplain").and_then(json_url))
            .or_else(|| v.get("mcpServers").and_then(|m| m.get("xplain")).and_then(json_url));
        if found.is_some() {
            return found;
        }
    }
    first_http_url(stdout)
}

fn first_http_url(text: &str) -> Option<String> {
    let is_end = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',');
    let mut from = 0;
    while from < text.len() {
        let rest = &text[from..];
        let start = match (rest.find("http://"), rest.find("https://")) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => return None,
        };
        let abs = from + start;
        let tail = &text[abs..];
        let scheme_len = if tail.starts_with("https://") { 8 } else { 7 };
        let end = tail.find(is_end).unwrap_or(tail.len());
        if end > scheme_len {
            return Some(tail[..end].to_string());
        }
        from = abs + scheme_len;
    }
    None
}

/// F-INTEG-02 generic check: `Err` or non-zero exit => NotRegistered; exit 0 => Registered, or Stale
/// when a URL was found and differs from `ep.url`.
pub fn parse_check(ep: &McpEndpoint, result: &CommandResult) -> RegStatus {
    match result {
        Ok(out) if out.code == 0 => match parse_url(&out.stdout) {
            Some(url) if url != ep.url => RegStatus::Stale,
            _ => RegStatus::Registered,
        },
        _ => RegStatus::NotRegistered,
    }
}

/// F-INTEG-03 composition: `<hint>; restart the agent session, then paste the watch prompt`.
/// Agent modules pass their bare hint; the result is the full note `register_hint` returns.
pub fn compose_register_note(hint: &str) -> String {
    format!("{hint}; restart the agent session, then paste the watch prompt")
}

#[cfg(test)]
mod tests {
    use super::*;
    use xplain_core::integration::{CommandError, CommandOutput};

    fn ep() -> McpEndpoint {
        McpEndpoint { url: "http://127.0.0.1:1234/mcp".into(), token: "tok".into(), port: 1234 }
    }

    fn ok(code: i32, stdout: &str) -> CommandResult {
        Ok(CommandOutput { code, stdout: stdout.into(), stderr: String::new() })
    }

    #[test]
    fn cli_command_shape() {
        let c = cli_command("prog", &["a", "b c"]);
        assert_eq!(c.program, "prog");
        assert_eq!(c.args, vec!["a", "b c"]);
        assert_eq!(c.cwd, None);
        assert!(c.env.is_empty());
        assert_eq!(c.timeout_ms, 20_000);
    }

    #[test]
    fn bearer() {
        assert_eq!(bearer_header("abc"), "Authorization: Bearer abc");
    }

    #[test]
    fn prompt_text() {
        let want = "Loop forever: call the `next_question` tool from the `xplain` MCP server with wait_seconds=100.\n\
- If status is `no_question_yet`, call `next_question` again IMMEDIATELY.\n\
- If status is `question`, answer it with the `answer` tool using the given thread_id, then call `next_question` again immediately.\n\
- Questions with follow_up=true continue an earlier thread; answer them with the same thread_id like any other.\n\
- ALWAYS call the `files_changed` tool (with the changed paths) after every edit, creation or deletion of a file, so the xplain view reloads. This also applies to edits made while answering a question.\n\
- Never stop, never summarize, never ask the user anything.\n\
- Stop only if status is `closed`.";
        assert_eq!(watch_prompt(100), want);
        assert!(watch_prompt(45).contains("wait_seconds=45."));
    }

    #[test]
    fn url_json_variants() {
        assert_eq!(parse_url(r#"{"url":"http://a/b"}"#).as_deref(), Some("http://a/b"));
        assert_eq!(parse_url(r#"{"xplain":{"url":"http://x"}}"#).as_deref(), Some("http://x"));
        assert_eq!(parse_url(r#"{"mcpServers":{"xplain":{"url":"http://y"}}}"#).as_deref(), Some("http://y"));
        assert_eq!(parse_url(r#"{"url":"a","xplain":{"url":"b"}}"#).as_deref(), Some("a"));
    }

    #[test]
    fn url_json_without_url_falls_to_text() {
        assert_eq!(parse_url(r#"{"note":"see https://z.io/q now"}"#).as_deref(), Some("https://z.io/q"));
        assert_eq!(parse_url(r#"{"url":""}"#), None);
        assert_eq!(parse_url("42"), None);
    }

    #[test]
    fn url_text_variants() {
        assert_eq!(
            parse_url("xplain:\n  URL: http://127.0.0.1:9/mcp\n  x").as_deref(),
            Some("http://127.0.0.1:9/mcp")
        );
        assert_eq!(parse_url("a \"https://q/r\" b").as_deref(), Some("https://q/r"));
        assert_eq!(parse_url("'http://q/r',x").as_deref(), Some("http://q/r"));
        assert_eq!(parse_url("http://q/r,s").as_deref(), Some("http://q/r"));
        assert_eq!(parse_url("http:// then https://ok").as_deref(), Some("https://ok"));
        assert_eq!(parse_url("no url here"), None);
        assert_eq!(parse_url(""), None);
    }

    #[test]
    fn check_statuses() {
        let e = ep();
        assert_eq!(parse_check(&e, &ok(0, "")), RegStatus::Registered);
        assert_eq!(parse_check(&e, &ok(0, "no url")), RegStatus::Registered);
        assert_eq!(parse_check(&e, &ok(0, "URL: http://127.0.0.1:1234/mcp\n")), RegStatus::Registered);
        assert_eq!(parse_check(&e, &ok(0, r#"{"url":"http://127.0.0.1:1234/mcp"}"#)), RegStatus::Registered);
        assert_eq!(parse_check(&e, &ok(0, "URL: http://127.0.0.1:99/mcp\n")), RegStatus::Stale);
        assert_eq!(parse_check(&e, &ok(1, "http://127.0.0.1:1234/mcp")), RegStatus::NotRegistered);
        for err in [CommandError::NotFound, CommandError::Timeout, CommandError::Other("x".into())] {
            assert_eq!(parse_check(&e, &Err(err)), RegStatus::NotRegistered);
        }
    }

    #[test]
    fn note() {
        assert_eq!(
            compose_register_note("Hi."),
            "Hi.; restart the agent session, then paste the watch prompt"
        );
    }
}
