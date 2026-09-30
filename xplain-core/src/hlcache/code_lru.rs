//! Small LRU of highlighted thread code lines, keyed by (language, text).

use std::collections::{HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::highlight::{ClassRun, LineRuns, highlight_one};

/// Thread code lines kept.
pub(super) const CODE_CAP: usize = 500;

#[derive(Debug, Clone)]
struct CodeEntry {
    lang: &'static str,
    text: String,
    runs: LineRuns,
}

#[derive(Debug, Default)]
pub(super) struct CodeLru {
    entries: HashMap<u64, CodeEntry>,
    /// Keys by recency, most recent last.
    order: VecDeque<u64>,
}

fn code_hash(lang: &str, text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    lang.hash(&mut h);
    text.hash(&mut h);
    h.finish()
}

impl CodeLru {
    pub fn get(&self, lang: &str, text: &str) -> Option<&[ClassRun]> {
        let e = self.entries.get(&code_hash(lang, text))?;
        (e.text == text && e.lang == lang).then_some(e.runs.as_slice())
    }

    /// Highlight and store a line (or refresh its recency); evicts the least recent beyond [`CODE_CAP`].
    pub fn insert(&mut self, lang: &'static str, text: &str) {
        let h = code_hash(lang, text);
        let hit = self.entries.get(&h).is_some_and(|e| e.text == text && e.lang == lang);
        if !hit {
            let runs = highlight_one(lang, text);
            self.entries.insert(h, CodeEntry { lang, text: text.to_string(), runs });
        }
        self.order.retain(|k| *k != h);
        self.order.push_back(h);
        while self.order.len() > CODE_CAP {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_code_lines_cached_once_with_lru() {
        let mut c = CodeLru::default();
        for i in 0..(CODE_CAP + 20) {
            c.insert("rust", &format!("let x{i} = 1;"));
        }
        assert_eq!(c.len(), CODE_CAP);
        assert!(c.get("rust", "let x0 = 1;").is_none(), "oldest evicted");
        assert!(c.get("rust", &format!("let x{} = 1;", CODE_CAP + 19)).is_some());
        // touching keeps an entry alive
        c.insert("rust", "let x25 = 1;");
        for i in 0..CODE_CAP - 2 {
            c.insert("rust", &format!("y{i}"));
        }
        assert!(c.get("rust", "let x25 = 1;").is_some());
        // keyed by language and text
        assert!(c.get("python", "let x25 = 1;").is_none());
    }
}
