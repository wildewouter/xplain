//! Delete / quit dialogs (F-COMMENT-08, F-QUIT-01).

use super::*;

/// Delete / quit dialog: one text row, one cell padding each side (F-COMMENT-08, F-QUIT-01).
pub(super) fn dialog(c: &mut Canvas, lk: &Look, size: Size, help_open: bool, text: &str) {
    let w = crate::textutil::cell_width(text) as u16 + 4;
    let (x, y) = place(size, w, 3, help_open);
    let inner = open_box(c, lk, x, y, w, 3);
    c.put(inner.x + 1, inner.y, text, lk.fill);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::modals::fixtures::*;
    use crate::view::testutil::{state, theme};

    #[test]
    fn f_layout_06_delete_modal_25x3_at_80x24() {
        let mut st = state(80, 24);
        st.overlay = Overlay::DeleteComment { id: "c1".into() };
        let mut c = canvas(80, 24);
        draw(&mut c, &st, &theme());
        let s = c.into_screen();
        assert_eq!(
            cols_text(&s, 11, 28, 52),
            "\u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}"
        );
        assert_eq!(cols_text(&s, 12, 28, 52), "\u{2502} Delete comment? (y/n) \u{2502}");
        assert_eq!(s.row_text(11).chars().take(28).collect::<String>().trim(), "");
        assert_eq!(s.row_text(14).trim(), "");
        assert!(s.row_text(13).trim().starts_with('\u{2570}'));
    }

    #[test]
    fn f_quit_01_dialog_text_and_size() {
        let mut st = state(80, 24);
        st.overlay = Overlay::Quit;
        let mut c = canvas(80, 24);
        draw(&mut c, &st, &theme());
        let s = c.into_screen();
        // width 22: left col ceil(58/2) = 29
        assert_eq!(cols_text(&s, 12, 29, 50), "\u{2502} Quit xplain? (y/n) \u{2502}");
        let th = theme();
        let x = cell_x(&s, 12, "Quit");
        assert_eq!(s.rows[12][x].style.fg, Some(th.modal_fg));
        assert_eq!(s.rows[12][x].style.bg, Some(th.modal_bg));
        assert_eq!(s.rows[12][29].style.fg, Some(th.modal_border));
    }

    #[test]
    fn f_layout_06_help_open_moves_modal_to_row_three() {
        let mut st = state(80, 24);
        st.overlay = Overlay::Quit;
        st.help = HelpLevel::L1;
        let (x, y) = place(Size { cols: 80, rows: 24 }, 22, 3, true);
        assert_eq!((x, y), (29, 2));
        let _ = &mut st;
    }
}
