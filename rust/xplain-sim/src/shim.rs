//! Fake command runner for integration CLI effects (`Effect::RunCommand`): scripted results per program name,
//! recorded calls, and calls that wait for `Sim::release`.

use xplain_core::integration::{CommandError, CommandOutput, CommandResult};

/// One scripted answer. The first rule whose `args` are a prefix of the call's argv wins (no `args` = every call).
#[derive(Debug, Clone)]
pub struct Rule {
    pub(crate) args: Vec<String>,
    pub(crate) result: CommandResult,
    pub(crate) block: bool,
}

impl Rule {
    /// Matches every call; exit 0, no output.
    pub fn any() -> Rule {
        Rule {
            args: Vec::new(),
            result: Ok(CommandOutput { code: 0, stdout: String::new(), stderr: String::new() }),
            block: false,
        }
    }
    /// Matches calls whose argv starts with `prefix`.
    pub fn args<I, S>(prefix: I) -> Rule
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Rule { args: prefix.into_iter().map(Into::into).collect(), ..Rule::any() }
    }
    pub fn exit(mut self, code: i32) -> Self {
        self = self.with_output(|o| o.code = code);
        self
    }
    pub fn stdout(self, text: &str) -> Self {
        self.with_output(|o| o.stdout = text.into())
    }
    pub fn stderr(self, text: &str) -> Self {
        self.with_output(|o| o.stderr = text.into())
    }
    /// Fail like a spawn error: `CommandError::NotFound`, `Timeout` or `Other(..)`.
    pub fn error(mut self, e: CommandError) -> Self {
        self.result = Err(e);
        self
    }
    /// Matching calls wait until `Sim::release(program)`.
    pub fn block(mut self) -> Self {
        self.block = true;
        self
    }
    fn with_output(mut self, f: impl FnOnce(&mut CommandOutput)) -> Self {
        if let Ok(o) = &mut self.result {
            f(o);
        }
        self
    }
    pub(crate) fn matches(&self, argv: &[String]) -> bool {
        argv.starts_with(&self.args)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Shim {
    pub rules: Vec<Rule>,
    /// Set by `release`: blocked calls ran and later ones no longer block.
    pub released: bool,
}

/// A recorded call of a fake CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub program: String,
    pub args: Vec<String>,
    /// Working directory the app asked for (absolute; the app cwd when unset).
    pub cwd: String,
    /// Extra env the app asked for.
    pub env: Vec<(String, String)>,
}

impl Call {
    /// Is `prefix` the start of the argv?
    pub fn starts_with(&self, prefix: &[&str]) -> bool {
        self.args.len() >= prefix.len() && self.args.iter().zip(prefix).all(|(a, p)| a == p)
    }
}
