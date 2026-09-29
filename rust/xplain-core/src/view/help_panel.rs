//! Help panel drawn over everything.
//!
//! Spec: F-HELP-01 (panel content, title, levels), F-HELP-02 (context title), F-HELP-03 (entries grouped),
//! F-LAYOUT-06 (help panel placement: top at screen row 3). Oracle: `src/components/HelpModal.tsx`.
//! Owner: component `viewframe` (F1). Data from `help::{help_ctx, ctx_label, entries_for, has_motions}`.

use crate::canvas::{Canvas, Rect};
use crate::help::{self, HelpCtx, HelpEntry};
use crate::screen::{Style, bold, fg};
use crate::state::{HelpLevel, State};
use crate::theme::Theme;

const CREDIT: &str = "Made by Wouter de Wild - 2026";
const CREDIT_W: usize = 29;

/// Contexts where `?` is typed text, so the panel has no close hint.
fn typing(ctx: HelpCtx) -> bool {
    matches!(ctx, HelpCtx::Editor | HelpCtx::Find | HelpCtx::Goto | HelpCtx::Search)
}

/// Word wrap to `w` chars: break at the last space that fits, hard cut long words.
fn wrap(s: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;
    for word in s.split(' ') {
        let wl = word.chars().count();
        if cur_len > 0 && cur_len + 1 + wl <= w {
            cur.push(' ');
            cur.push_str(word);
            cur_len += 1 + wl;
        } else {
            if cur_len > 0 {
                out.push(std::mem::take(&mut cur));
            }
            cur = word.to_string();
            cur_len = wl;
            while cur_len > w {
                let head: String = cur.chars().take(w).collect();
                cur = cur.chars().skip(w).collect();
                cur_len -= w;
                out.push(head);
            }
        }
    }
    out.push(cur);
    out
}

/// One group of consecutive-by-first-appearance entries with the lines each wraps to.
struct Group {
    name: String,
    items: Vec<(String, Vec<String>)>,
}

/// Computed panel: what to draw and how big (F-HELP-01).
pub struct Panel {
    title: String,
    /// Box width and height in cells.
    pub w: u16,
    pub h: u16,
    inner: usize,
    pad: usize,
    groups: Vec<Group>,
    more: Option<usize>,
    hint: Option<&'static str>,
    credit: bool,
}

