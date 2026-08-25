use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_decode_order() -> Vec<String> {
    vec![
        "url".into(),
        "html_entity".into(),
        "base64".into(),
        "hex".into(),
        "unicode".into(),
        "chr".into(),
    ]
}

/// Decode policy: per-encoding on/off + execution order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodePolicy {
    pub max_depth: u8,
    pub max_expand_bytes: usize,
    #[serde(default = "default_true")]
    pub url: bool,
    #[serde(default = "default_true")]
    pub html_entity: bool,
    #[serde(default = "default_true")]
    pub base64: bool,
    #[serde(default)]
    pub hex: bool,
    #[serde(default)]
    pub unicode: bool,
    #[serde(default)]
    pub chr: bool,
    #[serde(default = "default_decode_order")]
    pub order: Vec<String>,
}

impl Default for DecodePolicy {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_expand_bytes: 65_536,
            url: true,
            html_entity: true,
            base64: true,
            hex: false,
            unicode: false,
            chr: false,
            order: default_decode_order(),
        }
    }
}

/// Compat legacy config field names.
pub type DecodeConfig = DecodePolicy;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodeLayer {
    pub depth: u8,
    pub op: String,
    pub text: String,
}

/// Recursively decode in policy order.
pub fn decode_chain(raw: &str, cfg: &DecodePolicy) -> Vec<DecodeLayer> {
    let mut layers = vec![DecodeLayer {
        depth: 0,
        op: "raw".into(),
        text: raw.to_string(),
    }];
    let mut current = raw.to_string();
    let mut depth = 0u8;

    while depth < cfg.max_depth {
        if current.len() > cfg.max_expand_bytes {
            break;
        }
        let mut advanced = false;

        for op in &cfg.order {
            if advanced {
                break;
            }
            let next = match op.as_str() {
                "url" if cfg.url => try_url_decode(&current),
                "html_entity" if cfg.html_entity => try_html_entities(&current),
                "base64" if cfg.base64 => try_base64(&current),
                "hex" if cfg.hex => try_hex(&current),
                "unicode" if cfg.unicode => try_unicode(&current),
                "chr" if cfg.chr => try_chr(&current),
                _ => None,
            };
            if let Some(next) = next {
                if next != current {
                    depth += 1;
                    current = next;
                    layers.push(DecodeLayer {
                        depth,
                        op: op.clone(),
                        text: current.clone(),
                    });
                    advanced = true;
                }
            }
        }

        if !advanced {
            break;
        }
    }

    layers
}

fn try_url_decode(s: &str) -> Option<String> {
    use nerve_waf_canonical::url_decode_uni;
    // Semantic `url` step: aligned with Antibody UrlDecodeUni; at most 3 iterations.
    let decoded = url_decode_uni(s, 3);
    if decoded != s {
        Some(decoded)
    } else {
        None
    }
}

fn try_html_entities(s: &str) -> Option<String> {
    if !s.contains('&') {
        return None;
    }
    use nerve_waf_canonical::html_entity_decode;
    let out = html_entity_decode(s);
    if out != s {
        Some(out)
    } else {
        None
    }
}

fn try_base64(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.len() < 8 || trimmed.len() > 4096 {
        return None;
    }
    if trimmed.starts_with('/') && trimmed.contains('/') {
        return None;
    }
    if !nerve_waf_canonical::is_plausible_b64_candidate(trimmed) {
        return None;
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    {
        return None;
    }
    if trimmed.len() % 4 != 0 {
        return None;
    }
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, trimmed)
        .or_else(|_| {
            base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, trimmed)
        })
        .ok()?;
    if decoded.is_empty() || decoded.len() > 8192 {
        return None;
    }
    let text = String::from_utf8_lossy(&decoded).into_owned();
    if text
        .chars()
        .filter(|c| c.is_control() && *c != '\n' && *c != '\r')
        .count()
        > 2
    {
        return None;
    }
    if text == s {
        return None;
    }
    Some(text)
}

