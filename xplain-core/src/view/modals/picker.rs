//! Picker and search modals (F-FILES-01, F-SEARCH-01).

use super::*;
use crate::diff::Status;
use crate::picker::PickerEntry;
use crate::view::layout::list_width;

/// Picker box (x, y, w, h) for `n` files (F-FILES-01).
pub(super) fn picker_geometry(size: Size, n: usize, help_open: bool) -> (u16, u16, u16, u16) {
    let rows_t = frame_height(size);
    let w = list_width(size.cols);
    let h = rows_t.min(5.max((n.min(usize::from(u16::MAX)) as u16).saturating_add(4).min(rows_t * 6 / 10)));
    let (x, y) = place(size, w, h, help_open);
    (x, y, w, h)
}

/// List window start: selection centered, clamped to the ends (F-FILES-01).
pub(super) fn window_start(sel: usize, total: usize, vis: usize) -> usize {
    sel.saturating_sub(vis / 2).min(total.saturating_sub(vis))
}

/// F-FILES-01.
pub(super) fn draw_picker(
    c: &mut Canvas,
    lk: &Look,
    entries: &[PickerEntry],
    sel: usize,
    current: usize,
    r: Rect,
) {
    let inner = open_box(c, lk, r.x, r.y, r.w, r.h);
    let vis = usize::from(r.h.saturating_sub(4)).max(1);
    line(c, inner, 0, &[seg(format!(" Files ({}/{})", sel + 1, entries.len()), bold(lk.fill))]);
    let start = window_start(sel, entries.len(), vis);
    for (k, e) in entries.iter().enumerate().skip(start).take(vis) {
        let on = k == sel;
        let base = lk.row(on);
        let (letter, adds, dels) = if on {
            (base, base, base)
        } else {
            let letter = match e.status {
                Status::Added => lk.theme.adds,
                Status::Deleted => lk.theme.dels,
                Status::Renamed => lk.theme.mode,
                Status::Modified => lk.theme.accent,
            };
            (
                Style { fg: Some(letter), ..base },
                Style { fg: Some(lk.theme.adds), ..base },
                Style { fg: Some(lk.theme.dels), ..base },
            )
        };
        let mut segs = vec![
            seg(if on { ">" } else { " " }, base),
            seg(e.status.letter().to_string(), letter),
            seg(" ", base),
            seg(e.label.clone(), base),
            seg(format!(" +{}", e.adds), adds),
            seg(format!(" -{}", e.dels), dels),
        ];
        if k == current {
            segs.push(seg(" *", base));
        }
        line(c, inner, 1 + (k - start) as u16, &segs);
    }
    line(
        c,
        inner,
        1 + vis as u16,
        &[seg(" j/k/\u{2191}\u{2193} move d/u half page enter open esc/q close", lk.dim)],
    );
}

