//! Theme ids and chrome palette.
//!
//! Spec: F-THEME-01/02, Colors table, F-CFGUI-01 (choices), F-CLI-01 (names). Owner: core lead.
//! Must not: know about terminals. Palette values come from the SPEC Colors table only.

use crate::screen::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeId {
    #[default]
    Solarized,
    Vibrant,
    Dull,
    Contrast,
    Colorblind,
    Light,
}

impl ThemeId {
    /// Cycle order for `t` and config choices.
    pub const ALL: [ThemeId; 6] = [
        ThemeId::Solarized,
        ThemeId::Vibrant,
        ThemeId::Dull,
        ThemeId::Contrast,
        ThemeId::Colorblind,
        ThemeId::Light,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ThemeId::Solarized => "solarized",
            ThemeId::Vibrant => "vibrant",
            ThemeId::Dull => "dull",
            ThemeId::Contrast => "contrast",
            ThemeId::Colorblind => "colorblind",
            ThemeId::Light => "light",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// Next theme, wraps.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Chrome colors per theme (SPEC Colors: one field per row of the table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub add_bg: Color,
    pub del_bg: Color,
    pub add_mark: Color,
    pub del_mark: Color,
    pub gutter: Color,
    pub hunk: Color,
    pub mode: Color,
    pub view: Color,
    pub file: Color,
    pub adds: Color,
    pub dels: Color,
    pub dim: Color,
    pub accent: Color,
    pub modal_border: Color,
    pub modal_bg: Color,
    pub modal_fg: Color,
    pub sel_bg: Color,
    pub sel_fg: Color,
    pub cur_bg: Color,
    pub vis_bg: Color,
    pub vis_fg: Color,
}

impl Theme {
    /// Palette for `id`, from the SPEC Colors table.
    pub fn of(id: ThemeId) -> Theme {
        match id {
            ThemeId::Vibrant => Theme {
                add_bg: Color::Rgb(0x1f, 0x4d, 0x2b),
                del_bg: Color::Rgb(0x5a, 0x1f, 0x26),
                add_mark: Color::GreenBright,
                del_mark: Color::RedBright,
                gutter: Color::Gray,
                hunk: Color::Cyan,
                mode: Color::Yellow,
                view: Color::Magenta,
                file: Color::Cyan,
                adds: Color::Green,
                dels: Color::Red,
                dim: Color::Gray,
                accent: Color::Cyan,
                modal_border: Color::Cyan,
                modal_bg: Color::Black,
                modal_fg: Color::Rgb(0xe4, 0xe4, 0xe4),
                sel_bg: Color::Cyan,
                sel_fg: Color::Black,
                cur_bg: Color::Rgb(0x33, 0x33, 0x6b),
                vis_bg: Color::Rgb(0x87, 0x5f, 0x00),
                vis_fg: Color::Rgb(0xff, 0xff, 0xff),
            },
            ThemeId::Dull => Theme {
                add_bg: Color::Rgb(0x26, 0x33, 0x2a),
                del_bg: Color::Rgb(0x38, 0x25, 0x28),
                add_mark: Color::Rgb(0x7f, 0x9c, 0x7f),
                del_mark: Color::Rgb(0xa8, 0x7f, 0x7f),
                gutter: Color::Rgb(0x5f, 0x63, 0x68),
                hunk: Color::Rgb(0x7f, 0x9f, 0xa8),
                mode: Color::Rgb(0xb3, 0x9f, 0x80),
                view: Color::Rgb(0xa8, 0x89, 0x9c),
                file: Color::Rgb(0x9a, 0xa5, 0xb1),
                adds: Color::Rgb(0x7f, 0x9c, 0x7f),
                dels: Color::Rgb(0xa8, 0x7f, 0x7f),
                dim: Color::Rgb(0x5f, 0x63, 0x68),
                accent: Color::Rgb(0x7f, 0x9f, 0xa8),
                modal_border: Color::Rgb(0x5f, 0x63, 0x68),
                modal_bg: Color::Rgb(0x1c, 0x1c, 0x1c),
                modal_fg: Color::Rgb(0xc0, 0xc0, 0xc0),
                sel_bg: Color::Rgb(0x3a, 0x3f, 0x47),
                sel_fg: Color::Rgb(0xd0, 0xd0, 0xd0),
                cur_bg: Color::Rgb(0x3f, 0x3f, 0x5f),
                vis_bg: Color::Rgb(0x6b, 0x5a, 0x2e),
                vis_fg: Color::Rgb(0xf0, 0xf0, 0xf0),
            },
            ThemeId::Contrast => Theme {
                add_bg: Color::Rgb(0x00, 0x5f, 0x00),
                del_bg: Color::Rgb(0x87, 0x00, 0x00),
                add_mark: Color::Rgb(0xff, 0xff, 0xff),
                del_mark: Color::Rgb(0xff, 0xff, 0xff),
                gutter: Color::Rgb(0x80, 0x80, 0x80),
                hunk: Color::Rgb(0xff, 0xff, 0xff),
                mode: Color::Rgb(0xff, 0xff, 0x00),
                view: Color::Rgb(0xff, 0x00, 0xff),
                file: Color::Rgb(0x00, 0xff, 0xff),
                adds: Color::Rgb(0x00, 0xff, 0x00),
                dels: Color::Rgb(0xff, 0x00, 0x00),
                dim: Color::Rgb(0x80, 0x80, 0x80),
                accent: Color::Rgb(0xff, 0xff, 0x00),
                modal_border: Color::Rgb(0xff, 0xff, 0xff),
                modal_bg: Color::Rgb(0x00, 0x00, 0x00),
                modal_fg: Color::Rgb(0xff, 0xff, 0xff),
                sel_bg: Color::Rgb(0xff, 0xff, 0x00),
                sel_fg: Color::Rgb(0x00, 0x00, 0x00),
                cur_bg: Color::Rgb(0x3a, 0x3a, 0xa8),
                vis_bg: Color::Rgb(0xaf, 0x5f, 0x00),
                vis_fg: Color::Rgb(0xff, 0xff, 0xff),
            },
            ThemeId::Colorblind => Theme {
                add_bg: Color::Rgb(0x12, 0x34, 0x5a),
                del_bg: Color::Rgb(0x5a, 0x34, 0x10),
                add_mark: Color::Rgb(0x5f, 0xaf, 0xff),
                del_mark: Color::Rgb(0xff, 0xaf, 0x3f),
                gutter: Color::Rgb(0x80, 0x80, 0x80),
                hunk: Color::Rgb(0x5f, 0xaf, 0xff),
                mode: Color::Rgb(0xf0, 0xc6, 0x74),
                view: Color::Rgb(0xb4, 0x8e, 0xad),
                file: Color::Rgb(0x56, 0xb6, 0xf7),
                adds: Color::Rgb(0x5f, 0xaf, 0xff),
                dels: Color::Rgb(0xff, 0xaf, 0x3f),
                dim: Color::Rgb(0x8a, 0x8a, 0x8a),
                accent: Color::Rgb(0x56, 0xb6, 0xf7),
                modal_border: Color::Rgb(0x56, 0xb6, 0xf7),
                modal_bg: Color::Rgb(0x00, 0x00, 0x00),
                modal_fg: Color::Rgb(0xe0, 0xe0, 0xe0),
                sel_bg: Color::Rgb(0x56, 0xb6, 0xf7),
                sel_fg: Color::Rgb(0x00, 0x00, 0x00),
                cur_bg: Color::Rgb(0x5a, 0x5a, 0x5a),
                vis_bg: Color::Rgb(0xb8, 0xa0, 0x00),
                vis_fg: Color::Rgb(0x00, 0x00, 0x00),
            },
            ThemeId::Light => Theme {
                add_bg: Color::Rgb(0xd4, 0xf0, 0xd4),
                del_bg: Color::Rgb(0xf8, 0xd4, 0xd4),
                add_mark: Color::Rgb(0x0a, 0x6b, 0x1f),
                del_mark: Color::Rgb(0xa0, 0x10, 0x10),
                gutter: Color::Rgb(0x6a, 0x6a, 0x6a),
                hunk: Color::Rgb(0x00, 0x60, 0x9c),
                mode: Color::Rgb(0x8a, 0x5a, 0x00),
                view: Color::Rgb(0x8f, 0x1f, 0x8f),
                file: Color::Rgb(0x00, 0x60, 0x9c),
                adds: Color::Rgb(0x0a, 0x6b, 0x1f),
                dels: Color::Rgb(0xa0, 0x10, 0x10),
                dim: Color::Rgb(0x6e, 0x6e, 0x6e),
                accent: Color::Rgb(0x00, 0x60, 0x9c),
                modal_border: Color::Rgb(0x30, 0x30, 0x30),
                modal_bg: Color::Rgb(0xf4, 0xf4, 0xf4),
                modal_fg: Color::Rgb(0x20, 0x20, 0x20),
                sel_bg: Color::Rgb(0xbc, 0xd8, 0xff),
                sel_fg: Color::Rgb(0x10, 0x10, 0x10),
                cur_bg: Color::Rgb(0xff, 0xe9, 0xa0),
                vis_bg: Color::Rgb(0x7f, 0xb2, 0xff),
                vis_fg: Color::Rgb(0x00, 0x00, 0x00),
            },
            ThemeId::Solarized => Theme {
                add_bg: Color::Rgb(0x0b, 0x3b, 0x1f),
                del_bg: Color::Rgb(0x4a, 0x1a, 0x1f),
                add_mark: Color::Rgb(0x85, 0x99, 0x00),
                del_mark: Color::Rgb(0xdc, 0x32, 0x2f),
                gutter: Color::Rgb(0x58, 0x6e, 0x75),
                hunk: Color::Rgb(0x2a, 0xa1, 0x98),
                mode: Color::Rgb(0xb5, 0x89, 0x00),
                view: Color::Rgb(0x6c, 0x71, 0xc4),
                file: Color::Rgb(0x26, 0x8b, 0xd2),
                adds: Color::Rgb(0x85, 0x99, 0x00),
                dels: Color::Rgb(0xdc, 0x32, 0x2f),
                dim: Color::Rgb(0x58, 0x6e, 0x75),
                accent: Color::Rgb(0xcb, 0x4b, 0x16),
                modal_border: Color::Rgb(0x26, 0x8b, 0xd2),
                modal_bg: Color::Rgb(0x00, 0x2b, 0x36),
                modal_fg: Color::Rgb(0x93, 0xa1, 0xa1),
                sel_bg: Color::Rgb(0x07, 0x36, 0x42),
                sel_fg: Color::Rgb(0x93, 0xa1, 0xa1),
                cur_bg: Color::Rgb(0x22, 0x58, 0x6b),
                vis_bg: Color::Rgb(0x6b, 0x4f, 0x00),
                vis_fg: Color::Rgb(0xfd, 0xf6, 0xe3),
            },
        }
    }
}

/// Theme-independent find hit colors (F-FIND-02): bg yellow, fg black.
pub const FIND_HIT_BG: Color = Color::Yellow;
pub const FIND_HIT_FG: Color = Color::Black;

/// Syntax token classes the highlighter maps onto (see `src/theme.ts` `syntax`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxClass {
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
    Attribute,
    Literal,
    Meta,
    Punctuation,
}

