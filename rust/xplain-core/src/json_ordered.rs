//! Order-preserving JSON value with a JS-compatible parser and printer.
//!
//! Spec: F-CONFIG-03 (parse error messages, warning text via `JSON.stringify`), F-CONFIG-05 (save keeps the
//! user's key order, tab indent). Oracle: JS `JSON.parse` / `JSON.stringify`. Pure, no state.
//! Must not: know about config keys (config.rs owns those).

/// Order-preserving JSON value (JS object semantics: insertion order, duplicate key keeps first position,
/// last value). serde_json's map sorts keys, which would reorder the user's file on save.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 256;

impl Json {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn from_value(v: &serde_json::Value) -> Json {
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

    pub(crate) fn parse(text: &str) -> Result<Json, String> {
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
    pub(crate) fn stringify(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, None, 0);
        s
    }

    pub(crate) fn write_pretty(&self, out: &mut String, depth: usize) {
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

    #[test]
    fn keeps_insertion_order_and_last_duplicate() {
        let v = Json::parse(r#"{"b":1,"a":2,"b":3}"#).unwrap();
        assert_eq!(v.stringify(), r#"{"b":3,"a":2}"#);
    }

    #[test]
    fn pretty_uses_tabs_and_empty_containers() {
        let v = Json::parse(r#"{"a":[1,{}],"b":[]}"#).unwrap();
        let mut out = String::new();
        v.write_pretty(&mut out, 0);
        assert_eq!(out, "{\n\t\"a\": [\n\t\t1,\n\t\t{}\n\t],\n\t\"b\": []\n}");
    }

    #[test]
    fn numbers_print_like_js() {
        assert_eq!(js_number(1.0), "1");
        assert_eq!(js_number(-0.0), "0");
        assert_eq!(js_number(1.5e-7), "1.5e-7");
        assert_eq!(js_number(1e21), "1e+21");
        assert_eq!(js_number(f64::NAN), "null");
    }

    #[test]
    fn parse_errors_name_the_token() {
        assert_eq!(Json::parse("").unwrap_err(), "Unexpected end of JSON input");
        assert_eq!(Json::parse("{x").unwrap_err(), "Unexpected token x in JSON at position 1");
        assert_eq!(Json::parse("[1,]").unwrap_err(), "Unexpected token ] in JSON at position 3");
        assert!(Json::parse(&"[".repeat(300)).is_err());
    }

    #[test]
    fn string_escapes_round_trip() {
        let v = Json::parse(r#""a\n\u0001é""#).unwrap();
        assert_eq!(v.stringify(), "\"a\\n\\u0001\u{e9}\"");
    }
}