/// F-SEARCH-01. `hits`: (path, matched char positions).
pub(super) fn draw_search(
    c: &mut Canvas,
    lk: &Look,
    query: &str,
    hits: &[(String, Vec<usize>)],
    sel: usize,
    r: Rect,
) {
    let inner = open_box(c, lk, r.x, r.y, r.w, r.h);
    let vis = usize::from(r.h.saturating_sub(5)).max(1);
    let shown = if hits.is_empty() { 0 } else { sel + 1 };
    line(c, inner, 0, &[seg(format!(" Search ({shown}/{})", hits.len()), bold(lk.fill))]);
    line(
        c,
        inner,
        1,
        &[
            seg(" ", lk.fill),
            seg("> ", lk.accent),
            seg(query, lk.fill),
            seg(" ", Style { reverse: true, ..lk.fill }),
        ],
    );
    let start = window_start(sel, hits.len(), vis);
    for (k, (path, idx)) in hits.iter().enumerate().skip(start).take(vis) {
        let on = k == sel;
        let base = lk.row(on);
        let mut segs = vec![seg(if on { "> " } else { "  " }, base)];
        for (j, ch) in path.chars().enumerate() {
            let hit = idx.contains(&j);
            let st = match (on, hit) {
                (true, true) => bold(base),
                (true, false) => base,
                (false, true) => bold(lk.accent),
                (false, false) => lk.fill,
            };
            segs.push(seg(ch.to_string(), st));
        }
        line(c, inner, 2 + (k - start) as u16, &segs);
    }
    line(c, inner, 2 + vis as u16, &[seg(" \u{2191}\u{2193}/^n^p move enter open esc close", lk.dim)]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::modals::fixtures::*;
    use crate::view::testutil::theme;

    fn entry(status: Status, label: &str, adds: u32, dels: u32) -> PickerEntry {
        PickerEntry { status, label: label.to_string(), adds, dels }
    }

    #[test]
    fn f_files_01_geometry_80x24() {
        // width min(80, max(20, 56)) = 56; height min(24, max(5, min(3+4, 14))) = 7
        let (x, y, w, h) = picker_geometry(Size { cols: 80, rows: 24 }, 3, false);
        assert_eq!((x, y, w, h), (12, 9, 56, 7));
        let (_, _, _, h) = picker_geometry(Size { cols: 80, rows: 24 }, 0, false);
        assert_eq!(h, 5);
        let (_, y, _, h) = picker_geometry(Size { cols: 80, rows: 24 }, 40, false);
        assert_eq!((y, h), (5, 14));
    }

    #[test]
    fn f_files_01_rows_title_hint_colors() {
        let entries = vec![
            entry(Status::Modified, "a.txt", 3, 1),
            entry(Status::Renamed, "old.txt -> b.txt", 0, 0),
            entry(Status::Added, "c.txt", 5, 0),
        ];
        let lk = look();
        let mut c = canvas(80, 24);
        draw_picker(&mut c, &lk, &entries, 0, 1, Rect { x: 12, y: 8, w: 56, h: 7 });
        let s = c.into_screen();
        assert!(cols_text(&s, 9, 12, 67).starts_with("\u{2502} Files (1/3)  "));
        assert!(cols_text(&s, 10, 12, 67).starts_with("\u{2502}>M a.txt +3 -1"));
        assert!(cols_text(&s, 11, 12, 67).starts_with("\u{2502} R old.txt -> b.txt +0 -0 *"));
        assert!(cols_text(&s, 12, 12, 67).starts_with("\u{2502} A c.txt +5 -0"));
        assert!(
            cols_text(&s, 13, 12, 67)
                .contains(" j/k/\u{2191}\u{2193} move d/u half page enter open esc/q close")
        );
        let th = theme();
        // selected row: all cells selBg/selFg (status letter, +N, -N too)
        for t in [">M", "a.txt", "+3", "-1"] {
            let x = cell_x(&s, 10, t);
            assert_eq!(s.rows[10][x].style.bg, Some(th.sel_bg), "{t}");
            assert_eq!(s.rows[10][x].style.fg, Some(th.sel_fg), "{t}");
        }
        // others: own colors
        assert_eq!(s.rows[11][cell_x(&s, 11, "R old")].style.fg, Some(th.mode));
        assert_eq!(s.rows[12][cell_x(&s, 12, "A c")].style.fg, Some(th.adds));
        assert_eq!(s.rows[12][cell_x(&s, 12, "+5")].style.fg, Some(th.adds));
        assert_eq!(s.rows[12][cell_x(&s, 12, "-0")].style.fg, Some(th.dels));
        assert_eq!(s.rows[10][cell_x(&s, 10, "M a")].style.fg, Some(th.sel_fg));
        assert_eq!(s.rows[11][cell_x(&s, 11, "old.txt")].style.bg, Some(th.modal_bg));
        assert!(
            s.rows[13][cell_x(&s, 13, "j/k")].style.dim
                || s.rows[13][cell_x(&s, 13, "j/k")].style.fg == Some(th.dim)
        );
    }

    #[test]
    fn f_files_01_empty_list_title() {
        let mut c = canvas(80, 24);
        let (x, y, w, h) = picker_geometry(Size { cols: 80, rows: 24 }, 0, false);
        draw_picker(&mut c, &look(), &[], 0, 0, Rect { x, y, w, h });
        assert!(c.into_screen().row_text(usize::from(y) + 1).contains(" Files (1/0)"));
    }

    #[test]
    fn f_files_01_window_centered_and_clamped() {
        assert_eq!(window_start(0, 20, 5), 0);
        assert_eq!(window_start(10, 20, 5), 8);
        assert_eq!(window_start(19, 20, 5), 15);
        assert_eq!(window_start(2, 3, 5), 0);
    }

    fn hit(path: &str, idx: &[usize]) -> (String, Vec<usize>) {
        (path.to_string(), idx.to_vec())
    }

    #[test]
    fn f_search_01_layout_and_highlights() {
        let hits = vec![hit("README.md", &[0, 1]), hit("readme.md", &[0, 1])];
        let mut c = canvas(80, 24);
        draw_search(&mut c, &look(), "rm", &hits, 0, Rect { x: 12, y: 4, w: 56, h: 16 });
        let s = c.into_screen();
        let th = theme();
        assert!(cols_text(&s, 5, 12, 67).starts_with("\u{2502} Search (1/2)  "));
        assert!(cols_text(&s, 6, 12, 67).starts_with("\u{2502} > rm "));
        assert!(cols_text(&s, 7, 12, 67).starts_with("\u{2502}> README.md "));
        assert!(cols_text(&s, 8, 12, 67).starts_with("\u{2502}  readme.md "));
        assert!(
            cols_text(&s, 18, 12, 67)
                .starts_with("\u{2502} \u{2191}\u{2193}/^n^p move enter open esc close ")
        );
        assert_eq!(cols_text(&s, 19, 12, 67), format!("\u{2570}{}\u{256f}", "\u{2500}".repeat(54)));
        // query row: accent `> `, inverse caret
        let x = cell_x(&s, 6, "> rm");
        assert_eq!(s.rows[6][x].style.fg, Some(th.accent));
        assert!(s.rows[6][x + 4].style.reverse);
        // selected row: sel colors, matched chars bold
        let r = cell_x(&s, 7, "README");
        assert_eq!(s.rows[7][r].style.bg, Some(th.sel_bg));
        assert_eq!(s.rows[7][r].style.fg, Some(th.sel_fg));
        assert!(s.rows[7][r].style.bold);
        assert!(!s.rows[7][r + 2].style.bold);
        // unselected: matched accent bold, others modalFg
        let u = cell_x(&s, 8, "readme");
        assert_eq!(s.rows[8][u].style.fg, Some(th.accent));
        assert!(s.rows[8][u + 1].style.bold);
        assert_eq!(s.rows[8][u + 2].style.fg, Some(th.modal_fg));
        assert_eq!(s.rows[8][u + 2].style.bg, Some(th.modal_bg));
        assert!(!s.rows[8][u + 2].style.bold);
    }

    #[test]
    fn f_search_01_no_hits_title() {
        let mut c = canvas(80, 24);
        draw_search(&mut c, &look(), "zzz", &[], 0, Rect { x: 12, y: 4, w: 56, h: 16 });
        assert!(c.into_screen().row_text(5).contains(" Search (0/0)"));
    }

    #[test]
    fn f_search_01_window_height_minus_five() {
        let hits: Vec<_> = (0..30).map(|i| hit(&format!("f{i:02}"), &[])).collect();
        let mut c = canvas(80, 24);
        draw_search(&mut c, &look(), "", &hits, 15, Rect { x: 12, y: 4, w: 56, h: 16 });
        let s = c.into_screen();
        // vis 11, start = 15-5 = 10 -> f10..f20 on rows 7..17
        assert!(s.row_text(7).contains("f10"));
        assert!(s.row_text(12).contains("> f15"));
        assert!(s.row_text(17).contains("f20"));
    }
}
