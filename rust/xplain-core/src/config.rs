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
pub fn resolve_config_path(env: &ConfigPathEnv) -> String {
    let set = |v: &Option<String>| v.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(p) = set(&env.flag).or_else(|| set(&env.xplain_config)) {
        return p;
    }
    let base = match set(&env.xdg_config_home) {
        Some(x) => x,
        None => format!("{}/.config", env.home.as_deref().unwrap_or("")),
    };
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/xplain/config.json")
}

/// F-CONFIG-02..04: validate `file` read from `path`, return config and warning lines.
pub fn load_config(path: &str, file: &ConfigFile) -> ConfigLoad {
    let mut out = ConfigLoad::default();
    let broken = |out: &mut ConfigLoad, msg: &str| {
        out.warnings
            .push(format!("xplain: config: {path}: {msg}; using defaults, file will not be modified"));
    };
    let text = match file {
        ConfigFile::Missing => return out,
        ConfigFile::Unreadable(reason) => {
            broken(&mut out, reason.as_str());
            return out;
        }
        ConfigFile::Text(t) => t,
    };
    let root = match Json::parse(text) {
        Ok(Json::Obj(o)) => o,
        Ok(_) => {
            broken(&mut out, "top level must be an object");
            return out;
        }
        Err(msg) => {
            broken(&mut out, &msg);
            return out;
        }
    };
    let get = |o: &[(String, Json)], k: &str| o.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
    let d = Config::default();
    let names = ThemeId::ALL.map(ThemeId::as_str).join("|");
    let mut warn = |m: String| out.warnings.push(format!("xplain: config: {m}"));
    let mut c = d.clone();
    if let Some(v) = get(&root, "theme") {
        match v.as_str().and_then(ThemeId::parse) {
            Some(t) => c.theme = t,
            None => warn(format!("invalid theme {} ({names}); using {}", v.stringify(), d.theme.as_str())),
        }
    }
    if let Some(v) = get(&root, "view") {
        match v {
            Json::Obj(o) => {
                if let Some(m) = get(&o, "mode") {
                    match m.as_str().and_then(DiffMode::parse) {
                        Some(m) => c.mode = m,
                        None => warn(format!(
                            "invalid view.mode {} (all|staged|unstaged); using {}",
                            m.stringify(),
                            d.mode.as_str()
                        )),
                    }
                }
                for (k, slot, def) in [("split", &mut c.split, d.split), ("full", &mut c.full, d.full)] {
                    if let Some(b) = get(&o, k) {
                        match b {
                            Json::Bool(b) => *slot = b,
                            other => {
                                warn(format!("invalid view.{k} {} (boolean); using {def}", other.stringify()))
                            }
                        }
                    }
                }
            }
            _ => warn("view must be an object; using defaults".to_string()),
        }
    }
    let section = |name: &str, key: &str, slot: &mut bool, def: bool, warn: &mut dyn FnMut(String)| {
        if let Some(v) = get(&root, name) {
            match v {
                Json::Obj(o) => {
                    if let Some(b) = get(&o, key) {
                        match b {
                            Json::Bool(b) => *slot = b,
                            other => warn(format!(
                                "invalid {name}.{key} {} (boolean); using {def}",
                                other.stringify()
                            )),
                        }
                    }
                }
                _ => warn(format!("{name} must be an object; using defaults")),
            }
        }
    };
    section("app", "confirmQuit", &mut c.confirm_quit, d.confirm_quit, &mut warn);
    section("mcp", "autostart", &mut c.mcp_autostart, d.mcp_autostart, &mut warn);
    out.config = c;
    out
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
        use serde_json::json;
        match self {
            ConfigChange::Theme(t) => json!({"theme": t.as_str()}),
            ConfigChange::Mode(m) => json!({"view": {"mode": m.as_str()}}),
            ConfigChange::Split(b) => json!({"view": {"split": b}}),
            ConfigChange::Full(b) => json!({"view": {"full": b}}),
            ConfigChange::ConfirmQuit(b) => json!({"app": {"confirmQuit": b}}),
            ConfigChange::McpAutostart(b) => json!({"mcp": {"autostart": b}}),
        }
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
pub fn apply_patch(existing: Option<&str>, patch: &serde_json::Value) -> Result<String, ConfigSaveError> {
    let base = match existing {
        None => vec![("version".to_string(), Json::Num(1.0))],
        Some(text) => match Json::parse(text) {
            Ok(Json::Obj(o)) => o,
            _ => return Err(ConfigSaveError::Unreadable),
        },
    };
    let merged = match (Json::Obj(base), Json::from_value(patch)) {
        (Json::Obj(a), Json::Obj(b)) => Json::Obj(merge(a, b)),
        (a, _) => a,
    };
    let mut out = String::new();
    merged.write_pretty(&mut out, 0);
    out.push('\n');
    Ok(out)
}

fn merge(mut a: Vec<(String, Json)>, b: Vec<(String, Json)>) -> Vec<(String, Json)> {
    for (k, v) in b {
        match a.iter().position(|(key, _)| *key == k) {
            Some(i) => {
                let old = std::mem::replace(&mut a[i].1, Json::Null);
                a[i].1 = match (old, v) {
                    (Json::Obj(x), Json::Obj(y)) => Json::Obj(merge(x, y)),
                    (_, v) => v,
                };
            }
            None => a.push((k, v)),
        }
    }
    a
}

/// Order-preserving JSON value (JS object semantics: insertion order, duplicate key keeps first position,
/// last value). serde_json's map sorts keys, which would reorder the user's file on save.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 256;

impl Json {
    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    fn from_value(v: &serde_json::Value) -> Json {
        use serde_json::Value;
        match v {
            Value::Null => Json::Null,
            Value::Bool(b) => Json::Bool(*b),
            Value::Number(n) => Json::Num(n.as_f64().unwrap_or(0.0)),
            Value::String(s) => Json::Str(s.clone()),
            Value::Array(a) => Json::Arr(a.iter().map(Json::from_value).collect()),
            Value::Object(o) => Json::Obj(o.iter().map(|(k, v)| (k.clone(), Json::from_value(v))).collect()),
        }
    }

    fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { b: text.as_bytes(), i: 0, text };
        p.ws();
        let v = p.value(0)?;
        p.ws();
        if p.i < p.b.len() {
            return Err(p.unexpected());
        }
        Ok(v)
    }

    /// `JSON.stringify(v)`.
    fn stringify(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, None, 0);
        s
    }

    fn write_pretty(&self, out: &mut String, depth: usize) {
        self.write(out, Some("\t"), depth);
    }

    fn write(&self, out: &mut String, indent: Option<&str>, depth: usize) {
        let nl = |out: &mut String, d: usize| {
            if let Some(ind) = indent {
                out.push('\n');
                for _ in 0..d {
                    out.push_str(ind);
                }
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(&js_number(*n)),
            Json::Str(s) => write_str(out, s),
            Json::Arr(a) if a.is_empty() => out.push_str("[]"),
            Json::Obj(o) if o.is_empty() => out.push_str("{}"),
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    nl(out, depth + 1);
                    v.write(out, indent, depth + 1);
                }
                nl(out, depth);
                out.push(']');
            }
            Json::Obj(o) => {
                out.push('{');
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    nl(out, depth + 1);
                    write_str(out, k);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    v.write(out, indent, depth + 1);
                }
                nl(out, depth);
                out.push('}');
            }
        }
    }
}

