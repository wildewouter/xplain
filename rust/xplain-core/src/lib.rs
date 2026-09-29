//! xplain-core: the pure heart of xplain.
//!
//! Elm architecture: `update(&mut State, Event) -> Vec<Effect>` and `view(&State) -> Screen`.
//! No IO, no async, no terminal crates, no clock, no randomness, no agent names.
//! Everything the outside world provides arrives as an [`Event`]; everything core wants done
//! leaves as an [`Effect`]. See `ARCHITECTURE.md` at the repo root.
//!
//! Owned spec: all behavior except process/terminal/socket plumbing (that is `xplain-app`) and
//! per-agent CLI knowledge (that is `xplain-integrations`).
//!
//! Crate leads: this file lists the boundary modules. Add inner modules under your own
//! sub-directories (`src/<module>/...`) and register them in that module's own `mod.rs`/file,
//! never here, so parallel workers do not conflict.

pub mod comments;
pub mod config;
pub mod diff;
pub mod effect;
pub mod errors;
pub mod event;
pub mod integration;
pub mod keys;
pub mod mcp;
pub mod options;
pub mod screen;
pub mod state;
pub mod theme;
pub mod update;
pub mod view;

pub use effect::Effect;
pub use event::Event;
pub use screen::Screen;
pub use state::State;
pub use update::update;
pub use view::view;
