//! Shared single-line text input rules for the find, goto, search and comment-editor inputs.
//!
//! Spec: F-FIND-01, F-GOTO-01, F-SEARCH-02, F-COMMENT-02 (newline handling on typed and pasted text).
//! Owner: component `navops` (C), used by `comments` (D). Pure string helpers; the caller owns the field
//! and any caret. Must not know which overlay is open or touch `State`.

/// What happens to CR/LF in inserted text. Explicit per input: the inputs deliberately differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewlinePolicy {
    /// Each run of CR/LF becomes one space (find, comment editor).
    Collapse,
    /// CR/LF are removed (goto, file search).
    Drop,
}

/// `s` with newlines handled per `policy`.
pub fn sanitize(s: &str, policy: NewlinePolicy) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if c == '\n' || c == '\r' {
            if policy == NewlinePolicy::Collapse && !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            in_run = false;
            out.push(c);
        }
    }
    out
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i)
}

/// Insert `s` (sanitized) into `text` at char index `caret` and advance `caret`.
pub fn insert(text: &mut String, caret: &mut usize, s: &str, policy: NewlinePolicy) {
    let s = sanitize(s, policy);
    text.insert_str(byte_at(text, *caret), &s);
    *caret += s.chars().count();
}

/// Delete the char before `caret` and move `caret` back. Nothing at the start.
pub fn backspace(text: &mut String, caret: &mut usize) {
    if *caret == 0 {
        return;
    }
    let from = byte_at(text, *caret - 1);
    let to = byte_at(text, *caret);
    text.replace_range(from..to, "");
    *caret -= 1;
}

/// Append `s` (sanitized) at the end of an input without a caret.
pub fn push(text: &mut String, s: &str, policy: NewlinePolicy) {
    text.push_str(&sanitize(s, policy));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_policies() {
        assert_eq!(sanitize("a\r\n\nb\nc", NewlinePolicy::Collapse), "a b c");
        assert_eq!(sanitize("a\r\n\nb\nc", NewlinePolicy::Drop), "abc");
        assert_eq!(sanitize("\n", NewlinePolicy::Collapse), " ");
        assert_eq!(sanitize("\n", NewlinePolicy::Drop), "");
    }

    #[test]
    fn insert_and_backspace_at_caret_multibyte() {
        let mut t = "aé".to_string();
        let mut caret = 1;
        insert(&mut t, &mut caret, "xy", NewlinePolicy::Drop);
        assert_eq!((t.as_str(), caret), ("axyé", 3));
        backspace(&mut t, &mut caret);
        assert_eq!((t.as_str(), caret), ("axé", 2));
        caret = 0;
        backspace(&mut t, &mut caret);
        assert_eq!((t.as_str(), caret), ("axé", 0));
    }

    #[test]
    fn push_appends_sanitized() {
        let mut t = String::new();
        push(&mut t, "a\nb", NewlinePolicy::Collapse);
        push(&mut t, "\n", NewlinePolicy::Drop);
        assert_eq!(t, "a b");
    }
}
