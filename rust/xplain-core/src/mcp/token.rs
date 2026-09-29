//! Token and port helpers (pure).
//!
//! Spec: F-MCPSRV-01 (token file reuse rule, base64url 43 chars from 32 random bytes), Test seams
//! (`XPLAIN_MCP_PORT`), F-INTEG-05/06 (masking the token in shown text). Owner: component `agent` (E).
//! The boundary fns `parse_port` / `plan_token` live in `mcp/mod.rs` and delegate here.

use super::TokenPlan;

/// Default listen port (Test seams).
pub const DEFAULT_PORT: u16 = 47615;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding.
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        let idx = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        let count = chunk.len() + 1;
        for &i in idx.iter().take(count) {
            out.push(B64[i as usize] as char);
        }
    }
    out
}

/// Replace every occurrence of `token` in `text` by `***` for display (F-INTEG-05, F-MCPUI-01).
pub fn mask(text: &str, token: &str) -> String {
    if token.is_empty() {
        return text.to_string();
    }
    text.replace(token, "***")
}

/// Contents of `mcp.json` for a token: `{"token": "<t>"}` as 2-space JSON plus `\n`.
pub fn file_contents(token: &str) -> String {
    let v = serde_json::json!({ "token": token });
    let mut s = serde_json::to_string_pretty(&v).unwrap_or_else(|_| String::from("{}"));
    s.push('\n');
    s
}

/// `XPLAIN_MCP_PORT`: unset/empty -> default; else 1-5 ASCII digits <= 65535.
pub fn parse_port(raw: Option<&str>) -> Result<u16, String> {
    let v = match raw {
        None | Some("") => return Ok(DEFAULT_PORT),
        Some(v) => v,
    };
    let ok = (1..=5).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_digit());
    if ok {
        if let Ok(n) = u16::try_from(v.parse::<u32>().unwrap_or(u32::MAX)) {
            return Ok(n);
        }
    }
    let quoted = serde_json::to_string(v).unwrap_or_else(|_| format!("\"{v}\""));
    Err(format!("invalid XPLAIN_MCP_PORT {quoted} (0-65535)"))
}

/// Reuse a string `token` of length >= 16 from the file, else new token from `random`.
pub fn plan_token(existing_file: Option<&str>, random: [u8; 32]) -> TokenPlan {
    let parsed = existing_file.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok());
    if let Some(serde_json::Value::String(t)) = parsed.as_ref().and_then(|v| v.get("token")) {
        if t.encode_utf16().count() >= 16 {
            return TokenPlan::Reuse(t.clone());
        }
    }
    let token = base64url(&random);
    let file_contents = file_contents(&token);
    TokenPlan::Write { token, file_contents }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_mcpsrv_01_base64url_vectors() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(&[0xfb, 0xff, 0xfe]), "-__-");
        assert_eq!(base64url(&[0u8; 32]).len(), 43);
    }

    #[test]
    fn f_mcpsrv_01_plan_reuse_and_write() {
        let long = "0123456789abcdef";
        assert_eq!(
            plan_token(Some(&format!("{{\"token\":\"{long}\",\"port\":1}}")), [1; 32]),
            TokenPlan::Reuse(long.to_string())
        );
        for bad in
            [None, Some(""), Some("nope"), Some("{\"token\":\"short\"}"), Some("{\"token\":5}"), Some("[]")]
        {
            match plan_token(bad, [7; 32]) {
                TokenPlan::Write { token, file_contents } => {
                    assert_eq!(token.len(), 43);
                    assert_eq!(file_contents, format!("{{\n  \"token\": \"{token}\"\n}}\n"));
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn test_seams_parse_port() {
        assert_eq!(parse_port(None), Ok(47615));
        assert_eq!(parse_port(Some("")), Ok(47615));
        assert_eq!(parse_port(Some("0")), Ok(0));
        assert_eq!(parse_port(Some("65535")), Ok(65535));
        assert_eq!(parse_port(Some("0080")), Ok(80));
        for bad in ["65536", "-1", "abc", "1e3", " 80", "123456", "8 0"] {
            assert_eq!(parse_port(Some(bad)), Err(format!("invalid XPLAIN_MCP_PORT \"{bad}\" (0-65535)")));
        }
    }

    #[test]
    fn f_integ_05_mask() {
        assert_eq!(mask("a tok b tok", "tok"), "a *** b ***");
        assert_eq!(mask("abc", ""), "abc");
    }
}