fn try_hex(s: &str) -> Option<String> {
    if s.contains("\\x") || s.contains("\\X") {
        let mut out = String::with_capacity(s.len());
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut changed = false;
        while i < bytes.len() {
            if i + 3 < bytes.len()
                && (bytes[i] == b'\\')
                && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
                && bytes[i + 2].is_ascii_hexdigit()
                && bytes[i + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[i + 2..i + 4]).ok()?;
                let byte = u8::from_str_radix(hex, 16).ok()?;
                out.push(byte as char);
                i += 4;
                changed = true;
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        if changed && out != s {
            return Some(out);
        }
    }

    let stripped = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if stripped.len() >= 4
        && stripped.len() % 2 == 0
        && stripped.chars().all(|c| c.is_ascii_hexdigit())
        && !stripped.chars().any(|c| c.is_ascii_alphabetic() && c.to_ascii_lowercase() > 'f')
    {
        let mut bytes = Vec::with_capacity(stripped.len() / 2);
        for chunk in stripped.as_bytes().chunks(2) {
            let hex = std::str::from_utf8(chunk).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text != s && !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn try_unicode(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut changed = false;

    while i < bytes.len() {
        if i + 5 < bytes.len() && bytes[i] == b'\\' && bytes[i + 1] == b'u' {
            if bytes[i + 2] == b'{' {
                if let Some((hex, consumed)) = read_hex_run(&bytes[i + 3..]) {
                    if i + 3 + consumed < bytes.len() && bytes[i + 3 + consumed] == b'}' {
                        if let Some(ch) = hex_to_char(hex) {
                            out.push(ch);
                            i += 4 + consumed;
                            changed = true;
                            continue;
                        }
                    }
                }
            } else if let Some((hex, consumed)) = read_fixed_hex(&bytes[i + 2..], 4) {
                if let Some(ch) = hex_to_char(hex) {
                    out.push(ch);
                    i += 2 + consumed;
                    changed = true;
                    continue;
                }
            }
        }
        if i + 5 < bytes.len() && bytes[i] == b'%' && bytes[i + 1] == b'u' {
            if let Some((hex, consumed)) = read_fixed_hex(&bytes[i + 2..], 4) {
                if let Some(ch) = hex_to_char(hex) {
                    out.push(ch);
                    i += 2 + consumed;
                    changed = true;
                    continue;
                }
            }
        }
        if bytes[i] == b'&'
            && i + 4 < bytes.len()
            && bytes[i + 1] == b'#'
            && (bytes[i + 2] == b'x' || bytes[i + 2] == b'X')
        {
            if let Some((hex, consumed)) = read_hex_run(&bytes[i + 3..]) {
                if i + 3 + consumed < bytes.len() && bytes[i + 3 + consumed] == b';' {
                    if let Some(ch) = hex_to_char(hex) {
                        out.push(ch);
                        i += 4 + consumed;
                        changed = true;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    if changed && out != s {
        Some(out)
    } else {
        None
    }
}

fn read_fixed_hex(bytes: &[u8], n: usize) -> Option<(&str, usize)> {
    if bytes.len() < n || !bytes[..n].iter().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    std::str::from_utf8(&bytes[..n])
        .ok()
        .map(|h| (h, n))
}

fn read_hex_run(bytes: &[u8]) -> Option<(&str, usize)> {
    let mut n = 0;
    while n < bytes.len() && n < 6 && bytes[n].is_ascii_hexdigit() {
        n += 1;
    }
    if n == 0 {
        return None;
    }
    std::str::from_utf8(&bytes[..n]).ok().map(|h| (h, n))
}

fn hex_to_char(hex: &str) -> Option<char> {
    char::from_u32(u32::from_str_radix(hex, 16).ok()?)
}

fn try_chr(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    if !lower.contains("char(") && !lower.contains("chr(") {
        return None;
    }
    let mut out = s.to_string();
    let mut changed = false;
    let mut search = 0;
    while let Some(idx) = lower[search..].find("char(").or_else(|| lower[search..].find("chr(")) {
        let start = search + idx;
        let open = lower[start..].find('(')? + start;
        let close = lower[open..].find(')')? + open;
        let inner = &s[open + 1..close].trim();
        let code: u32 = if let Some(hex) = inner.strip_prefix("0x").or_else(|| inner.strip_prefix("0X"))
        {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            inner.parse().ok()?
        };
        if let Some(ch) = char::from_u32(code) {
            let full = &s[start..=close];
            out = out.replacen(full, &ch.to_string(), 1);
            changed = true;
        }
        search = close + 1;
    }
    if changed && out != s {
        Some(out)
    } else {
        None
    }
}

pub fn is_static_asset_path(uri: &str) -> bool {
    let path = uri.split('?').next().unwrap_or(uri).to_ascii_lowercase();
    [
        ".js", ".css", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".woff", ".woff2", ".ico",
        ".webp", ".map", ".ttf", ".eot",
    ]
    .iter()
    .any(|ext| path.ends_with(ext))
}

pub fn looks_suspicious(s: &str) -> bool {
    if s.len() < 3 {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    [
        "select", "union", "insert", "update", "delete", "drop", "sleep", "benchmark",
        "<script", "javascript:", "onerror", "onload", "../", "..\\", "cmd=", "exec(",
        "' or ", " or 1", "1=1", "0x", "char(", "waitfor", "/etc/passwd",
    ]
    .iter()
    .any(|m| lower.contains(m))
}
