//! Config model, path resolution, parse/validate (exact warning strings), save-patch merge.
//!
//! Spec: F-CONFIG-01..05, F-CFGUI-03 (save errors). Owner: core lead (config component).
//! Must not: touch the filesystem or env. The runtime reads the file / env and calls these fns, and
//! executes `Effect::SaveConfig` by calling [`apply_patch`] around its own read/write.

use crate::errors::IoReason;
use crate::options::DiffMode;
use crate::theme::ThemeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub theme: ThemeId,
    pub mode: DiffMode,
    pub split: bool,
    /// true = full-file scope.
    pub full: bool,
    pub confirm_quit: bool,
    pub mcp_autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: ThemeId::Solarized,
            mode: DiffMode::All,
            split: false,
            full: true,
            confirm_quit: true,
            mcp_autostart: false,
        }
    }
}

/// Result of loading: config plus complete stderr lines (no trailing newline), each already starting
/// with `xplain: config: `. The runtime prints them in order before the UI starts (F-CLI-05).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigLoad {
    pub config: Config,
    pub warnings: Vec<String>,
}

/// What the runtime found when reading the config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigFile {
    Missing,
    Text(String),
    /// Exists but unreadable (e.g. is a directory). Text between prefix and suffix is UNSPEC-29.
    Unreadable(IoReason),
}

/// Inputs to path resolution (F-CONFIG-01). Empty strings must already be treated as unset by the
/// resolver itself; the runtime passes raw values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigPathEnv {
    pub flag: Option<String>,
    pub xplain_config: Option<String>,
    pub xdg_config_home: Option<String>,
    pub home: Option<String>,
}

/// F-CONFIG-01 order: flag, `$XPLAIN_CONFIG`, `$XDG_CONFIG_HOME/xplain/config.json`, `$HOME/.config/xplain/config.json`.
pub fn resolve_config_path(_env: &ConfigPathEnv) -> String {
    todo!("F-CONFIG-01")
}

/// F-CONFIG-02..04: validate `file` read from `path`, return config and warning lines.
pub fn load_config(_path: &str, _file: &ConfigFile) -> ConfigLoad {
    todo!("F-CONFIG-02..04")
}

/// One setting change from the config modal (F-CONFIG-05 patch table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigChange {
    Theme(ThemeId),
    Mode(DiffMode),
    Split(bool),
    Full(bool),
    ConfirmQuit(bool),
    McpAutostart(bool),
}

impl ConfigChange {
    /// JSON merge patch per F-CONFIG-05, e.g. `Mode(All)` -> `{"view":{"mode":"all"}}`.
    pub fn to_patch(&self) -> serde_json::Value {
        todo!("F-CONFIG-05 patch table")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSaveError {
    /// Existing file does not parse to a JSON object: note `config unreadable, not saved (<path>)`.
    Unreadable,
    /// Write failed: note `config save failed: <path>: <reason>`.
    Io(IoReason),
}

/// F-CONFIG-05: deep-merge `patch` into `existing` (`None` = missing file, base `{"version": 1}`),
/// serialize with tab indent + trailing newline. Pure; the runtime does the read/mkdir/write.
pub fn apply_patch(_existing: Option<&str>, _patch: &serde_json::Value) -> Result<String, ConfigSaveError> {
    todo!("F-CONFIG-05")
}
