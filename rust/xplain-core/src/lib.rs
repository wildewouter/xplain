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
//! Module tree and ownership: see `MODULES.md`. Every module is registered here (and `nav`, `mcp`,
//! `view` register their submodules up front), so workers never edit registries.

pub mod ask;
pub mod browse;
pub mod canvas;
mod comments;
pub mod config;
pub mod config_ui;
pub mod diff;
pub mod editor;
pub mod effect;
pub mod errors;
pub mod event;
pub mod export;
pub mod find;
pub mod fuzzy;
pub mod help;
mod highlight;
mod hlcache;
pub mod integration;
pub mod jump;
pub mod keys;
pub mod mcp;
pub mod mcp_ui;
pub mod messages;
pub mod nav;
pub mod options;
pub mod picker;
pub mod quit;
pub mod reload;
pub mod rows;
pub mod screen;
pub mod search;
pub mod state;
pub mod textinput;
pub mod textutil;
pub mod theme;
pub mod thread;
pub mod thread_layout;
pub mod update;
mod view;

pub use comments::PaneSide;
pub use effect::Effect;
pub use event::Event;
pub use highlight::highlight_lines;
pub use hlcache::HlKey;
pub use screen::Screen;
pub use state::State;
pub use update::update;
pub use view::view;
