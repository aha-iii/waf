//! 解码视图可信度：区分「真实编码载荷」与 API 签名/ID 等高熵噪声。

/// 是否应尝试 Base64 解码该 token（与具体参数名无关，只看 token 形态）。
pub fn is_plausible_b64_candidate(tok: &str) -> bool {
    let t = tok.trim_end_matches('=');
    if t.is_empty() {
        return false;
    }
    if t.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if t.len() >= 32 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    if t.len() >= 10 {
        let b = t.as_bytes();
        if b[0].is_ascii_uppercase() && b[1..].iter().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    if !t.contains('+') && !t.contains('/') && !t.ends_with('=') {
        if t.len() >= 8 && t.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
            return false;
        }
    }
    true
}

/// 解码结果是否像「可读文本载荷」，而非 hex/b64 误解码的随机字节。
pub fn is_plausible_decoded_text(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let s = match std::str::from_utf8(bytes) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if !is_plausible_inspect_text(s) {
        return false;
    }
    let n = bytes.len().min(4096);
    let mut printable = 0usize;
    for &b in bytes.iter().take(n) {
        if (0x20..=0x7e).contains(&b) || b == b'\n' || b == b'\r' || b == b'\t' {
            printable += 1;
        }
    }
    (printable as f32 / n as f32) >= 0.85
}

/// 是否应对解码视图 / 软信号做攻击形启发式。
pub fn is_plausible_inspect_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let n = bytes.len().min(8192);
    let mut ctrl = 0usize;
    let mut backtick = 0usize;
    for &b in bytes.iter().take(n) {
        if (b < 0x20 && !matches!(b, b'\n' | b'\r' | b'\t')) || b == 0x7f {
            ctrl += 1;
        }
        if b == b'`' {
            backtick += 1;
        }
    }
    if ctrl * 20 > n {
        return false;
    }
    if backtick >= 2 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_api_signature_tokens() {
        assert!(!is_plausible_b64_candidate(
            "05A6E5389357923F02BCC67FAA5036131ECCDC122950646D3A1178FD1F7E5976"
        ));
        assert!(!is_plausible_b64_candidate("1785289955109"));
        assert!(!is_plausible_b64_candidate("M1902534480440528898"));
    }

    #[test]
    fn keeps_real_b64_payloads() {
        assert!(is_plausible_b64_candidate("PGlmcmFtZQ=="));
    }
}