/// Color of a syntax class in a theme, and whether syntax tokens are bold (F-THEME-02, UNSPEC-31).
/// `None` = terminal default (punctuation has no token color in the oracle).
pub fn syntax_color(id: ThemeId, class: SyntaxClass) -> Option<Color> {
    let p: [Color; 6] = match id {
        ThemeId::Vibrant => [
            Color::Rgb(0xff, 0x5f, 0xd7),
            Color::Rgb(0x87, 0xff, 0x5f),
            Color::Rgb(0xff, 0xaf, 0x00),
            Color::Rgb(0x00, 0xd7, 0xff),
            Color::Rgb(0x5f, 0xaf, 0xff),
            Color::Rgb(0x7a, 0x7a, 0x9a),
        ],
        ThemeId::Dull => [
            Color::Rgb(0xa8, 0x89, 0x9c),
            Color::Rgb(0x8f, 0xa3, 0x8a),
            Color::Rgb(0xb3, 0x9f, 0x80),
            Color::Rgb(0x7f, 0x9f, 0xa8),
            Color::Rgb(0x8a, 0x9b, 0xb0),
            Color::Rgb(0x5f, 0x63, 0x68),
        ],
        ThemeId::Contrast => [
            Color::Rgb(0xff, 0x00, 0xff),
            Color::Rgb(0x00, 0xb8, 0x00),
            Color::Rgb(0xff, 0x87, 0x00),
            Color::Rgb(0x00, 0x87, 0xff),
            Color::Rgb(0x00, 0xaf, 0xaf),
            Color::Rgb(0x80, 0x80, 0x80),
        ],
        ThemeId::Colorblind => [
            Color::Rgb(0xb4, 0x8e, 0xad),
            Color::Rgb(0xf0, 0xc6, 0x74),
            Color::Rgb(0xff, 0x9f, 0x43),
            Color::Rgb(0x56, 0xb6, 0xf7),
            Color::Rgb(0x8a, 0xb4, 0xf8),
            Color::Rgb(0x8a, 0x8a, 0x8a),
        ],
        ThemeId::Light => [
            Color::Rgb(0x8f, 0x1f, 0x8f),
            Color::Rgb(0x0a, 0x6b, 0x1f),
            Color::Rgb(0xa3, 0x4a, 0x00),
            Color::Rgb(0x00, 0x60, 0x9c),
            Color::Rgb(0x1f, 0x3f, 0x9f),
            Color::Rgb(0x6a, 0x6a, 0x6a),
        ],
        ThemeId::Solarized => [
            Color::Rgb(0x85, 0x99, 0x00),
            Color::Rgb(0x2a, 0xa1, 0x98),
            Color::Rgb(0xd3, 0x36, 0x82),
            Color::Rgb(0xb5, 0x89, 0x00),
            Color::Rgb(0x26, 0x8b, 0xd2),
            Color::Rgb(0x58, 0x6e, 0x75),
        ],
    };
    // keyword, string, number-ish, type, function, comment-ish (src/theme.ts syntax maps)
    let i = match class {
        SyntaxClass::Keyword => 0,
        SyntaxClass::String => 1,
        SyntaxClass::Number | SyntaxClass::Literal | SyntaxClass::Attribute => 2,
        SyntaxClass::Type => 3,
        SyntaxClass::Function => 4,
        SyntaxClass::Comment | SyntaxClass::Meta => 5,
        SyntaxClass::Punctuation => return None,
    };
    Some(p[i])
}

