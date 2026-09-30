//! Syntax highlighting: text lines -> token-class runs, by path or fence language. Colors come later
//! (`run_style`), so a theme change never re-parses.
//!
//! Spec: UNSPEC-31 (syntax colors are free but must not alter text), F-EDGE-08, F-ASK-06 (fence languages).
//! Oracle: `src/highlight.ts` (lang table, fence aliases). Owner: component `viewrows` (F2).
//! Uses syntect with the pure-Rust regex backend (feature `default-fancy`); syntax set loaded lazily in a
//! `OnceLock`. Must not: change the text, panic on odd input (fall back to plain), or touch state.
//!
//! Parsing runs off the render path (`hlcache` plans it, the app executor runs `highlight_lines` on a worker).
//! Within one call the parse state carries across lines; a range starting mid-file starts fresh (accepted:
//! rare wrong colors inside multi-line constructs that begin before the range).

use std::sync::{Arc, OnceLock};

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

use crate::screen::Color;
use crate::textutil::expand_tabs;
use crate::theme::{SyntaxClass, ThemeId, syntax_bold, syntax_color};

/// Extension -> language id (`langs` in highlight.ts).
const EXT_LANGS: &[(&str, &str)] = &[
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("mts", "typescript"),
    ("cts", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("json", "json"),
    ("md", "markdown"),
    ("css", "css"),
    ("html", "xml"),
    ("xml", "xml"),
    ("yml", "yaml"),
    ("yaml", "yaml"),
    ("sh", "bash"),
    ("bash", "bash"),
    ("zsh", "bash"),
    ("py", "python"),
    ("go", "go"),
    ("rs", "rust"),
    ("java", "java"),
    ("c", "c"),
    ("h", "c"),
    ("cpp", "cpp"),
    ("rb", "ruby"),
    ("sql", "sql"),
    ("toml", "ini"),
];

/// Fence-only aliases (`fenceAlias` in highlight.ts).
const FENCE_ALIASES: &[(&str, &str)] =
    &[("shell", "bash"), ("console", "bash"), ("golang", "go"), ("c++", "cpp"), ("yml", "yaml")];

/// Language ids a fence may name besides the extension table values (highlight.js names).
const EXTRA_LANGS: &[&str] = &[
    "php",
    "kotlin",
    "swift",
    "csharp",
    "diff",
    "dockerfile",
    "makefile",
    "lua",
    "perl",
    "r",
    "scala",
    "haskell",
    "objectivec",
    "groovy",
    "clojure",
    "erlang",
    "lisp",
    "latex",
    "graphql",
    "dart",
    "elixir",
    "ocaml",
    "powershell",
    "protobuf",
    "nginx",
    "http",
    "less",
    "scss",
    "vbnet",
    "fortran",
    "julia",
    "matlab",
    "awk",
    "coffeescript",
];

fn known_lang(name: &str) -> Option<&'static str> {
    EXT_LANGS.iter().map(|(_, l)| *l).chain(EXTRA_LANGS.iter().copied()).find(|l| *l == name)
}

/// Language id for a file path by extension (same table as `langs` in highlight.ts), `None` = plain.
pub fn language_for_path(path: &str) -> Option<&'static str> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    // text after the last `.` of the whole path (a dotless name maps to itself and misses the table)
    let ext = name.rsplit('.').next().unwrap_or(name).to_lowercase();
    if !name.contains('.') {
        return None;
    }
    EXT_LANGS.iter().find(|(e, _)| *e == ext).map(|(_, l)| *l)
}

/// Language id for a markdown fence info string (aliases like `shell`, `golang`), `None` when unknown.
pub fn language_for_fence(name: &str) -> Option<&'static str> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return None;
    }
    let l = EXT_LANGS
        .iter()
        .find(|(e, _)| *e == n)
        .map(|(_, l)| *l)
        .or_else(|| FENCE_ALIASES.iter().find(|(a, _)| *a == n).map(|(_, l)| *l))
        .unwrap_or(n.as_str());
    known_lang(l)
}

fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Syntect syntax for a language id; `None` when the bundled set lacks it.
fn syntax_for(lang: &str) -> Option<&'static SyntaxReference> {
    let set = syntax_set();
    let name = match lang {
        "typescript" | "javascript" | "coffeescript" => "JavaScript",
        "json" => "JSON",
        "markdown" => "Markdown",
        "css" => "CSS",
        "xml" => "HTML",
        "yaml" => "YAML",
        "bash" => "Bash",
        "python" => "Python",
        "go" => "Go",
        "rust" => "Rust",
        "java" => "Java",
        "c" => "C",
        "cpp" => "C++",
        "ruby" => "Ruby",
        "sql" => "SQL",
        "csharp" => "C#",
        "objectivec" => "Objective-C",
        "latex" => "LaTeX",
        "makefile" => "Makefile",
        "diff" => "Diff",
        "php" => "PHP",
        "lua" => "Lua",
        "perl" => "Perl",
        "r" => "R",
        "scala" => "Scala",
        "haskell" => "Haskell",
        "groovy" => "Groovy",
        "clojure" => "Clojure",
        "erlang" => "Erlang",
        "lisp" => "Lisp",
        "ocaml" => "OCaml",
        "awk" => "Awk",
        "graphql" => "GraphQL",
        _ => return set.find_syntax_by_token(lang),
    };
    set.find_syntax_by_name(name).or_else(|| set.find_syntax_by_token(lang))
}

/// Scope prefix -> class, most specific first; scanned per scope of the stack, innermost scope first.
fn class_table() -> &'static [(Scope, SyntaxClass)] {
    static TABLE: OnceLock<Vec<(Scope, SyntaxClass)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        use SyntaxClass::*;
        let raw: &[(&str, SyntaxClass)] = &[
            ("punctuation.definition.comment", Comment),
            ("comment", Comment),
            ("punctuation.definition.string", String),
            ("string", String),
            ("constant.numeric", Number),
            ("constant.character.escape", Literal),
            ("constant.language", Literal),
            ("support.constant", Literal),
            ("constant", Literal),
            ("punctuation.definition.preprocessor", Meta),
            ("meta.preprocessor", Meta),
            ("keyword.operator", Punctuation),
            ("keyword", Keyword),
            ("storage", Keyword),
            ("entity.name.function", Function),
            ("support.function", Function),
            ("variable.function", Function),
            ("entity.name.tag", Type),
            ("entity.name.type", Type),
            ("entity.name.class", Type),
            ("entity.name.struct", Type),
            ("entity.name.enum", Type),
            ("entity.name.trait", Type),
            ("entity.name.namespace", Type),
            ("support.type", Type),
            ("support.class", Type),
            ("entity.other.inherited-class", Type),
            ("entity.other.attribute-name", Attribute),
            ("variable.annotation", Attribute),
            ("meta.annotation", Attribute),
        ];
        raw.iter().filter_map(|(s, c)| Scope::new(s).ok().map(|s| (s, *c))).collect()
    })
}

fn punct_scope() -> Option<Scope> {
    static P: OnceLock<Option<Scope>> = OnceLock::new();
    *P.get_or_init(|| Scope::new("punctuation").ok())
}

fn classify(stack: &ScopeStack) -> Option<SyntaxClass> {
    let table = class_table();
    let punct = punct_scope();
    let mut fallback = None;
    for sc in stack.as_slice().iter().rev() {
        if let Some((_, class)) = table.iter().find(|(p, _)| p.is_prefix_of(*sc)) {
            return Some(*class);
        }
        if fallback.is_none() && punct.is_some_and(|p| p.is_prefix_of(*sc)) {
            fallback = Some(SyntaxClass::Punctuation);
        }
    }
    fallback
}

/// A run of `len` chars of the tab-expanded line text with one token class (`None` = default color).
/// Runs cover the whole text; an empty `Vec` means a plain (or blank) line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassRun {
    pub len: u32,
    pub class: Option<SyntaxClass>,
}

/// Runs of one line.
pub type LineRuns = Vec<ClassRun>;

/// Parser state saved at the end of a highlighted range, so the next range continues inside multi-line
/// constructs. Cheap to clone (shared); equality is identity.
#[derive(Clone)]
pub struct Carry(Arc<CarryInner>);

struct CarryInner {
    parse: ParseState,
    stack: ScopeStack,
}

impl std::fmt::Debug for Carry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Carry")
    }
}

