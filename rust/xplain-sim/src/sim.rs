//! The in-process app: `State` + effect executor + manual clock, driven by keys, pastes, HTTP requests and
//! clock advances. See the crate README for the API overview.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use regex::Regex;
use xplain_app::config_io::read_config_file;
use xplain_app::env::RawEnv;
use xplain_app::run::{Startup, prepare};
use xplain_core::event::{Event, ReqId, TimerId};
use xplain_core::integration::{CommandError, CommandOutput, CommandResult, CommandSpec};
use xplain_core::mcp::{ConnId, HttpRequest, McpEndpoint, port_busy_message};
use xplain_core::screen::{Screen, Size};
use xplain_core::state::Now;
use xplain_core::{Effect, State};

use crate::exec::{Env, ensure_token};
use crate::fixture::{Fixture, TempRoot, build_fixture, must, run_git};
use crate::http::{Http, HttpReply, Pending};
use crate::keys::parse_keys;
use crate::shim::{Call, Rule, Shim};
use crate::view::{self, CellExpect, CellView, Pos};

/// Fixed start of the manual clock: 2025-01-02T03:04:05Z. `advance_clock` moves it.
pub const START_UNIX_MS: i64 = 1_735_787_045_000;

/// Builds a [`Sim`]. Every setter has a sensible default: standard fixture, no args, 120x40, config absent.
pub struct SimBuilder {
    fixture: Fixture,
    args: Vec<String>,
    size: (u16, u16),
    env: Vec<(String, Option<String>)>,
    config: Option<String>,
    files: Vec<(String, Vec<u8>)>,
    cwd: Option<String>,
    theme: Option<String>,
    shims: Vec<(String, Vec<Rule>)>,
    hold_io: bool,
    busy_ports: Vec<u16>,
    utc_offset_secs: i32,
}

impl SimBuilder {
    /// `standard` (default), `empty` or `nogit`.
    pub fn fixture(mut self, f: Fixture) -> Self {
        self.fixture = f;
        self
    }
    /// argv without the program name, parsed by the app's own parser. `${TMP} ${REPO} ${HOME} ${CONFIG} ${STATE}`
    /// expand in every arg.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }
    /// Terminal size in columns and rows.
    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.size = (cols, rows);
        self
    }
    /// Set an env var the app reads: `HOME XDG_CONFIG_HOME XDG_STATE_HOME XPLAIN_CONFIG XPLAIN_MCP_PORT
    /// XPLAIN_SYNC COLORTERM` (others panic). Placeholders expand. Empty string = set but empty.
    pub fn env(mut self, k: &str, v: &str) -> Self {
        self.env.push((k.into(), Some(v.into())));
        self
    }
    /// Remove an env var (defaults: HOME, XDG_CONFIG_HOME, XDG_STATE_HOME point into the temp dir).
    pub fn env_unset(mut self, k: &str) -> Self {
        self.env.push((k.into(), None));
        self
    }
    /// Contents of `${CONFIG}/xplain/config.json`, written before start.
    pub fn config(mut self, text: &str) -> Self {
        self.config = Some(text.into());
        self
    }
    /// Same as [`config`](Self::config) from a JSON value.
    pub fn config_json(self, v: serde_json::Value) -> Self {
        self.config(&v.to_string())
    }
    /// A file written before start (`path` relative to the repo, or with a placeholder). Parents are created.
    pub fn file(mut self, path: &str, contents: &str) -> Self {
        self.files.push((path.into(), contents.as_bytes().to_vec()));
        self
    }
    /// Like [`file`](Self::file) with raw bytes.
    pub fn file_bytes(mut self, path: &str, contents: &[u8]) -> Self {
        self.files.push((path.into(), contents.to_vec()));
        self
    }
    /// App cwd, relative to the repo, created if missing. Default: the repo.
    pub fn cwd(mut self, rel: &str) -> Self {
        self.cwd = Some(rel.into());
        self
    }
    /// `--theme <name>` (prepended to the args).
    pub fn theme(mut self, name: &str) -> Self {
        self.theme = Some(name.into());
        self
    }
    /// Fake CLI `name` exists: exit 0, no output.
    pub fn shim(mut self, name: &str) -> Self {
        self.shims.push((name.into(), Vec::new()));
        self
    }
    /// Fake CLI `name` with scripted rules (first match wins).
    pub fn shim_rules(mut self, name: &str, rules: Vec<Rule>) -> Self {
        self.shims.push((name.into(), rules));
        self
    }
    /// Hold git and file effects (also the initial diff load) until [`Sim::release_io`]. The screen shows
    /// `Loading...` meanwhile.
    pub fn hold_io(mut self) -> Self {
        self.hold_io = true;
        self
    }
    /// MCP start on this port fails as if it were taken.
    pub fn busy_port(mut self, port: u16) -> Self {
        self.busy_ports.push(port);
        self
    }
    /// Local UTC offset reported to the app (export file names use local time). Default 0.
    pub fn utc_offset_secs(mut self, secs: i32) -> Self {
        self.utc_offset_secs = secs;
        self
    }

    /// Create the temp tree, start the app and settle. Panics on harness errors (git missing, unknown env var).
    pub fn build(self) -> Sim {
        Sim::start(self)
    }
}

