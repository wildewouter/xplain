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

fn var(name: &str) -> Option<String> {
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

fn set(v: &Option<String>) -> Option<&str> {
    v.as_deref().filter(|s| !s.is_empty())
}

impl RawEnv {
    /// Read the process environment (values kept raw, empty strings preserved).
    pub fn from_process() -> RawEnv {
        RawEnv {
            home: var("HOME"),
            xdg_config_home: var("XDG_CONFIG_HOME"),
            xdg_state_home: var("XDG_STATE_HOME"),
            xplain_config: var("XPLAIN_CONFIG"),
            xplain_mcp_port: var("XPLAIN_MCP_PORT"),
            xplain_sync: var("XPLAIN_SYNC"),
            colorterm: var("COLORTERM"),
        }
    }

    /// Inputs for `xplain_core::config::resolve_config_path` with the `--config` flag value.
    pub fn config_path_env(&self, flag: Option<&str>) -> ConfigPathEnv {
        ConfigPathEnv {
            flag: flag.map(str::to_string),
            xplain_config: self.xplain_config.clone(),
            xdg_config_home: self.xdg_config_home.clone(),
            home: self.home.clone(),
        }
    }

    /// `<state dir>` for `mcp.json` (F-MCPSRV-01). Empty `XDG_STATE_HOME` counts as unset.
    pub fn state_dir(&self) -> String {
        match set(&self.xdg_state_home) {
            Some(x) => format!("{}/xplain", x.trim_end_matches('/')),
            None => format!("{}/.local/state/xplain", set(&self.home).unwrap_or("").trim_end_matches('/')),
        }
    }

    /// `XPLAIN_SYNC` set to `1`.
    pub fn sync(&self) -> bool {
        self.xplain_sync.as_deref() == Some("1")
    }

    /// 24-bit colors wanted (`COLORTERM` truecolor or 24bit).
    pub fn truecolor(&self) -> bool {
        matches!(self.colorterm.as_deref(), Some("truecolor" | "24bit"))
    }

    /// Build `EnvInfo` (abs cwd already resolved: process cwd joined with `--cwd`, or process cwd).
    pub fn env_info(&self, abs_cwd: String, config_path: String) -> EnvInfo {
        EnvInfo {
            abs_cwd,
            sync: self.sync(),
            mcp_port_raw: set(&self.xplain_mcp_port).map(str::to_string),
            state_dir: self.state_dir(),
            config_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> RawEnv {
        RawEnv { home: Some("/h".into()), ..RawEnv::default() }
    }

    #[test]
    fn state_dir_order() {
        assert_eq!(env().state_dir(), "/h/.local/state/xplain");
        let mut e = env();
        e.xdg_state_home = Some(String::new());
        assert_eq!(e.state_dir(), "/h/.local/state/xplain");
        e.xdg_state_home = Some("/s".into());
        assert_eq!(e.state_dir(), "/s/xplain");
    }

    #[test]
    fn flags() {
        let mut e = env();
        assert!(!e.sync() && !e.truecolor());
        e.xplain_sync = Some("1".into());
        e.colorterm = Some("truecolor".into());
        assert!(e.sync() && e.truecolor());
        e.xplain_sync = Some("0".into());
        e.colorterm = Some("24bit".into());
        assert!(!e.sync() && e.truecolor());
        e.colorterm = Some("yes".into());
        assert!(!e.truecolor());
    }

    #[test]
    fn config_env_passthrough() {
        let mut e = env();
        e.xplain_config = Some("/c".into());
        e.xdg_config_home = Some(String::new());
        let c = e.config_path_env(Some("/f"));
        assert_eq!(c.flag.as_deref(), Some("/f"));
        assert_eq!(c.xplain_config.as_deref(), Some("/c"));
        assert_eq!(c.xdg_config_home.as_deref(), Some(""));
        assert_eq!(c.home.as_deref(), Some("/h"));
        assert_eq!(e.config_path_env(None).flag, None);
    }

    #[test]
    fn env_info_fields() {
        let mut e = env();
        e.xplain_mcp_port = Some(String::new());
        e.xplain_sync = Some("1".into());
        let i = e.env_info("/w".into(), "/cfg".into());
        assert_eq!(i.abs_cwd, "/w");
        assert_eq!(i.config_path, "/cfg");
        assert_eq!(i.mcp_port_raw, None);
        assert!(i.sync);
        e.xplain_mcp_port = Some("abc".into());
        assert_eq!(e.env_info(String::new(), String::new()).mcp_port_raw.as_deref(), Some("abc"));
    }
}