/// Lay out the panel for `entries` in a `cols` x `rows` screen (F-HELP-01 width, height, overflow, tiny).
pub fn layout(
    label: &str,
    entries: &[HelpEntry],
    ctx_has_motions: bool,
    motions: bool,
    is_typing: bool,
    cols: u16,
    rows: u16,
) -> Panel {
    let close = !is_typing;
    let hint: &'static str =
        if close { if !motions && ctx_has_motions { " ? move keys" } else { " ? close" } } else { "" };
    let title = format!(" Help \u{b7} {label}");
    let mut names: Vec<&str> = Vec::new();
    for e in entries {
        if !names.contains(&e.group) {
            names.push(e.group);
        }
    }
    let pad = entries.iter().map(|e| e.keys.chars().count()).max().unwrap_or(0) + 1;
    let hint_part = if hint.is_empty() { 0 } else { hint.chars().count() + 1 };
    let natural = [
        2 + pad + entries.iter().map(|e| e.desc.chars().count()).max().unwrap_or(0),
        names.iter().map(|g| 1 + g.chars().count()).max().unwrap_or(0),
        title.chars().count(),
        hint_part + CREDIT_W + 1,
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    let width_cap = usize::from(cols) * 6 / 10;
    let width_cap = width_cap.clamp(36, 96);
    let inner = (width_cap.min(natural + 2) - 2).max(1);
    let desc_w = inner.saturating_sub(2 + pad).max(1);
    let groups: Vec<Group> = names
        .iter()
        .map(|g| Group {
            name: (*g).to_string(),
            items: entries
                .iter()
                .filter(|e| e.group == *g)
                .map(|e| (e.keys.clone(), wrap(&e.desc, desc_w)))
                .collect(),
        })
        .collect();
    let height_of = |gs: &[Group]| -> usize {
        gs.iter().map(|g| 1 + g.items.iter().map(|i| i.1.len()).sum::<usize>()).sum()
    };
    let max_height = usize::from(rows.saturating_sub(3).max(3));
    let room = max_height.saturating_sub(3);
    let tiny = room < 2;
    let avail = if tiny { room } else { room - usize::from(close) };
    let total = entries.len();
    let cut = height_of(&groups) > avail;
    let limit = if cut { avail.saturating_sub(1) } else { avail };
    let mut shown = 0usize;
    let mut body_h = 0usize;
    let mut body: Vec<Group> = Vec::new();
    for g in groups {
        if body_h + 2 > limit {
            break;
        }
        body_h += 1;
        let mut items = Vec::new();
        let all = g.items.len();
        for i in g.items {
            let ih = i.1.len();
            if body_h + ih > limit {
                break;
            }
            body_h += ih;
            items.push(i);
        }
        if items.is_empty() {
            break;
        }
        shown += items.len();
        let partial = items.len() < all;
        body.push(Group { name: g.name, items });
        if partial {
            break;
        }
    }
    let credit = !tiny && inner > hint_part + CREDIT_W && (close || (!cut && body_h < avail));
    let bottom = !tiny && (close || credit);
    let more = (cut && avail > 0).then_some(total - shown);
    let h = 3 + body_h + usize::from(more.is_some()) + usize::from(bottom);
    Panel {
        title,
        w: (inner + 2) as u16,
        h: h as u16,
        inner,
        pad,
        groups: body,
        more,
        hint: (bottom && close).then_some(hint),
        credit,
    }
}

impl Panel {
    /// Draw at (x, y).
    fn draw(&self, c: &mut Canvas, x: u16, y: u16, theme: &Theme) {
        let fill = Style { fg: Some(theme.modal_fg), bg: Some(theme.modal_bg), ..Style::default() };
        let border = fg(theme.modal_border);
        c.draw_box(Rect { x, y, w: self.w, h: self.h }, border, fill);
        let ix = x + 1;
        let iw = self.inner as u16;
        let mut row = y + 1;
        c.put_trunc(ix, row, iw, &self.title, bold(fill));
        row += 1;
        let group_style = Style { fg: Some(theme.accent), ..fill };
        let key_style = Style { fg: Some(theme.mode), ..fill };
        let dim = Style { fg: Some(theme.dim), ..fill };
        let desc_x = ix + 2 + self.pad as u16;
        for g in &self.groups {
            c.put_trunc(ix, row, iw, &format!(" {}", g.name), group_style);
            row += 1;
            for (key, lines) in &g.items {
                c.put_trunc(ix, row, 2 + self.pad as u16, &format!("  {key}"), key_style);
                for (k, line) in lines.iter().enumerate() {
                    c.put_trunc(desc_x, row + k as u16, iw.saturating_sub(2 + self.pad as u16), line, fill);
                }
                row += lines.len() as u16;
            }
        }
        if let Some(k) = self.more {
            c.put_trunc(ix, row, iw, &format!(" \u{2026} {k} more"), dim);
            row += 1;
        }
        if self.hint.is_some() || self.credit {
            if let Some(h) = self.hint {
                c.put_trunc(ix, row, iw, h, dim);
            }
            if self.credit {
                let cx = ix + iw - (CREDIT_W as u16 + 1);
                c.put(cx, row, &format!("{CREDIT} "), dim);
            }
        }
    }
}

/// F-LAYOUT-06 help placement: right edge on the last column, bottom edge on the row above the footer,
/// top never above screen row 3 (0-based row 2).
pub fn origin(cols: u16, rows: u16, w: u16, h: u16) -> (u16, u16) {
    (cols.saturating_sub(w), rows.saturating_sub(1).saturating_sub(h).max(2))
}

pub fn draw(c: &mut Canvas, state: &State, theme: &Theme) {
    if state.help == HelpLevel::Closed {
        return;
    }
    let ctx = help::help_ctx(state);
    let entries = help::entries_for(ctx, state.help == HelpLevel::L2);
    let size = c.size();
    let p = layout(
        help::ctx_label(ctx),
        &entries,
        help::has_motions(ctx),
        state.help == HelpLevel::L2,
        typing(ctx),
        size.cols,
        crate::view::frame_height(size),
    );
    let (x, y) = origin(size.cols, crate::view::frame_height(size), p.w, p.h);
    p.draw(c, x, y, theme);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Size;
    use crate::view::testutil::{rgb, theme};

    fn e(group: &'static str, keys: &str, desc: &str) -> HelpEntry {
        HelpEntry { group, keys: keys.to_string(), desc: desc.to_string() }
    }

    /// Diff view L1 (F-HELP-03).
    fn diff_l1() -> Vec<HelpEntry> {
        vec![
            e("Find", "]/[", "next/prev change"),
            e("Find", "/ n/N", "find, next/prev"),
            e("Find", ":", "go to line"),
            e("Find", "tab/S-tab", "next / prev file"),
            e("Find", "f/F", "file picker / search"),
            e("Comments", "v/V", "select chars/lines"),
            e("Comments", "a", "comment on line"),
            e("Comments", ")/(", "numbered, any file"),
            e("Comments", "E", "export comments"),
            e("General", "s/c/m", "split, full, staged"),
            e("General", "t/r", "theme / reload"),
            e("General", "p", "old/new pane"),
            e("General", "M/C", "MCP / config"),
            e("General", "q", "quit"),
        ]
    }

    fn render(p: &Panel, cols: u16, rows: u16) -> crate::screen::Screen {
        let mut c = Canvas::new(Size { cols, rows });
        let (x, y) = origin(cols, rows, p.w, p.h);
        p.draw(&mut c, x, y, &theme());
        c.into_screen()
    }

    #[test]
    fn f_help_01_layout_80x24_diff_view_l1() {
        let p = layout("Diff view", &diff_l1(), true, false, false, 80, 24);
        assert_eq!((p.w, p.h), (45, 21));
        let (x, y) = origin(80, 24, p.w, p.h);
        assert_eq!((x, y), (35, 2));
        let s = render(&p, 80, 24);
        assert!(s.row_text(2).ends_with("\u{256e}"));
        assert!(s.row_text(2).trim_end().starts_with(&" ".repeat(35)));
        assert!(s.row_text(3).contains("\u{2502} Help \u{b7} Diff view"));
        let hint = s.row_text(21);
        assert!(hint.contains("\u{2502} ? move keys"), "{hint}");
        assert!(hint.ends_with("Made by Wouter de Wild - 2026 \u{2502}"), "{hint}");
        assert!(s.row_text(22).ends_with("\u{256f}"));
        assert_eq!(s.row_text(23).trim(), "");
        assert_eq!(s.row_text(22).chars().count(), 80);
    }

    #[test]
    fn f_help_01_colors() {
        let p = layout("Diff view", &diff_l1(), true, false, false, 80, 24);
        let s = render(&p, 80, 24);
        let th = theme();
        let at = |row: usize, text: &str| {
            let t = s.row_text(row);
            let x = t.find(text).map(|b| t[..b].chars().count()).unwrap_or(0);
            s.rows[row][x].style
        };
        assert_eq!(at(22, "\u{2570}").fg, Some(th.modal_border));
        assert_eq!(at(22, "\u{2570}").bg, None);
        assert!(at(3, "Help").bold);
        assert_eq!(at(3, "Help").bg, Some(th.modal_bg));
        assert_eq!(at(4, "Find").fg, Some(th.accent));
        assert_eq!(at(5, "]/[").fg, Some(th.mode));
        assert_eq!(at(5, "next/prev change").fg, Some(th.modal_fg));
        assert_eq!(at(21, "? move keys").fg, Some(th.dim));
        assert_eq!(at(21, "Made by").fg, Some(th.dim));
        assert_eq!(th.modal_bg, rgb(0x002b36));
    }

    #[test]
    fn f_help_01_width_min_and_cap() {
        let small = [e("G", "a", "b")];
        let p = layout("Confirm", &small, false, false, false, 60, 24);
        // natural = hint(8)+1+30 = 39; W = max(36, 36) -> box 41 capped by W: min(36, 41) = 36
        assert_eq!(p.w, 36);
        let p = layout("Confirm", &small, false, false, false, 200, 24);
        assert_eq!(p.w, 41);
    }

    #[test]
    fn f_help_01_text_input_credit_only() {
        let ents = [
            e("Editor", "type", "comment text"),
            e("Editor", "tab", "save / ask agent"),
            e("Editor", "left/right", "move cursor"),
            e("Editor", "backspace", "delete char"),
        ];
        let p = layout("Editor", &ents, false, false, true, 120, 40);
        assert_eq!(p.h, 3 + 5 + 1);
        let s = render(&p, 120, 40);
        let (_, y) = origin(120, 40, p.w, p.h);
        let last = s.row_text(usize::from(y + p.h - 2));
        assert!(last.contains("Made by Wouter de Wild - 2026 \u{2502}"), "{last}");
        assert!(!last.contains('?'));
    }

    #[test]
    fn f_help_01_overflow_cut_more_row_no_credit_in_text_input() {
        let ents = [
            e("Find", "type", "search text"),
            e("Find", "backspace", "delete char"),
            e("Find", "Enter", "jump to match"),
            e("Find", "esc", "cancel"),
        ];
        // R=10: M=7, room 4, avail 4 (text input); cut since 5 rows; limit 3: group + 2 items
        let p = layout("Find in file", &ents, false, false, true, 80, 10);
        assert_eq!(p.more, Some(2));
        assert!(!p.credit);
        assert_eq!(p.h, 3 + 3 + 1);
        let s = render(&p, 80, 10);
        let (_, y) = origin(80, 10, p.w, p.h);
        assert!(s.row_text(usize::from(y + p.h - 2)).contains("\u{2026} 2 more"));
    }

    #[test]
    fn f_help_01_tiny_r7_and_r6() {
        let p = layout("Diff view", &diff_l1(), true, false, false, 80, 7);
        assert_eq!((p.h, p.more), (4, Some(14)));
        assert!(p.hint.is_none() && !p.credit);
        let p = layout("Diff view", &diff_l1(), true, false, false, 80, 6);
        assert_eq!((p.h, p.more), (3, None));
    }

    #[test]
    fn f_help_01_l2_context_without_motions_shows_close_hint() {
        let ents = [e("Dialogs", "y/Enter", "confirm")];
        let p = layout("Confirm", &ents, false, true, false, 80, 24);
        assert_eq!(p.hint, Some(" ? close"));
    }

    #[test]
    fn wrap_breaks_at_spaces_and_hard_cuts() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap("x", 3), vec!["x"]);
    }

    #[test]
    fn origin_never_above_row_three() {
        assert_eq!(origin(80, 24, 40, 30), (40, 2));
        assert_eq!(origin(20, 24, 40, 5), (0, 18));
    }
}
