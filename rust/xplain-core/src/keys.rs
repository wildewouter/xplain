//! Terminal-independent key model. Produced by the runtime input decoder, consumed by core keymaps.
//!
//! Spec: F-NAV-07 (ctrl combos), F-CURSOR-05 (digits), Test seams (Escape = lone ESC before barrier;
//! barrier bytes are never keys and never reach core). Owner: core lead (types frozen at skeleton).
//! Must not: reference crossterm.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Tab,
    /// Shift+Tab. Runtime normalizes so `Tab` with shift never appears.
    BackTab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    /// Only meaningful for non-char keys. For `Char`, shift is already in the char (`J`, `?`).
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Mods,
}

impl KeyEvent {
    pub const fn plain(key: Key) -> Self {
        Self { key, mods: Mods { ctrl: false, alt: false, shift: false } }
    }
    pub const fn ch(c: char) -> Self {
        Self::plain(Key::Char(c))
    }
    pub const fn ctrl(c: char) -> Self {
        Self { key: Key::Char(c), mods: Mods { ctrl: true, alt: false, shift: false } }
    }
    pub fn is_ctrl_c(&self) -> bool {
        self.mods.ctrl && matches!(self.key, Key::Char('c') | Key::Char('C'))
    }
}
