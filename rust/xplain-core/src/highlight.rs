//! Syntax highlighting: text line -> colored spans, by path or fence language, per theme.
//!
//! Spec: UNSPEC-31 (syntax colors are free but must not alter text), F-EDGE-08, F-ASK-06 (fence languages).
//! Oracle: `src/highlight.ts` (lang table, fence aliases). Owner: component `viewrows` (F2).
//! Uses syntect with the pure-Rust regex backend (feature `default-fancy`); syntax set loaded lazily in a
//! `OnceLock`. Must not: change the text, panic on odd input (fall back to plain), or touch state.
//!
//! Lines are highlighted one at a time with a fresh parse state (like the oracle), so multi-line constructs
//! (block comments, heredocs) are colored only where they start inside the line.

use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

use crate::screen::Color;
use crate::theme::{SyntaxClass, ThemeId, syntax_bold, syntax_color};

/// One highlighted span; concatenated `text` equals the input line exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlSpan {
    pub text: String,
    pub fg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
}

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
    // oracle: text after the last `.` of the whole path (a dotless name maps to itself and misses the table)
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

fn plain(text: &str) -> Vec<HlSpan> {
    vec![HlSpan { text: text.to_string(), fg: None, bold: false, italic: false }]
}

fn push_span(out: &mut Vec<HlSpan>, text: &str, fg: Option<Color>, bold: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut() {
        if last.fg == fg && last.bold == bold {
            last.text.push_str(text);
            return;
        }
    }
    out.push(HlSpan { text: text.to_string(), fg, bold, italic: false });
}

fn try_highlight(text: &str, lang: &str, theme: ThemeId) -> Option<Vec<HlSpan>> {
    let syntax = syntax_for(lang)?;
    let set = syntax_set();
    let mut state = ParseState::new(syntax);
    let line = format!("{text}\n");
    let ops = state.parse_line(&line, set).ok()?;
    let bold_on = syntax_bold(theme);
    let mut stack = ScopeStack::new();
    let mut out: Vec<HlSpan> = Vec::new();
    let mut pos = 0usize;
    let emit = |stack: &ScopeStack, from: usize, to: usize, out: &mut Vec<HlSpan>| {
        let to = to.min(text.len());
        if from >= to || !text.is_char_boundary(from) || !text.is_char_boundary(to) {
            return;
        }
        let fg = classify(stack).and_then(|c| syntax_color(theme, c));
        push_span(out, &text[from..to], fg, bold_on && fg.is_some());
    };
    for (at, op) in &ops {
        let at = (*at).min(text.len());
        if at > pos {
            emit(&stack, pos, at, &mut out);
            pos = at;
        }
        stack.apply(op).ok()?;
    }
    if pos < text.len() {
        emit(&stack, pos, text.len(), &mut out);
    }
    let joined: usize = out.iter().map(|s| s.text.len()).sum();
    if joined != text.len() {
        return None;
    }
    Some(out)
}

/// Highlight one line. Blank input or unknown language -> one plain span.
pub fn highlight_line(text: &str, lang: Option<&str>, theme: ThemeId) -> Vec<HlSpan> {
    let Some(lang) = lang else { return plain(text) };
    if text.trim().is_empty() {
        return plain(text);
    }
    match try_highlight(text, lang, theme) {
        Some(spans) if !spans.is_empty() => spans,
        _ => plain(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(spans: &[HlSpan]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

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

    #[test]
    fn unknown_or_blank_is_plain() {
        for t in ThemeId::ALL {
            assert_eq!(highlight_line("let x = 1;", None, t), plain("let x = 1;"));
            assert_eq!(highlight_line("   ", Some("rust"), t), plain("   "));
            assert_eq!(highlight_line("", Some("rust"), t), plain(""));
            assert_eq!(highlight_line("x", Some("nonsense"), t), plain("x"));
        }
    }

    #[test]
    fn unxplain_31_text_roundtrip() {
        let samples: &[(&str, &str)] = &[
            ("rust", "fn main() { let s = \"héllo\"; // c\n"),
            ("typescript", "const a: number = 1 + 2; /* x */ `t${a}`"),
            ("python", "def f(x): return 'a' + \"b\"  # 日本語"),
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
            ("ini", "[a]\nk = v"),
            ("rust", "\t\ttabs\tinside \u{1F600} wide 日本"),
        ];
        for theme in ThemeId::ALL {
            for (lang, line) in samples {
                let spans = highlight_line(line, Some(lang), theme);
                assert_eq!(joined(&spans), *line, "{lang}");
                assert!(spans.iter().all(|s| !s.text.is_empty()) || line.is_empty());
            }
        }
    }

    #[test]
    fn rust_line_gets_colors() {
        let spans = highlight_line("fn main() { let s = \"x\"; }", Some("rust"), ThemeId::Solarized);
        assert!(spans.len() > 1);
        assert!(spans.iter().any(|s| s.fg.is_some()));
    }

    #[test]
    fn odd_input_never_panics() {
        let odd = "\u{0}\u{1b}[31m\r\u{feff}\"unterminated /* `";
        for lang in ["rust", "python", "markdown", "xml", "bash", "yaml"] {
            let spans = highlight_line(odd, Some(lang), ThemeId::Light);
            assert_eq!(joined(&spans), odd);
        }
    }
}
