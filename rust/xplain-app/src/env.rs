//! Process environment snapshot and derived paths.
//!
//! Spec: F-CONFIG-01 (inputs to path resolution; empty = unset), Contract surface env list,
//! F-MCPSRV-01 (state dir: `$XDG_STATE_HOME/xplain`, else `$HOME/.local/state/xplain`; empty = unset),
//! Test seams (`XPLAIN_MCP_PORT` raw, `XPLAIN_SYNC=1`), Colors (`COLORTERM=truecolor`).
//! Owner: component B (startup).
//! Must not: parse the port (core does), read config files, or print. `RawEnv::from_process` is the only
//! place that reads `std::env`; everything else takes a `RawEnv` so it is unit-testable.

use xplain_core::config::ConfigPathEnv;
use xplain_core::options::EnvInfo;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawEnv {
    pub home: Option<String>,
    pub xdg_config_home: Option<String>,
    pub xdg_state_home: Option<String>,
    pub xplain_config: Option<String>,
    pub xplain_mcp_port: Option<String>,
    pub xplain_sync: Option<String>,
    pub colorterm: Option<String>,
}

impl RawEnv {
    /// Read the process environment (values kept raw, empty strings preserved).
    pub fn from_process() -> RawEnv {
        todo!("std::env::var_os lossy")
    }

    /// Inputs for `xplain_core::config::resolve_config_path` with the `--config` flag value.
    pub fn config_path_env(&self, _flag: Option<&str>) -> ConfigPathEnv {
        todo!("F-CONFIG-01")
    }

    /// `<state dir>` for `mcp.json` (F-MCPSRV-01). Empty `XDG_STATE_HOME` counts as unset.
    pub fn state_dir(&self) -> String {
        todo!("F-MCPSRV-01")
    }

    /// `XPLAIN_SYNC` set to `1`.
    pub fn sync(&self) -> bool {
        todo!("XPLAIN_SYNC=1")
    }

    /// 24-bit colors wanted (`COLORTERM` truecolor or 24bit).
    pub fn truecolor(&self) -> bool {
        todo!("Colors")
    }

    /// Build `EnvInfo` (abs cwd already resolved: process cwd joined with `--cwd`, or process cwd).
    pub fn env_info(&self, _abs_cwd: String, _config_path: String) -> EnvInfo {
        todo!("EnvInfo")
    }
}