pub fn syntax_bold(id: ThemeId) -> bool {
    id == ThemeId::Contrast
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_theme_02_solarized_pins() {
        let t = Theme::of(ThemeId::Solarized);
        assert_eq!(t.add_bg, Color::Rgb(0x0b, 0x3b, 0x1f));
        assert_eq!(t.del_bg, Color::Rgb(0x4a, 0x1a, 0x1f));
        assert_eq!(t.cur_bg, Color::Rgb(0x22, 0x58, 0x6b));
        assert_eq!((t.sel_bg, t.sel_fg), (Color::Rgb(0x07, 0x36, 0x42), Color::Rgb(0x93, 0xa1, 0xa1)));
        assert_eq!(t.mode, Color::Rgb(0xb5, 0x89, 0x00));
    }

    #[test]
    fn f_theme_02_vibrant_pins() {
        let t = Theme::of(ThemeId::Vibrant);
        assert_eq!(t.add_bg, Color::Rgb(0x1f, 0x4d, 0x2b));
        assert_eq!(t.add_mark, Color::GreenBright);
        assert_eq!(t.del_mark, Color::RedBright);
        assert_eq!(t.gutter, Color::Gray);
        assert_eq!(t.mode, Color::Yellow);
        assert_eq!((t.sel_bg, t.sel_fg), (Color::Cyan, Color::Black));
        assert_eq!(t.cur_bg, Color::Rgb(0x33, 0x33, 0x6b));
    }

    #[test]
    fn f_theme_02_dull_pins() {
        let t = Theme::of(ThemeId::Dull);
        assert_eq!(t.del_bg, Color::Rgb(0x38, 0x25, 0x28));
        assert_eq!(t.cur_bg, Color::Rgb(0x3f, 0x3f, 0x5f));
        assert_eq!((t.sel_bg, t.sel_fg), (Color::Rgb(0x3a, 0x3f, 0x47), Color::Rgb(0xd0, 0xd0, 0xd0)));
    }

    #[test]
    fn f_theme_02_contrast_pins() {
        let t = Theme::of(ThemeId::Contrast);
        assert_eq!(t.add_bg, Color::Rgb(0x00, 0x5f, 0x00));
        assert_eq!(t.del_bg, Color::Rgb(0x87, 0x00, 0x00));
        assert_eq!(t.cur_bg, Color::Rgb(0x3a, 0x3a, 0xa8));
        assert_eq!((t.sel_bg, t.sel_fg), (Color::Rgb(0xff, 0xff, 0x00), Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn f_theme_02_colorblind_pins() {
        let t = Theme::of(ThemeId::Colorblind);
        assert_eq!(t.add_bg, Color::Rgb(0x12, 0x34, 0x5a));
        assert_eq!(t.del_bg, Color::Rgb(0x5a, 0x34, 0x10));
        assert_eq!(t.cur_bg, Color::Rgb(0x5a, 0x5a, 0x5a));
        assert_eq!((t.sel_bg, t.sel_fg), (Color::Rgb(0x56, 0xb6, 0xf7), Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn f_theme_02_light_pins() {
        let t = Theme::of(ThemeId::Light);
        assert_eq!(t.add_bg, Color::Rgb(0xd4, 0xf0, 0xd4));
        assert_eq!(t.del_bg, Color::Rgb(0xf8, 0xd4, 0xd4));
        assert_eq!(t.cur_bg, Color::Rgb(0xff, 0xe9, 0xa0));
        assert_eq!((t.sel_bg, t.sel_fg), (Color::Rgb(0xbc, 0xd8, 0xff), Color::Rgb(0x10, 0x10, 0x10)));
        assert_eq!(t.modal_bg, Color::Rgb(0xf4, 0xf4, 0xf4));
    }

    #[test]
    fn f_theme_02_syntax() {
        assert_eq!(
            syntax_color(ThemeId::Solarized, SyntaxClass::Keyword),
            Some(Color::Rgb(0x85, 0x99, 0x00))
        );
        assert_eq!(
            syntax_color(ThemeId::Solarized, SyntaxClass::Comment),
            Some(Color::Rgb(0x58, 0x6e, 0x75))
        );
        assert_eq!(
            syntax_color(ThemeId::Vibrant, SyntaxClass::Attribute),
            Some(Color::Rgb(0xff, 0xaf, 0x00))
        );
        assert_eq!(syntax_color(ThemeId::Light, SyntaxClass::Punctuation), None);
        assert!(syntax_bold(ThemeId::Contrast));
        assert!(!syntax_bold(ThemeId::Dull));
    }

    #[test]
    fn f_theme_01_cycle() {
        assert_eq!(ThemeId::Light.next(), ThemeId::Solarized);
        assert_eq!(ThemeId::parse("dull"), Some(ThemeId::Dull));
    }
}
