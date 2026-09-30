//! xplain-sim: in-process scenario test harness (dev only, `publish = false`).
//!
//! Drives `xplain-core` (`update`/`view`) with the real integration registry, a manual clock and real git and
//! files in temp dirs, without a terminal, sockets or sleeps. See `README.md` for the API and its limits.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod exec;
pub mod fixture;
pub mod http;
pub mod keys;
pub mod shim;
pub mod sim;
pub mod view;

pub use fixture::Fixture;
pub use http::{Http, HttpReply, Pending};
pub use shim::{Call, Rule};
pub use sim::{Sim, SimBuilder};
pub use view::{CellExpect, CellView, Pos};
pub use xplain_core::integration::CommandError;