fn js_number(n: f64) -> String {
    if !n.is_finite() {
        return "null".into();
    }
    if n == 0.0 {
        return "0".into();
    }
    let a = n.abs();
    if !(1e-6..1e21).contains(&a) {
        let s = format!("{n:e}");
        return match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        };
    }
    format!("{n}")
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    text: &'a str,
}

impl Parser<'_> {
    fn unexpected(&self) -> String {
        match self.text.get(self.i..).and_then(|r| r.chars().next()) {
            None => "Unexpected end of JSON input".to_string(),
            Some(c) => {
                format!("Unexpected token {c} in JSON at position {}", self.text[..self.i].chars().count())
            }
        }
    }

    fn ws(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Result<Json, String> {
        for &c in word.as_bytes() {
            if self.b.get(self.i) != Some(&c) {
                return Err(self.unexpected());
            }
            self.i += 1;
        }
        Ok(v)
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err("nesting too deep".to_string());
        }
        match self.b.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut o: Vec<(String, Json)> = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(o));
                }
                loop {
                    self.ws();
                    if self.b.get(self.i) != Some(&b'"') {
                        return Err(self.unexpected());
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') {
                        return Err(self.unexpected());
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value(depth + 1)?;
                    match o.iter().position(|(key, _)| *key == k) {
                        Some(p) => o[p].1 = v,
                        None => o.push((k, v)),
                    }
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(o));
                        }
                        _ => return Err(self.unexpected()),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(a));
                }
                loop {
                    self.ws();
                    a.push(self.value(depth + 1)?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(a));
                        }
                        _ => return Err(self.unexpected()),
                    }
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.unexpected()),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        match self.b.get(self.i) {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.unexpected()),
        }
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            if !matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                return Err(self.unexpected());
            }
            self.digits();
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
                return Err(self.unexpected());
            }
            self.digits();
        }
        let n = self.text[start..self.i].parse::<f64>().map_err(|_| self.unexpected())?;
        Ok(Json::Num(n))
    }

    fn digits(&mut self) {
        while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
            self.i += 1;
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.text.get(self.i..self.i + 4).ok_or_else(|| self.unexpected())?;
        if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.unexpected());
        }
        self.i += 4;
        u32::from_str_radix(h, 16).map_err(|_| self.unexpected())
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut s = String::new();
        loop {
            let Some(rest) = self.text.get(self.i..) else { return Err(self.unexpected()) };
            let Some(c) = rest.chars().next() else { return Err(self.unexpected()) };
            match c {
                '"' => {
                    self.i += 1;
                    return Ok(s);
                }
                '\\' => {
                    self.i += 1;
                    let Some(&e) = self.b.get(self.i) else { return Err(self.unexpected()) };
                    self.i += 1;
                    match e {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{8}'),
                        b'f' => s.push('\u{c}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let mut u = self.hex4()?;
                            if (0xD800..0xDC00).contains(&u) && self.text[self.i..].starts_with("\\u") {
                                let save = self.i;
                                self.i += 2;
                                match self.hex4() {
                                    Ok(lo) if (0xDC00..0xE000).contains(&lo) => {
                                        u = 0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00);
                                    }
                                    _ => self.i = save,
                                }
                            }
                            s.push(char::from_u32(u).unwrap_or('\u{FFFD}'));
                        }
                        _ => {
                            self.i -= 1;
                            return Err(self.unexpected());
                        }
                    }
                }
                c if (c as u32) < 0x20 => return Err(self.unexpected()),
                c => {
                    s.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(f: &str, x: &str, xdg: &str, h: &str) -> ConfigPathEnv {
        let o = |s: &str| Some(s.to_string());
        ConfigPathEnv { flag: o(f), xplain_config: o(x), xdg_config_home: o(xdg), home: o(h) }
    }

    fn load(text: &str) -> ConfigLoad {
        load_config("/p.json", &ConfigFile::Text(text.to_string()))
    }

    #[test]
    fn f_config_01_order_and_empty() {
        assert_eq!(resolve_config_path(&env("f", "x", "g", "h")), "f");
        assert_eq!(resolve_config_path(&env("", "x", "g", "h")), "x");
        assert_eq!(resolve_config_path(&env("", "", "g", "h")), "g/xplain/config.json");
        assert_eq!(resolve_config_path(&env("", "", "", "/h")), "/h/.config/xplain/config.json");
        assert_eq!(
            resolve_config_path(&ConfigPathEnv { home: Some("/h".into()), ..Default::default() }),
            "/h/.config/xplain/config.json"
        );
    }

    #[test]
    fn f_config_02_defaults_and_missing() {
        let l = load_config("/p", &ConfigFile::Missing);
        assert_eq!(l, ConfigLoad::default());
        assert_eq!(l.config, Config::default());
        assert!(load("{}").warnings.is_empty());
    }

    #[test]
    fn f_config_02_valid_values() {
        let l = load(
            r#"{"theme":"dull","view":{"mode":"staged","split":true,"full":false},"app":{"confirmQuit":false},"mcp":{"autostart":true},"x":1}"#,
        );
        assert!(l.warnings.is_empty());
        assert_eq!(
            l.config,
            Config {
                theme: ThemeId::Dull,
                mode: DiffMode::Staged,
                split: true,
                full: false,
                confirm_quit: false,
                mcp_autostart: true
            }
        );
    }

    #[test]
    fn f_config_03_every_warning() {
        let l = load(
            r#"{"theme":"x","view":{"mode":1,"split":"s","full":null},"app":{"confirmQuit":[]},"mcp":{"autostart":{"a":1}}}"#,
        );
        assert_eq!(
            l.warnings,
            [
                r#"xplain: config: invalid theme "x" (solarized|vibrant|dull|contrast|colorblind|light); using solarized"#,
                "xplain: config: invalid view.mode 1 (all|staged|unstaged); using all",
                r#"xplain: config: invalid view.split "s" (boolean); using false"#,
                "xplain: config: invalid view.full null (boolean); using true",
                "xplain: config: invalid app.confirmQuit [] (boolean); using true",
                r#"xplain: config: invalid mcp.autostart {"a":1} (boolean); using false"#,
            ]
        );
        assert_eq!(l.config, Config::default());
    }

    #[test]
    fn f_config_03_not_object_sections() {
        let l = load(r#"{"view":[],"app":3,"mcp":null}"#);
        assert_eq!(
            l.warnings,
            [
                "xplain: config: view must be an object; using defaults",
                "xplain: config: app must be an object; using defaults",
                "xplain: config: mcp must be an object; using defaults",
            ]
        );
    }

    #[test]
    fn f_config_03_bad_key_keeps_rest() {
        let l = load(r#"{"theme":"nope","view":{"split":true}}"#);
        assert_eq!(l.warnings.len(), 1);
        assert!(l.config.split);
        assert_eq!(l.config.theme, ThemeId::Solarized);
    }

    #[test]
    fn f_config_04_broken() {
        let suffix = "; using defaults, file will not be modified";
        for text in ["", "{", "nope"] {
            let l = load(text);
            assert_eq!(l.warnings.len(), 1);
            assert!(l.warnings[0].starts_with("xplain: config: /p.json: "));
            assert!(l.warnings[0].ends_with(suffix));
            assert_eq!(l.config, Config::default());
        }
        for text in ["[]", "1", "null"] {
            assert_eq!(
                load(text).warnings,
                [format!("xplain: config: /p.json: top level must be an object{suffix}")]
            );
        }
        let u = load_config("/d", &ConfigFile::Unreadable(IoReason::IsDirectory));
        assert_eq!(u.warnings, [format!("xplain: config: /d: is a directory{suffix}")]);
    }

    #[test]
    fn f_config_05_patch_table() {
        let p = |c: ConfigChange| c.to_patch().to_string();
        assert_eq!(p(ConfigChange::Theme(ThemeId::Dull)), r#"{"theme":"dull"}"#);
        assert_eq!(p(ConfigChange::Mode(DiffMode::All)), r#"{"view":{"mode":"all"}}"#);
        assert_eq!(p(ConfigChange::Split(true)), r#"{"view":{"split":true}}"#);
        assert_eq!(p(ConfigChange::Full(false)), r#"{"view":{"full":false}}"#);
        assert_eq!(p(ConfigChange::ConfirmQuit(false)), r#"{"app":{"confirmQuit":false}}"#);
        assert_eq!(p(ConfigChange::McpAutostart(true)), r#"{"mcp":{"autostart":true}}"#);
    }

    #[test]
    fn f_config_05_fresh_file() {
        let out = apply_patch(None, &ConfigChange::Theme(ThemeId::Dull).to_patch());
        assert_eq!(out, Ok("{\n\t\"version\": 1,\n\t\"theme\": \"dull\"\n}\n".to_string()));
    }

    #[test]
    fn f_config_05_merge_keeps_unknown_and_order() {
        let existing = r#"{"zeta":[1,{"a":null}],"view":{"split":true,"other":"k"},"version":1,"keys":{}}"#;
        let out =
            apply_patch(Some(existing), &ConfigChange::Mode(DiffMode::Staged).to_patch()).unwrap_or_default();
        let expect = "{\n\t\"zeta\": [\n\t\t1,\n\t\t{\n\t\t\t\"a\": null\n\t\t}\n\t],\n\t\"view\": {\n\t\t\"split\": true,\n\t\t\"other\": \"k\",\n\t\t\"mode\": \"staged\"\n\t},\n\t\"version\": 1,\n\t\"keys\": {}\n}\n";
        assert_eq!(out, expect);
    }

    #[test]
    fn f_config_05_overwrite_non_object_value() {
        let out =
            apply_patch(Some(r#"{"view":3}"#), &ConfigChange::Full(true).to_patch()).unwrap_or_default();
        assert_eq!(out, "{\n\t\"view\": {\n\t\t\"full\": true\n\t}\n}\n");
    }

    #[test]
    fn f_config_05_unreadable_existing() {
        let p = ConfigChange::Split(true).to_patch();
        for t in ["", "[]", "3", "null", "{bad"] {
            assert_eq!(apply_patch(Some(t), &p), Err(ConfigSaveError::Unreadable));
        }
    }

    #[test]
    fn f_config_05_string_and_number_roundtrip() {
        let out = apply_patch(
            Some(r#"{"a":"q\"\\\n\u0001é","n":1.5,"e":1e21}"#),
            &ConfigChange::Split(false).to_patch(),
        )
        .unwrap_or_default();
        assert!(out.contains(r#""a": "q\"\\\n\u0001é""#), "{out}");
        assert!(out.contains("\"n\": 1.5"));
        assert!(out.contains("\"e\": 1e+21"));
    }
}
