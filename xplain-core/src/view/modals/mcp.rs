//! MCP modal (F-MCPUI-01).

use super::*;
use crate::integration::RegStatus;
use crate::state::McpModal;
use crate::textutil;
use crate::view::layout::MCP_W;

pub(super) fn host_of(url: &str) -> String {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    rest.split('/').next().unwrap_or("").to_string()
}

pub(super) const MCP_CLIENTS_MAX: usize = 2;
pub(super) const MCP_PREVIEW_MAX: usize = 3;

/// All inner rows of the MCP modal (F-MCPUI-01), each a run of styled segments.
pub(super) fn mcp_lines(state: &State, m: &McpModal, lk: &Look) -> Vec<Vec<Seg>> {
    let mcp = &state.mcp;
    let inner_w = 10.max(usize::from(state.size.cols.min(MCP_W)).saturating_sub(4));
    let mut out: Vec<Vec<Seg>> = vec![vec![seg(" MCP", bold(lk.fill))]];
    let base = lk.row(m.row == 0);
    let mut power = format!(
        "{} {}",
        if m.row == 0 { ">" } else { " " },
        if mcp.starting().is_some() {
            "\u{2026} starting"
        } else if mcp.is_running() {
            "\u{25cf} on "
        } else {
            "\u{25cb} off"
        }
    );
    if mcp.is_running() {
        let host = mcp.endpoint().map(|e| host_of(&e.url)).unwrap_or_default();
        power.push_str(&format!(" {host}"));
    }
    out.push(vec![seg(power, base)]);
    if let Some(e) = mcp.start_error() {
        out.push(vec![seg(format!(" {e}"), lk.err)]);
    }
    out.push(vec![seg(
        format!(" clients {} pending {} delivered {}", mcp.clients.len(), mcp.queue.len(), mcp.delivered),
        lk.dim,
    )]);
    for cl in mcp.clients.iter().take(MCP_CLIENTS_MAX) {
        let version = if cl.version.is_empty() { String::new() } else { format!(" {}", cl.version) };
        let poll = if cl.polling { " \u{27f3}" } else { "" };
        out.push(vec![seg(format!("   {}{version}{poll}", cl.name), lk.fill)]);
    }
    if mcp.clients.len() > MCP_CLIENTS_MAX {
        out.push(vec![seg(format!("   \u{2026} +{} more", mcp.clients.len() - MCP_CLIENTS_MAX), lk.dim)]);
    }
    out.push(vec![seg(" INTEGRATIONS", lk.accent)]);
    for (i, integ) in state.integrations.iter().enumerate() {
        let ui = state.integration_state.get(i).cloned().unwrap_or_default();
        let on = m.row == i + 1;
        let status = if !integ.can_register() {
            "copy-paste only"
        } else {
            match ui.status {
                RegStatus::Registered => "registered",
                RegStatus::Stale => "stale",
                RegStatus::NotRegistered => "not registered",
            }
        };
        let restart =
            if integ.registration().is_some_and(|r| r.needs_restart()) { "  restart needed" } else { "" };
        out.push(vec![seg(
            format!(
                "{} {} {}{status}{restart}",
                if on { ">" } else { " " },
                integ.label(),
                if ui.busy { "\u{2026} " } else { "" }
            ),
            lk.row(on),
        )]);
    }
    let confirming = m.confirm.is_some();
    if let Some((idx, register)) = m.confirm {
        if let Some(integ) = state.integrations.get(idx) {
            let label = integ.label();
            if register {
                out.push(vec![seg(format!(" Register {label} MCP server? (y/n)"), lk.accent)]);
                out.push(vec![seg(format!(" adds the xplain MCP server to {label} config"), lk.dim)]);
            } else {
                out.push(vec![seg(format!(" Remove {label} registration? (y/n)"), lk.accent)]);
                out.push(vec![seg(format!(" removes the xplain MCP server from {label} config"), lk.dim)]);
            }
        }
    }
    if !confirming {
        let selected_msg =
            m.row.checked_sub(1).and_then(|i| state.integration_state.get(i)).and_then(|u| u.message.clone());
        if let Some(note) = m.note.clone().or(selected_msg) {
            for l in textutil::wrap_text(&note, inner_w).into_iter().take(2) {
                out.push(vec![seg(format!(" {l}"), lk.accent)]);
            }
        }
        if let Some((what, text)) = &m.preview {
            out.push(vec![seg(format!(" {what}:"), lk.dim)]);
            let pv = textutil::wrap_text(text, inner_w);
            let mut shown: Vec<String> = pv.iter().take(MCP_PREVIEW_MAX).cloned().collect();
            if pv.len() > MCP_PREVIEW_MAX {
                if let Some(last) = shown.last_mut() {
                    *last = format!("{}\u{2026}", last.chars().take(inner_w - 1).collect::<String>());
                }
            }
            for l in shown {
                out.push(vec![seg(format!(" {l}"), lk.fill)]);
            }
        }
    }
    let hint = if confirming {
        " y confirm  n/esc cancel"
    } else {
        " j/k  enter register  d remove  c/w copy  R refresh  esc"
    };
    out.push(vec![seg(hint, lk.dim)]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Color;
    use crate::view::modals::fixtures::*;
    use crate::view::testutil::{rgb, state, theme};

    fn mcp_state(rows: usize) -> State {
        let mut st = state(80, 24);
        st.mcp.server = crate::mcp::ServerState::Running(crate::mcp::McpEndpoint {
            url: "http://127.0.0.1:47615/mcp".into(),
            token: "t".into(),
            port: 47615,
        });
        st.integration_state.resize(rows, Default::default());
        st
    }

    fn texts(lines: &[Vec<Seg>]) -> Vec<String> {
        lines.iter().map(|l| l.iter().map(|s| s.0.as_str()).collect::<String>()).collect()
    }

    #[test]
    fn f_mcpui_01_spec_example() {
        let st = mcp_state(st_ints());
        let lines = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(lines[0], " MCP");
        assert_eq!(lines[1], "> \u{25cf} on  127.0.0.1:47615");
        assert_eq!(lines[2], " clients 0 pending 0 delivered 0");
        assert_eq!(lines[3], " INTEGRATIONS");
        assert_eq!(lines[4], "  Alpha not registered  restart needed");
        assert_eq!(lines[5], "  Beta copy-paste only");
        assert_eq!(lines[6], " j/k  enter register  d remove  c/w copy  R refresh  esc");
        assert_eq!(lines.len(), 7);
    }

    fn st_ints() -> usize {
        state(80, 24).integrations.len()
    }

    #[test]
    fn f_mcpui_01_power_row_variants() {
        let mut st = mcp_state(2);
        st.mcp.set_running(false);
        let l = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(l[1], "> \u{25cb} off");
        st.mcp.server = crate::mcp::ServerState::Starting(crate::event::ReqId(1));
        let l = texts(&mcp_lines(&st, &McpModal { row: 1, ..Default::default() }, &look()));
        assert_eq!(l[1], "  \u{2026} starting");
        st.mcp.server = crate::mcp::ServerState::Stopped { last_error: Some("port busy".into()) };
        let l = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(l[2], " port busy");
    }

    #[test]
    fn f_mcpui_01_clients_list() {
        let mut st = mcp_state(2);
        let cl = |n: &str, v: &str, p: bool| crate::mcp::ClientInfo {
            id: n.into(),
            name: n.into(),
            version: v.into(),
            polling: p,
        };
        st.mcp.clients = vec![cl("one", "1.2", true), cl("two", "", false), cl("three", "3", false)];
        st.mcp.delivered = 4;
        let l = texts(&mcp_lines(&st, &McpModal::default(), &look()));
        assert_eq!(l[2], " clients 3 pending 0 delivered 4");
        assert_eq!(l[3], "   one 1.2 \u{27f3}");
        assert_eq!(l[4], "   two");
        assert_eq!(l[5], "   \u{2026} +1 more");
        assert_eq!(l[6], " INTEGRATIONS");
    }

    #[test]
    fn f_mcpui_01_busy_status_and_selection() {
        let mut st = mcp_state(2);
        st.integration_state[0].busy = true;
        st.integration_state[0].status = RegStatus::Stale;
        let m = McpModal { row: 1, ..Default::default() };
        let lk = look();
        let lines = mcp_lines(&st, &m, &lk);
        let t = texts(&lines);
        assert_eq!(t[4], "> Alpha \u{2026} stale  restart needed");
        assert_eq!(lines[4][0].1, lk.sel);
        assert_eq!(lines[1][0].1, lk.fill);
    }

    #[test]
    fn f_mcpui_01_confirm_hides_note_and_preview() {
        let st = mcp_state(2);
        let m = McpModal {
            row: 1,
            confirm: Some((0, true)),
            note: Some("hidden".into()),
            preview: Some(("watch prompt (copied)".into(), "x".into())),
        };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " Register Alpha MCP server? (y/n)");
        assert_eq!(t[7], " adds the xplain MCP server to Alpha config");
        assert_eq!(t[8], " y confirm  n/esc cancel");
        assert_eq!(t.len(), 9);
        let m = McpModal { confirm: Some((0, false)), ..m };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " Remove Alpha registration? (y/n)");
        assert_eq!(t[7], " removes the xplain MCP server from Alpha config");
    }

    #[test]
    fn f_mcpui_01_note_wrapped_max_two_rows_and_selected_message() {
        let mut st = mcp_state(2);
        st.integration_state[0].message = Some("last result".into());
        let m = McpModal { row: 1, ..Default::default() };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " last result");
        let m = McpModal { row: 1, note: Some("own note".into()), ..Default::default() };
        assert_eq!(texts(&mcp_lines(&st, &m, &look()))[6], " own note");
        let long = "word ".repeat(40);
        let m = McpModal { row: 1, note: Some(long), ..Default::default() };
        let l = mcp_lines(&st, &m, &look());
        let t = texts(&l);
        assert_eq!(t.len(), 9);
        assert_eq!(l[6][0].1, look().accent);
    }

    #[test]
    fn f_mcpui_01_preview_lines_and_ellipsis() {
        let st = mcp_state(2);
        let m = McpModal {
            preview: Some(("register command (copied)".into(), "short".into())),
            ..Default::default()
        };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[6], " register command (copied):");
        assert_eq!(t[7], " short");
        assert_eq!(t[8], " j/k  enter register  d remove  c/w copy  R refresh  esc");
        // inner width 60: four 60-char lines -> 3 rows, third cut to 59 + ellipsis
        let text = ["a".repeat(60), "b".repeat(60), "c".repeat(60), "d".repeat(60)].join("\n");
        let m = McpModal { preview: Some(("p".into(), text)), ..Default::default() };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[7], format!(" {}", "a".repeat(60)));
        assert_eq!(t[9], format!(" {}\u{2026}", "c".repeat(59)));
        assert_eq!(t.len(), 11);
        // blank third line gives ` …`
        let m = McpModal { preview: Some(("p".into(), "a\nb\n\nd".into())), ..Default::default() };
        let t = texts(&mcp_lines(&st, &m, &look()));
        assert_eq!(t[9], " \u{2026}");
    }

    #[test]
    fn f_mcpui_01_modal_drawn_centered_width_64() {
        let mut st = mcp_state(2);
        st.overlay = Overlay::Mcp(McpModal::default());
        let mut c = canvas(80, 24);
        // this overlay needs no other component
        draw(&mut c, &st, &theme());
        let s = c.into_screen();
        // 7 content rows + 2 -> h 9 at y = ceil(15/2) = 8; x = 8
        assert!(s.row_text(8).chars().skip(8).take(64).collect::<String>().starts_with('\u{256d}'));
        assert!(s.row_text(9).contains("\u{2502} MCP"));
        assert!(s.row_text(16).chars().skip(8).take(64).collect::<String>().ends_with('\u{256f}'));
        let x = cell_x(&s, 10, "> ");
        assert_eq!(s.rows[10][x].style.bg, Some(theme().sel_bg));
        assert_eq!(theme().sel_bg, rgb(0x073642));
        assert_eq!(s.rows[10][0].style.fg, None::<Color>);
    }
}