struct TimerEntry {
    id: TimerId,
    due: u64,
    seq: u64,
    background: bool,
}

/// A running in-process app. Drop removes the temp dir (set `XPLAIN_SIM_KEEP=1` to keep it for debugging).
pub struct Sim {
    root: TempRoot,
    repo: PathBuf,
    home: PathBuf,
    proc_cwd: PathBuf,
    config_home: PathBuf,
    state_home: PathBuf,
    state: Option<Box<State>>,
    size: Size,
    elapsed_ms: u64,
    utc_offset_secs: i32,
    timers: Vec<TimerEntry>,
    timer_seq: u64,
    events: VecDeque<Event>,
    jobs: VecDeque<Effect>,
    hold_io: bool,
    shims: HashMap<String, Shim>,
    calls: Vec<Call>,
    blocked: Vec<(String, ReqId, CommandResult)>,
    clipboard: Vec<String>,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    replies: HashMap<ConnId, HttpReply>,
    parked: HashSet<ConnId>,
    server: Option<McpEndpoint>,
    busy_ports: HashSet<u16>,
    next_port: u16,
    next_conn: u64,
    next_token: u64,
    effects: Vec<Effect>,
}

impl Sim {
    /// Start configuring a Sim.
    pub fn builder() -> SimBuilder {
        SimBuilder {
            fixture: Fixture::Standard,
            args: Vec::new(),
            size: (120, 40),
            env: Vec::new(),
            config: None,
            files: Vec::new(),
            cwd: None,
            theme: None,
            shims: Vec::new(),
            hold_io: false,
            busy_ports: Vec::new(),
            utc_offset_secs: 0,
        }
    }

