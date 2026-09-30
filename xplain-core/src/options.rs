//! Resolved startup inputs: CLI options, environment snapshot, and the `Init` bundle for `State::new`.
//!
//! Spec: F-CLI-03, F-CLI-06, F-CONFIG-01 (path resolution order), F-MODE-01 (mode), Environment.
//! Owner: core lead. Argv *parsing* lives in `xplain-app::cli`; this file holds only the result types.
//! Must not: read env vars or files (the runtime fills [`EnvInfo`]).

use crate::config::Config;
use crate::screen::Size;
use crate::theme::ThemeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DiffMode {
    #[default]
    All,
    Staged,
    Unstaged,
}

impl DiffMode {
    pub const ALL: [DiffMode; 3] = [DiffMode::All, DiffMode::Staged, DiffMode::Unstaged];
    pub fn as_str(self) -> &'static str {
        match self {
            DiffMode::All => "all",
            DiffMode::Staged => "staged",
            DiffMode::Unstaged => "unstaged",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
    /// `m` key order: all, staged, unstaged, all.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Result of successful argv parsing (flags override config, F-CLI-03).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    /// `--cwd` verbatim; `None` = process cwd.
    pub cwd: Option<String>,
    /// `--config` verbatim; empty already mapped to `None`.
    pub config_path: Option<String>,
    pub mode: Option<DiffMode>,
    pub split: Option<bool>,
    /// `Some(false)` when `--changes-only`.
    pub full: Option<bool>,
    pub theme: Option<ThemeId>,
    /// Everything not a flag, in order (F-CLI-03).
    pub git_args: Vec<String>,
}

/// Snapshot of process environment, filled once by the runtime.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvInfo {
    /// Absolute cwd used for export header (`repo:`), export file location and `cannot read <cwd>/<p>`.
    pub abs_cwd: String,
    /// Raw `XPLAIN_MCP_PORT` (`None` = unset or empty). Core validates (Environment).
    pub mcp_port_raw: Option<String>,
    /// `<state dir>` for `mcp.json` (F-MCPSRV-01), already resolved by the runtime.
    pub state_dir: String,
    /// Resolved config path (F-CONFIG-01), needed for saves and config notes.
    pub config_path: String,
}

/// Everything `State::new` needs.
#[derive(Debug, Clone)]
pub struct Init {
    pub options: Options,
    /// Config after validation (warnings already printed by the runtime).
    pub config: Config,
    pub env: EnvInfo,
    pub size: Size,
}