impl PartialEq for Carry {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Lines longer than this (chars) are not parsed (minified files); they render plain.
const MAX_LINE_CHARS: usize = 4000;

fn push_run(out: &mut LineRuns, text: &str, class: Option<SyntaxClass>) {
    let len = text.chars().count() as u32;
    if len == 0 {
        return;
    }
    if let Some(last) = out.last_mut() {
        if last.class == class {
            last.len += len;
            return;
        }
    }
    out.push(ClassRun { len, class });
}

/// Parse one already tab-expanded line, advancing `parse`/`stack`. `None` = parser failure (state unusable).
fn line_runs(parse: &mut ParseState, stack: &mut ScopeStack, text: &str) -> Option<LineRuns> {
    let set = syntax_set();
    let line = format!("{text}\n");
    let ops = parse.parse_line(&line, set).ok()?;
    let mut out = LineRuns::new();
    let mut pos = 0usize;
    for (at, op) in &ops {
        let at = (*at).min(text.len());
        if at > pos {
            if !text.is_char_boundary(pos) || !text.is_char_boundary(at) {
                return None;
            }
            push_run(&mut out, &text[pos..at], classify(stack));
            pos = at;
        }
        stack.apply(op).ok()?;
    }
    if pos < text.len() {
        if !text.is_char_boundary(pos) {
            return None;
        }
        push_run(&mut out, &text[pos..], classify(stack));
    }
    if text.trim().is_empty() {
        return Some(LineRuns::new());
    }
    Some(out)
}

/// Highlight consecutive `lines` (raw text, tabs not yet expanded) of language `lang`, continuing from `carry`
/// (`None` = fresh parse state). Returns one [`LineRuns`] per line and the state after the last line. Unknown
/// language: all lines plain, no state. Never changes text; a parser failure resets the state and leaves
/// that line plain.
pub fn highlight_lines(
    lang: &str,
    lines: &[String],
    carry: Option<&Carry>,
) -> (Vec<LineRuns>, Option<Carry>) {
    let Some(syntax) = syntax_for(lang) else { return (vec![LineRuns::new(); lines.len()], None) };
    let fresh = || (ParseState::new(syntax), ScopeStack::new());
    let (mut parse, mut stack) = match carry {
        Some(c) => (c.0.parse.clone(), c.0.stack.clone()),
        None => fresh(),
    };
    let mut out = Vec::with_capacity(lines.len());
    for raw in lines {
        let text = expand_tabs(raw);
        if text.chars().count() > MAX_LINE_CHARS {
            out.push(LineRuns::new());
            continue;
        }
        match line_runs(&mut parse, &mut stack, &text) {
            Some(r) => out.push(r),
            None => {
                (parse, stack) = fresh();
                out.push(LineRuns::new());
            }
        }
    }
    (out, Some(Carry(Arc::new(CarryInner { parse, stack }))))
}

/// Runs of one standalone line (fresh state), used for thread code blocks.
pub fn highlight_one(lang: &str, text: &str) -> LineRuns {
    let (mut runs, _) = highlight_lines(lang, &[text.to_string()], None);
    runs.pop().unwrap_or_default()
}

/// Color and boldness of a class in `theme` (`None` class = default). The view maps classes to colors, so a
/// theme change needs no re-parse.
pub fn run_style(theme: ThemeId, class: Option<SyntaxClass>) -> (Option<Color>, bool) {
    let fg = class.and_then(|c| syntax_color(theme, c));
    (fg, syntax_bold(theme) && fg.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_for_path_table() {
        assert_eq!(language_for_path("src/a.tsx"), Some("typescript"));
        assert_eq!(language_for_path("a/B.RS"), Some("rust"));
        assert_eq!(language_for_path("Cargo.toml"), Some("ini"));
        assert_eq!(language_for_path("x.html"), Some("xml"));
        assert_eq!(language_for_path("x.zsh"), Some("bash"));
        assert_eq!(language_for_path("Makefile"), None);
        assert_eq!(language_for_path("a.b/noext"), None);
        assert_eq!(language_for_path("x.unknown"), None);
    }

    #[test]
    fn f_ask_06_language_for_fence() {
        assert_eq!(language_for_fence("ts"), Some("typescript"));
        assert_eq!(language_for_fence("TS"), Some("typescript"));
        assert_eq!(language_for_fence("shell"), Some("bash"));
        assert_eq!(language_for_fence("console"), Some("bash"));
        assert_eq!(language_for_fence("golang"), Some("go"));
        assert_eq!(language_for_fence("c++"), Some("cpp"));
        assert_eq!(language_for_fence("yml"), Some("yaml"));
        assert_eq!(language_for_fence("python"), Some("python"));
        assert_eq!(language_for_fence("php"), Some("php"));
        assert_eq!(language_for_fence("nonsense"), None);
        assert_eq!(language_for_fence(""), None);
    }

    fn one(lang: &str, text: &str) -> LineRuns {
        highlight_one(lang, text)
    }

    fn total(runs: &LineRuns) -> usize {
        runs.iter().map(|r| r.len as usize).sum()
    }

    #[test]
    fn unknown_or_blank_is_plain() {
        assert!(one("nonsense", "x").is_empty());
        assert!(one("rust", "   ").is_empty());
        assert!(one("rust", "").is_empty());
    }

    #[test]
    fn unxplain_31_runs_cover_text() {
        let samples: &[(&str, &str)] = &[
            ("rust", "fn main() { let s = \"h\u{e9}llo\"; // c"),
            ("typescript", "const a: number = 1 + 2; /* x */ `t${a}`"),
            ("python", "def f(x): return 'a' + \"b\"  # \u{65e5}\u{672c}\u{8a9e}"),
            ("go", "func (s *S) Run() error { return nil }"),
            ("bash", "if [ -f \"$1\" ]; then echo $(ls); fi"),
            ("json", "{\"a\": [1, 2.5, true, null]}"),
            ("yaml", "key: value # c"),
            ("markdown", "# Title `code` **bold**"),
            ("css", "a.b > c { color: #fff; }"),
            ("xml", "<div class=\"x\">text &amp;</div>"),
            ("sql", "SELECT a, b FROM t WHERE c = 'x';"),
            ("ruby", "def foo; puts 'x'; end"),
            ("c", "#include <stdio.h>"),
            ("cpp", "std::vector<int> v{1,2};"),
            ("java", "public static void main(String[] a) {}"),
            ("ini", "[a]"),
            ("rust", "\t\ttabs\tinside \u{1F600} wide \u{65e5}\u{672c}"),
        ];
        for (lang, line) in samples {
            let runs = one(lang, line);
            if !runs.is_empty() {
                assert_eq!(total(&runs), expand_tabs(line).chars().count(), "{lang}");
            }
            assert!(runs.iter().all(|r| r.len > 0));
        }
    }

    #[test]
    fn rust_line_gets_classes() {
        let runs = one("rust", "fn main() { let s = \"x\"; }");
        assert!(runs.len() > 1);
        assert!(runs.iter().any(|r| r.class == Some(SyntaxClass::Keyword)));
        assert!(runs.iter().any(|r| r.class == Some(SyntaxClass::String)));
    }

    #[test]
    fn theme_maps_class_to_color_without_reparse() {
        let runs = one("rust", "fn x() {}");
        let kw = runs.iter().find(|r| r.class == Some(SyntaxClass::Keyword)).map(|r| r.class);
        let a = run_style(ThemeId::Solarized, kw.flatten());
        let b = run_style(ThemeId::Vibrant, kw.flatten());
        assert!(a.0.is_some() && b.0.is_some());
        assert_ne!(a.0, b.0);
        assert_eq!(run_style(ThemeId::Light, None), (None, false));
    }

    #[test]
    fn carry_continues_block_comment_across_ranges() {
        let first = vec!["/* start".to_string(), "still".to_string()];
        let (r1, carry) = highlight_lines("rust", &first, None);
        assert!(r1[1].iter().all(|r| r.class == Some(SyntaxClass::Comment)));
        let (with, _) = highlight_lines("rust", &["end */ let x = 1;".to_string()], carry.as_ref());
        let (fresh, _) = highlight_lines("rust", &["end */ let x = 1;".to_string()], None);
        assert_eq!(with[0].first().map(|r| r.class), Some(Some(SyntaxClass::Comment)));
        assert_ne!(with, fresh);
    }

    #[test]
    fn multi_line_state_within_one_call() {
        let lines: Vec<String> = ["/*", "x", "*/ fn"].iter().map(|s| s.to_string()).collect();
        let (r, _) = highlight_lines("rust", &lines, None);
        assert!(r[1].iter().all(|x| x.class == Some(SyntaxClass::Comment)));
    }

    #[test]
    fn odd_input_never_panics() {
        let odd = "\u{0}\u{1b}[31m\r\u{feff}\"unterminated /* `";
        for lang in ["rust", "python", "markdown", "xml", "bash", "yaml"] {
            let runs = one(lang, odd);
            if !runs.is_empty() {
                assert_eq!(total(&runs), odd.chars().count());
            }
        }
    }

    #[test]
    fn very_long_line_is_plain() {
        let long = "a".repeat(MAX_LINE_CHARS + 1);
        assert!(one("rust", &long).is_empty());
    }
}
