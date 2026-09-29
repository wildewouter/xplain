//! Body line model and rich text: fenced code blocks become copy-button + gutter lines, prose is wrapped (F-ASK-06/08).

use super::{CODE_GUTTER, COPY_BTN};
use crate::comments::AnswerStatus;
use crate::textutil;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Msg,
    Fu,
    Div,
    Ans,
    Btn,
    Code,
}

/// One display line of a thread body (`BodyLine` in answerView.ts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyLine {
    pub text: String,
    pub kind: BodyKind,
    pub err: bool,
    /// Placeholder line of a live answer without text (spinner prefix).
    pub live: Option<AnswerStatus>,
    /// Button / code lines: fence language.
    pub lang: Option<String>,
    /// Button / code lines: code block index within the thread.
    pub blk: usize,
    /// Button lines: raw block text (copied).
    pub code: Option<String>,
}

pub(super) fn plain_line(text: String, kind: BodyKind) -> BodyLine {
    BodyLine { text, kind, err: false, live: None, lang: None, blk: 0, code: None }
}

/// Fence open: optional leading whitespace, 3+ backticks or tildes, optional lang (first word).
pub(super) fn fence_open(line: &str) -> Option<(char, usize, Option<String>)> {
    let t = line.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|&x| x == c).count();
    if n < 3 {
        return None;
    }
    let rest: String = t.chars().skip(n).collect();
    let lang: String = rest.trim_start().chars().take_while(|x| !x.is_whitespace() && *x != '`').collect();
    Some((c, n, (!lang.is_empty()).then_some(lang)))
}

/// Fence close: same char, length >= open, only whitespace around.
pub(super) fn fence_close(line: &str, c: char, n: usize) -> bool {
    let t = line.trim();
    t.chars().count() >= n && t.chars().all(|x| x == c)
}

/// Prose wrapped as usual; each fenced block becomes a copy button line plus hard-cut code lines
/// (fences not shown) (F-ASK-06). `blk` numbers blocks across the whole thread.
pub fn rich_lines(text: &str, width: usize, kind: BodyKind, blk: &mut usize) -> Vec<BodyLine> {
    let mut out = Vec::new();
    let mut prose: Vec<String> = Vec::new();
    let flush = |prose: &mut Vec<String>, out: &mut Vec<BodyLine>| {
        if !prose.is_empty() {
            for t in textutil::wrap_text(&prose.join("\n"), width) {
                out.push(plain_line(t, kind));
            }
        }
        prose.clear();
    };
    let src: Vec<String> = text.replace('\r', "").split('\n').map(str::to_string).collect();
    let mut i = 0;
    while i < src.len() {
        let Some((fc, n, lang)) = fence_open(&src[i]) else {
            prose.push(src[i].clone());
            i += 1;
            continue;
        };
        flush(&mut prose, &mut out);
        let mut code: Vec<&str> = Vec::new();
        i += 1;
        while i < src.len() && !fence_close(&src[i], fc, n) {
            code.push(&src[i]);
            i += 1;
        }
        i += 1; // closing fence (or past the end)
        let idx = *blk;
        *blk += 1;
        let label = match &lang {
            Some(l) => format!("{COPY_BTN} {l}"),
            None => COPY_BTN.to_string(),
        };
        out.push(BodyLine {
            text: label,
            kind: BodyKind::Btn,
            err: false,
            live: None,
            lang: lang.clone(),
            blk: idx,
            code: Some(code.join("\n")),
        });
        let cw = width.saturating_sub(CODE_GUTTER).max(1);
        for raw in code {
            let chars: Vec<char> = raw.replace('\t', "  ").chars().collect();
            let mut j = 0;
            while j == 0 || j < chars.len() {
                let from = j.min(chars.len());
                let end = (j + cw).min(chars.len());
                out.push(BodyLine {
                    text: chars[from..end].iter().collect(),
                    kind: BodyKind::Code,
                    err: false,
                    live: None,
                    lang: lang.clone(),
                    blk: idx,
                    code: None,
                });
                j += cw;
            }
        }
    }
    flush(&mut prose, &mut out);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::comments::AnswerStatus;
    use crate::comments::testutil::{answer, comment};
    use crate::thread_layout::fixtures::*;
    use crate::thread_layout::*;

    #[test]
    fn f_ask_06_code_blocks_button_and_gutter_lines() {
        let mut n = 0;
        let l = rich_lines("intro\n```ts title\nlet a = 1;\n\n\tx\n```\nafter", 30, BodyKind::Msg, &mut n);
        let t: Vec<_> = l.iter().map(|x| (x.kind, x.text.as_str())).collect();
        assert_eq!(
            t,
            [
                (BodyKind::Msg, "intro"),
                (BodyKind::Btn, "[ copy ] ts"),
                (BodyKind::Code, "let a = 1;"),
                (BodyKind::Code, ""),
                (BodyKind::Code, "  x"),
                (BodyKind::Msg, "after"),
            ]
        );
        assert_eq!(l[1].code.as_deref(), Some("let a = 1;\n\n\tx"), "raw text, tabs kept");
        assert_eq!(l[2].lang.as_deref(), Some("ts"));
        assert_eq!(n, 1);
    }

    #[test]
    fn f_ask_06_fence_rules_close_length_tilde_and_unclosed() {
        let mut n = 0;
        let l = rich_lines("````\na\n```\nb\n````\nz", 30, BodyKind::Ans, &mut n);
        let code: Vec<_> = l.iter().filter(|x| x.kind == BodyKind::Code).map(|x| x.text.as_str()).collect();
        assert_eq!(code, ["a", "```", "b"], "shorter fence does not close");
        let mut n = 0;
        let l = rich_lines("~~~\nx\n~~~ \ny", 30, BodyKind::Ans, &mut n);
        assert_eq!(l.iter().filter(|x| x.kind == BodyKind::Btn).count(), 1);
        assert_eq!(l.last().map(|x| x.text.as_str()), Some("y"));
        let mut n = 0;
        let l = rich_lines("```\nopen\nforever", 30, BodyKind::Ans, &mut n);
        assert_eq!(l.iter().filter(|x| x.kind == BodyKind::Code).count(), 2, "no close: to the end");
        let mut n = 0;
        let l = rich_lines("``not a fence", 30, BodyKind::Ans, &mut n);
        assert!(l.iter().all(|x| x.kind == BodyKind::Ans));
    }

    #[test]
    fn f_ask_06_long_code_line_split_not_wrapped() {
        let mut n = 0;
        let l = rich_lines("```\n0123456789abc def\n```", 10, BodyKind::Msg, &mut n);
        let code: Vec<_> = l.iter().filter(|x| x.kind == BodyKind::Code).map(|x| x.text.as_str()).collect();
        assert_eq!(code, ["01234567", "89abc de", "f"], "width-2 chars per line");
    }

    #[test]
    fn f_ask_06_blocks_numbered_across_thread() {
        let mut c = comment("q1", 1, 1, "```\na\n```");
        c.turns[0].answer = Some(answer(AnswerStatus::Done, "```\nb\n```"));
        let body = thread_body(&c, 40);
        let blks: Vec<_> = body.iter().filter(|l| l.kind == BodyKind::Btn).map(|l| l.blk).collect();
        assert_eq!(blks, [0, 1]);
        assert_eq!(block_text(&c, 1).as_deref(), Some("b"));
        assert_eq!(block_text(&c, 2), None);
        assert_eq!(block_count(&st(), &c), 2);
    }
}