    fn start(b: SimBuilder) -> Sim {
        let root = TempRoot::new();
        let tmp = root.path().to_path_buf();
        let (repo, home) = (tmp.join("repo"), tmp.join("home"));
        let (config_home, state_home) = (tmp.join("config"), tmp.join("state"));
        must(std::fs::create_dir_all(&home), "create home");
        build_fixture(b.fixture, &repo, &home);
        let proc_cwd = match &b.cwd {
            Some(c) => repo.join(c),
            None => repo.clone(),
        };
        must(std::fs::create_dir_all(&proc_cwd), "create cwd");

        let mut sim = Sim {
            root,
            repo,
            home,
            proc_cwd,
            config_home,
            state_home,
            state: None,
            size: Size { cols: b.size.0, rows: b.size.1 },
            elapsed_ms: 0,
            utc_offset_secs: b.utc_offset_secs,
            timers: Vec::new(),
            timer_seq: 0,
            events: VecDeque::new(),
            jobs: VecDeque::new(),
            hold_io: b.hold_io,
            shims: HashMap::new(),
            calls: Vec::new(),
            blocked: Vec::new(),
            clipboard: Vec::new(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            replies: HashMap::new(),
            parked: HashSet::new(),
            server: None,
            busy_ports: b.busy_ports.iter().copied().collect(),
            next_port: 40000,
            next_conn: 1,
            next_token: 1,
            effects: Vec::new(),
        };
        for (name, rules) in b.shims {
            sim.shims.insert(name, Shim { rules, released: false });
        }
        if let Some(text) = &b.config {
            let p = sim.config_home.join("xplain/config.json");
            sim.write_at(&p, text.as_bytes());
        }
        for (path, contents) in &b.files {
            let p = sim.path(path);
            sim.write_at(&p, contents);
        }

        let mut raw = RawEnv {
            home: Some(sim.home.to_string_lossy().into_owned()),
            xdg_config_home: Some(sim.config_home.to_string_lossy().into_owned()),
            xdg_state_home: Some(sim.state_home.to_string_lossy().into_owned()),
            ..RawEnv::default()
        };
        for (k, v) in &b.env {
            let v = v.as_deref().map(|v| sim.expand(v));
            match k.as_str() {
                "HOME" => raw.home = v,
                "XDG_CONFIG_HOME" => raw.xdg_config_home = v,
                "XDG_STATE_HOME" => raw.xdg_state_home = v,
                "XPLAIN_CONFIG" => raw.xplain_config = v,
                "XPLAIN_MCP_PORT" => raw.xplain_mcp_port = v,
                "XPLAIN_SYNC" => raw.xplain_sync = v,
                "COLORTERM" => raw.colorterm = v,
                other => panic!("xplain-sim: env var {other} is not read by the app"),
            }
        }
        let mut args: Vec<String> = Vec::new();
        if let Some(t) = &b.theme {
            args.extend(["--theme".to_string(), t.clone()]);
        }
        args.extend(b.args.iter().map(|a| sim.expand(a)));

        let abs_cwd = sim.proc_cwd.to_string_lossy().into_owned();
        // relative config paths are relative to the app's cwd, not the test process's
        let read = |p: &str| read_config_file(&std::path::Path::new(&abs_cwd).join(p).to_string_lossy());
        match prepare(&args, &raw, &abs_cwd, sim.size, &read) {
            Startup::Exit { code, stdout, stderr } => {
                sim.exit_code = Some(code);
                sim.stdout = stdout;
                sim.stderr = stderr;
            }
            Startup::Ui { state, effects, warnings, .. } => {
                sim.stderr = warnings.iter().map(|w| format!("{w}\n")).collect();
                sim.state = Some(state);
                sim.dispatch(effects);
                sim.deliver(Event::Started);
                sim.settle();
            }
        }
        sim
    }

    // ---- paths and temp files -------------------------------------------------------------------------------

    /// Expand `${TMP} ${REPO} ${HOME} ${CONFIG} ${STATE}` (same names as the e2e suite).
    pub fn expand(&self, s: &str) -> String {
        s.replace("${TMP}", &self.root.path().to_string_lossy())
            .replace("${REPO}", &self.repo.to_string_lossy())
            .replace("${HOME}", &self.home.to_string_lossy())
            .replace("${CONFIG}", &self.config_home.to_string_lossy())
            .replace("${STATE}", &self.state_home.to_string_lossy())
    }
    /// Absolute path for `p` after expansion; relative paths are relative to the repo.
    pub fn path(&self, p: &str) -> PathBuf {
        self.repo.join(self.expand(p))
    }
    /// The temp root of this Sim.
    pub fn tmp(&self) -> &Path {
        self.root.path()
    }
    /// The fixture repo (`${REPO}`).
    pub fn repo(&self) -> &Path {
        &self.repo
    }
    /// `${HOME}`.
    pub fn home(&self) -> &Path {
        &self.home
    }
    /// `${CONFIG}` (default `XDG_CONFIG_HOME`).
    pub fn config_dir(&self) -> &Path {
        &self.config_home
    }
    /// `${STATE}` (default `XDG_STATE_HOME`); the token file is `${STATE}/xplain/mcp.json`.
    pub fn state_dir(&self) -> &Path {
        &self.state_home
    }

    fn write_at(&self, p: &Path, contents: &[u8]) {
        if let Some(d) = p.parent() {
            must(std::fs::create_dir_all(d), "create parent dir");
        }
        must(std::fs::write(p, contents), &format!("write {}", p.display()));
    }
    /// Write a text file (parents created). Does not tell the app; press `r` to reload.
    pub fn write_file(&self, path: &str, contents: &str) {
        self.write_at(&self.path(path), contents.as_bytes());
    }
    /// Write raw bytes.
    pub fn write_bytes(&self, path: &str, contents: &[u8]) {
        self.write_at(&self.path(path), contents);
    }
    /// Append text to a file (created if missing).
    pub fn append_file(&self, path: &str, contents: &str) {
        let mut cur = std::fs::read(self.path(path)).unwrap_or_default();
        cur.extend_from_slice(contents.as_bytes());
        self.write_at(&self.path(path), &cur);
    }
    /// Remove a file or directory tree.
    pub fn remove_file(&self, path: &str) {
        let p = self.path(path);
        let r = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        must(r, &format!("remove {}", p.display()));
    }
    /// Create a directory (and parents).
    pub fn mkdir(&self, path: &str) {
        must(std::fs::create_dir_all(self.path(path)), "mkdir");
    }
    /// Read a file as text; panics when missing.
    #[track_caller]
    pub fn file(&self, path: &str) -> String {
        let p = self.path(path);
        match std::fs::read(&p) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => panic!("cannot read {}: {e}", p.display()),
        }
    }
    /// Read a file as text; `None` when it does not exist or cannot be read.
    pub fn try_file(&self, path: &str) -> Option<String> {
        std::fs::read(self.path(path)).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
    }
    /// Does the path exist?
    pub fn file_exists(&self, path: &str) -> bool {
        self.path(path).exists()
    }
    /// Sorted names in a directory (empty when missing). Use to find files with generated names.
    pub fn list_dir(&self, path: &str) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.path(path))
            .map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }
    /// Unix permission bits of a path.
    pub fn file_mode(&self, path: &str) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        let p = self.path(path);
        must(std::fs::metadata(&p), &format!("stat {}", p.display())).permissions().mode() & 0o777
    }
    /// Run `git args...` in the repo (hermetic env); returns stdout, panics on non-zero exit.
    #[track_caller]
    pub fn git(&self, args: &[&str]) -> String {
        let o = run_git(&self.repo, &self.home, args);
        assert!(o.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    }
    /// Run git and return its exit code (no assertion).
    pub fn git_code(&self, args: &[&str]) -> i32 {
        run_git(&self.repo, &self.home, args).status.code().unwrap_or(-1)
    }

    // ---- driving --------------------------------------------------------------------------------------------

    /// Send keys in e2e notation, one at a time, settling after each (see [`crate::keys`]).
    #[track_caller]
    pub fn keys(&mut self, spec: &str) -> &mut Self {
        for k in parse_keys(spec) {
            self.input(Event::Key(k));
        }
        self
    }
    /// Bracketed paste of `text` (raw, newlines as given), then settle.
    #[track_caller]
    pub fn paste(&mut self, text: &str) -> &mut Self {
        self.input(Event::Paste(text.to_string()));
        self
    }
    /// Terminal resize.
    #[track_caller]
    pub fn resize(&mut self, cols: u16, rows: u16) -> &mut Self {
        self.size = Size { cols, rows };
        self.input(Event::Resize(self.size));
        self
    }
    #[track_caller]
    fn input(&mut self, ev: Event) {
        assert!(
            self.exit_code.is_none() && self.state.is_some(),
            "input after the app exited (code {:?})",
            self.exit_code
        );
        self.deliver(ev);
        self.settle();
    }

    /// Run everything that is ready: effect results, until no work is left (the sync barrier). Held IO stays
    /// held; timers only move with [`advance_clock`](Self::advance_clock).
    pub fn settle(&mut self) {
        loop {
            if let Some(ev) = self.events.pop_front() {
                self.deliver(ev);
            } else if !self.hold_io
                && let Some(job) = self.jobs.pop_front()
            {
                self.run_job(job);
            } else {
                break;
            }
        }
    }

    /// Hold git and file effects from now on (results wait for [`release_io`](Self::release_io)).
    pub fn hold_io(&mut self) -> &mut Self {
        self.hold_io = true;
        self
    }
    /// Run held git and file effects and stop holding, then settle.
    pub fn release_io(&mut self) -> &mut Self {
        self.hold_io = false;
        self.settle();
        self
    }
    /// Number of git and file effects waiting because of [`hold_io`](Self::hold_io).
    pub fn held_io(&self) -> usize {
        self.jobs.len()
    }

    /// Move the manual clock forward `ms`, firing due timers in order (each followed by a settle).
    pub fn advance_clock(&mut self, ms: u64) -> &mut Self {
        let target = self.elapsed_ms + ms;
        loop {
            let next =
                self.timers.iter().filter(|t| t.due <= target).min_by_key(|t| (t.due, t.seq)).map(|t| t.id);
            let Some(id) = next else { break };
            let Some(pos) = self.timers.iter().position(|t| t.id == id) else { break };
            let t = self.timers.remove(pos);
            self.elapsed_ms = self.elapsed_ms.max(t.due);
            if self.exit_code.is_some() {
                break;
            }
            self.deliver(Event::Timer(id));
            self.settle();
        }
        self.elapsed_ms = target;
        self
    }
    /// Armed timers as `(id, ms until due, background)`, soonest first.
    pub fn timers(&self) -> Vec<(TimerId, u64, bool)> {
        let mut v: Vec<_> = self
            .timers
            .iter()
            .map(|t| (t.due.saturating_sub(self.elapsed_ms), t.seq, t.id, t.background))
            .collect();
        v.sort_by_key(|(d, s, ..)| (*d, *s));
        v.into_iter().map(|(d, _, id, bg)| (id, d, bg)).collect()
    }
    /// Milliseconds the manual clock has advanced.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    fn now(&self) -> Now {
        Now { unix_ms: START_UNIX_MS + self.elapsed_ms as i64, utc_offset_secs: self.utc_offset_secs }
    }

    fn deliver(&mut self, ev: Event) {
        if self.exit_code.is_some() {
            return;
        }
        let now = self.now();
        let Some(state) = self.state.as_mut() else { return };
        state.clock = now;
        let fx = xplain_core::update(state, ev);
        self.dispatch(fx);
    }

    fn dispatch(&mut self, fx: Vec<Effect>) {
        for e in fx {
            self.effects.push(e.clone());
            match e {
                Effect::Clipboard(t) => self.clipboard.push(t),
                Effect::SetTimer { id, after_ms, background } => {
                    self.timers.retain(|t| t.id != id);
                    self.timer_seq += 1;
                    self.timers.push(TimerEntry {
                        id,
                        due: self.elapsed_ms + after_ms,
                        seq: self.timer_seq,
                        background,
                    });
                }
                Effect::CancelTimer(id) => self.timers.retain(|t| t.id != id),
                Effect::Exit { code } => {
                    self.exit_code = Some(code);
                    self.timers.clear();
                    return;
                }
                Effect::HttpReply { conn, response } => {
                    if self.parked.remove(&conn) {
                        self.replies.insert(conn, response.into());
                    }
                }
                Effect::RunCommand { req, cmd } => self.run_command(req, cmd),
                Effect::Highlight { key, lang, start, lines, carry } => {
                    let (runs, end) = xplain_core::highlight_lines(&lang, &lines, carry.as_ref());
                    self.events.push_back(Event::Highlighted { key, start, runs, end });
                }
                io => self.jobs.push_back(io),
            }
        }
    }

    fn env(&self) -> Env<'_> {
        Env { cwd: &self.proc_cwd, home: &self.home }
    }

    fn run_job(&mut self, e: Effect) {
        let ev = match e {
            Effect::LoadDiff { req, spec } => Event::DiffLoaded { req, result: self.env().load_diff(&spec) },
            Effect::ListFiles { req, cwd } => {
                Event::FilesListed { req, files: self.env().list_files(cwd.as_deref()) }
            }
            Effect::ReadFile { req, path } => Event::FileRead { req, result: self.env().read_file(&path) },
            Effect::SaveConfig { req, path, change } => {
                Event::ConfigSaved { req, result: self.env().save_config(&path, &change) }
            }
            Effect::WriteExport { req, path, contents } => {
                Event::ExportWritten { req, result: self.env().write_export(&path, &contents) }
            }
            Effect::McpStart { req, port, state_dir } => {
                Event::McpStarted { req, result: self.mcp_start(port, &state_dir) }
            }
            Effect::McpStop { req } => {
                self.server = None;
                self.parked.clear();
                Event::McpStopped { req }
            }
            other => panic!("xplain-sim: effect {other:?} is not an IO job"),
        };
        self.events.push_back(ev);
    }

    fn mcp_start(&mut self, port: u16, state_dir: &str) -> Result<McpEndpoint, String> {
        self.server = None;
        self.parked.clear();
        let n = self.next_token;
        self.next_token += 1;
        let mut random = [0u8; 32];
        for (i, b) in random.iter_mut().enumerate() {
            *b = (n as u8).wrapping_mul(31).wrapping_add((i as u8).wrapping_mul(7)).wrapping_add(1);
        }
        let token = ensure_token(state_dir, random)?;
        if self.busy_ports.contains(&port) {
            return Err(port_busy_message(port));
        }
        let actual = if port == 0 {
            self.next_port += 1;
            self.next_port
        } else {
            port
        };
        let ep = McpEndpoint { url: format!("http://127.0.0.1:{actual}/mcp"), token, port: actual };
        self.server = Some(ep.clone());
        Ok(ep)
    }

    fn run_command(&mut self, req: ReqId, cmd: CommandSpec) {
        let cwd = self.env().resolve(cmd.cwd.as_deref()).to_string_lossy().into_owned();
        let name = Path::new(&cmd.program)
            .file_name()
            .map_or(cmd.program.clone(), |n| n.to_string_lossy().into_owned());
        self.calls.push(Call { program: name.clone(), args: cmd.args.clone(), cwd, env: cmd.env.clone() });
        let Some(shim) = self.shims.get(&name) else {
            self.events.push_back(Event::CommandDone { req, result: Err(CommandError::NotFound) });
            return;
        };
        let rule = shim.rules.iter().find(|r| r.matches(&cmd.args));
        let (result, block) = match rule {
            Some(r) => (r.result.clone(), r.block && !shim.released),
            None => (Ok(CommandOutput { code: 0, stdout: String::new(), stderr: String::new() }), false),
        };
        if block {
            self.blocked.push((name, req, result));
        } else {
            self.events.push_back(Event::CommandDone { req, result });
        }
    }

    // ---- fake integration CLIs ------------------------------------------------------------------------------

    /// Create or replace the rules of fake CLI `name` (replacing stops blocking).
    pub fn set_shim(&mut self, name: &str, rules: Vec<Rule>) -> &mut Self {
        self.shims.insert(name.into(), Shim { rules, released: false });
        self
    }
    /// The CLI is gone from PATH: later calls fail with `CommandError::NotFound`.
    pub fn remove_shim(&mut self, name: &str) -> &mut Self {
        self.shims.remove(name);
        self
    }
    /// Answer every call of `name` that is waiting on a `block()` rule, stop blocking it, settle.
    #[track_caller]
    pub fn release(&mut self, name: &str) -> &mut Self {
        let Some(shim) = self.shims.get_mut(name) else { panic!("release: no shim named {name:?}") };
        shim.released = true;
        let (now, later): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.blocked).into_iter().partition(|(n, ..)| n == name);
        self.blocked = later;
        for (_, req, result) in now {
            self.events.push_back(Event::CommandDone { req, result });
        }
        self.settle();
        self
    }
    /// Calls of fake CLI `name` so far, oldest first (blocked calls are logged when they start).
    pub fn calls(&self, name: &str) -> Vec<Call> {
        self.calls.iter().filter(|c| c.program == name).cloned().collect()
    }
    /// Number of calls of `name` waiting on a `block()` rule.
    pub fn blocked_calls(&self, name: &str) -> usize {
        self.blocked.iter().filter(|(n, ..)| n == name).count()
    }

    // ---- MCP over HTTP (in-process, no sockets) -------------------------------------------------------------

    /// The running MCP server's endpoint, `None` when stopped (requests then panic: connection refused).
    pub fn mcp_endpoint(&self) -> Option<&McpEndpoint> {
        self.server.as_ref()
    }
    /// Send a request without waiting for the reply (a long poll parks). Settles after delivery.
    #[track_caller]
    pub fn http_start(&mut self, req: Http) -> Pending {
        let Some(ep) = self.server.clone() else { panic!("connection refused: MCP server is not running") };
        let conn = ConnId(self.next_conn);
        self.next_conn += 1;
        let mut headers = Vec::new();
        if req.default_host {
            headers.push(("host".to_string(), format!("127.0.0.1:{}", ep.port)));
        }
        let token = match &req.auth {
            None => Some(ep.token.clone()),
            Some(t) => t.clone(),
        };
        if let Some(t) = token {
            headers.push(("authorization".into(), format!("Bearer {t}")));
        }
        headers.extend(req.headers.iter().cloned());
        let mut entropy = [0u8; 16];
        for (i, b) in entropy.iter_mut().enumerate() {
            *b = (conn.0 as u8).wrapping_mul(17).wrapping_add((i as u8).wrapping_mul(13)).wrapping_add(3);
        }
        self.parked.insert(conn);
        let request = HttpRequest {
            conn,
            method: req.method,
            path: req.path,
            headers,
            body: req.body,
            body_too_large: req.too_large,
            remote_port: req.remote_port,
            entropy,
        };
        self.input(Event::McpHttp(request));
        Pending(conn)
    }
    /// The reply for `p` if it was written already.
    pub fn http_reply(&self, p: &Pending) -> Option<HttpReply> {
        self.replies.get(&p.0).cloned()
    }
    /// Settle and return the reply for `p`; panics (with the screen) when it is still parked.
    #[track_caller]
    pub fn http_await(&mut self, p: &Pending) -> HttpReply {
        self.settle();
        match self.http_reply(p) {
            Some(r) => r,
            None => panic!("http request {:?} has no reply yet (parked)\n{}", p.0, self.dump()),
        }
    }
    /// Send `req` and return its reply (the reply must not park).
    #[track_caller]
    pub fn http(&mut self, req: Http) -> HttpReply {
        let p = self.http_start(req);
        self.http_await(&p)
    }
    /// MCP `tools/call` of `tool` with `args`, authorized with the real token.
    #[track_caller]
    pub fn mcp_call(&mut self, tool: &str, args: serde_json::Value) -> HttpReply {
        self.http(Http::tool(tool, args))
    }
    /// JSON-RPC `method` with `params` (`initialize`, `tools/list`, ...), authorized.
    #[track_caller]
    pub fn mcp_rpc(&mut self, method: &str, params: serde_json::Value) -> HttpReply {
        self.http(Http::rpc(method, params))
    }
    /// Drop a request's connection: core gets `McpConnClosed` when it was still parked.
    #[track_caller]
    pub fn http_abort(&mut self, p: &Pending) -> &mut Self {
        if self.parked.remove(&p.0) {
            self.input(Event::McpConnClosed(p.0));
        }
        self
    }

    // ---- observation ----------------------------------------------------------------------------------------

    /// Render the current screen (blank when the app exited during startup).
    pub fn render(&self) -> Screen {
        match &self.state {
            Some(s) => xplain_core::view(s),
            None => Screen::blank(self.size),
        }
    }
    /// All rows as text, right-trimmed.
    pub fn screen(&self) -> Vec<String> {
        view::rows(&self.render())
    }
    /// The screen as one string, rows joined by `\n`, right-trimmed.
    pub fn text(&self) -> String {
        self.screen().join("\n")
    }
    /// Row `n` (negative counts from the bottom), right-trimmed.
    #[track_caller]
    pub fn row(&self, n: isize) -> String {
        let s = self.render();
        view::row_text(&s, view::resolve_row(&s, n))
    }
    /// Full screen dump with row numbers, as printed by failing assertions.
    pub fn dump(&self) -> String {
        view::dump(&self.render())
    }
    /// Cell at column `x`, row `y` (negative from the bottom).
    #[track_caller]
    pub fn cell(&self, x: usize, y: isize) -> CellView {
        view::cell(&self.render(), x, y)
    }
    /// First occurrence of `text` (rows top to bottom).
    pub fn find(&self, text: &str) -> Option<Pos> {
        view::find(&self.render(), text, 0)
    }
    /// `nth` (0-based) occurrence of `text`.
    pub fn find_nth(&self, text: &str, nth: usize) -> Option<Pos> {
        view::find(&self.render(), text, nth)
    }
    /// First occurrence of `text` within row `row`.
    #[track_caller]
    pub fn find_in_row(&self, row: isize, text: &str) -> Option<Pos> {
        let s = self.render();
        let y = view::resolve_row(&s, row);
        view::find_in_row(&s, y, text, 0).map(|x| Pos { x, y })
    }
    /// Cell at the first occurrence of `text`; panics with the screen when absent.
    #[track_caller]
    pub fn cell_of(&self, text: &str) -> CellView {
        match self.find(text) {
            Some(p) => self.cell(p.x, p.y as isize),
            None => panic!("text {text:?} not on screen\n{}", self.dump()),
        }
    }
    /// Cell at the first occurrence of `text` within row `row`.
    #[track_caller]
    pub fn cell_of_in_row(&self, row: isize, text: &str) -> CellView {
        match self.find_in_row(row, text) {
            Some(p) => self.cell(p.x, p.y as isize),
            None => panic!("text {text:?} not in row {row}\n{}", self.dump()),
        }
    }

    /// Does the screen contain `text`?
    pub fn contains(&self, text: &str) -> bool {
        self.text().contains(text)
    }
    /// Does the multiline regex `pattern` match the screen text (`^`/`$` match at row edges)?
    #[track_caller]
    pub fn matches(&self, pattern: &str) -> bool {
        re(&format!("(?m){pattern}")).is_match(&self.text())
    }

    /// Assert the screen contains `text`.
    #[track_caller]
    pub fn assert_contains(&self, text: &str) {
        assert!(self.contains(text), "screen does not contain {text:?}\n{}", self.dump());
    }
    /// Assert the screen does not contain `text`.
    #[track_caller]
    pub fn assert_not_contains(&self, text: &str) {
        assert!(!self.contains(text), "screen unexpectedly contains {text:?}\n{}", self.dump());
    }
    /// Assert the multiline regex matches somewhere in the screen text.
    #[track_caller]
    pub fn assert_matches(&self, pattern: &str) {
        assert!(self.matches(pattern), "screen does not match /{pattern}/\n{}", self.dump());
    }
    /// Assert row `n` (negative from the bottom, right-trimmed) equals `want`.
    #[track_caller]
    pub fn assert_row(&self, n: isize, want: &str) {
        let got = self.row(n);
        assert!(got == want, "row {n}: want {want:?}, got {got:?}\n{}", self.dump());
    }
    /// Assert row `n` contains `text`.
    #[track_caller]
    pub fn assert_row_contains(&self, n: isize, text: &str) {
        let got = self.row(n);
        assert!(got.contains(text), "row {n} does not contain {text:?}, got {got:?}\n{}", self.dump());
    }
    /// Assert the regex matches (unanchored) in row `n`.
    #[track_caller]
    pub fn assert_row_matches(&self, n: isize, pattern: &str) {
        let got = self.row(n);
        assert!(
            re(pattern).is_match(&got),
            "row {n} does not match /{pattern}/, got {got:?}\n{}",
            self.dump()
        );
    }
    /// Assert the cell at `(x, y)` has the expected properties.
    #[track_caller]
    pub fn assert_cell(&self, x: usize, y: isize, want: CellExpect) {
        let c = self.cell(x, y);
        let d = want.diff(&c);
        assert!(d.is_empty(), "cell ({x},{y}): {}\n{}", d.join("; "), self.dump());
    }
    /// Assert the cell at the first occurrence of `text` (plus `offset` columns) has the expected properties.
    #[track_caller]
    pub fn assert_text_cell(&self, text: &str, offset: usize, want: CellExpect) {
        let Some(p) = self.find(text) else { panic!("text {text:?} not on screen\n{}", self.dump()) };
        self.assert_cell(p.x + offset, p.y as isize, want);
    }
    /// Groups of the first match of `pattern` in row `n` (index 0 = whole match, unmatched groups empty).
    /// Panics with the screen when there is no match. Use to read generated names off the screen.
    #[track_caller]
    pub fn capture_row(&self, n: isize, pattern: &str) -> Vec<String> {
        let got = self.row(n);
        match re(pattern).captures(&got) {
            Some(c) => c.iter().map(|m| m.map_or(String::new(), |m| m.as_str().to_string())).collect(),
            None => panic!("row {n} does not match /{pattern}/, got {got:?}\n{}", self.dump()),
        }
    }

    // ---- outputs --------------------------------------------------------------------------------------------

    /// Last text copied with OSC 52.
    pub fn clipboard(&self) -> Option<&str> {
        self.clipboard.last().map(String::as_str)
    }
    /// Every copy so far, oldest first.
    pub fn clipboard_all(&self) -> &[String] {
        &self.clipboard
    }
    /// Exit code once the app exited (also for `--help` and flag errors at startup).
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
    /// Stdout of a non-UI startup outcome (`--help`, `config path`).
    pub fn stdout(&self) -> &str {
        &self.stdout
    }
    /// Startup stderr: flag errors and config warnings.
    pub fn stderr(&self) -> &str {
        &self.stderr
    }
    /// Every effect core emitted so far, in order (for asserting effects nobody can see).
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    /// Take and clear the effect log.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
    /// Has core finished the first diff load (is the app ready)?
    pub fn is_ready(&self) -> bool {
        self.state.as_ref().is_some_and(|s| s.is_ready())
    }
}

#[track_caller]
fn re(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => panic!("bad regex /{pattern}/: {e}"),
    }
}
