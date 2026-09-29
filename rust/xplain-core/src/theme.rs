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
    /// Palette for `id`. Implemented by the theme worker from the SPEC table.
    pub fn of(_id: ThemeId) -> Theme {
        todo!("theme palette table")
    }
}

/// Theme-independent find hit colors (F-FIND-02): bg yellow, fg black.
pub const FIND_HIT_BG: Color = Color::Yellow;
pub const FIND_HIT_FG: Color = Color::Black;
