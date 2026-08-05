//! Base64 展开快速门闩：无 b64 特征时跳过 regex 扫描。

use crate::plausibility::is_plausible_b64_candidate;

/// 文本是否可能含需展开的 base64 载荷。
pub fn text_may_contain_b64_payload(text: &str) -> bool {
    if text.len() < 8 {
        return false;
    }
    if !(text.contains('+') || text.contains('/') || text.contains('=')) {
        return false;
    }
    for part in text.split('&') {
        if let Some((_k, val)) = part.split_once('=') {
            if candidate_looks_like_b64(val) {
                return true;
            }
        }
    }
    for tok in split_b64_tokens(text) {
        if candidate_looks_like_b64(tok) {
            return true;
        }
    }
    false
}

fn candidate_looks_like_b64(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 8 {
        return false;
    }
    let has_b64_markers = t.contains('+') || t.contains('/') || t.ends_with('=');
    if !has_b64_markers {
        return false;
    }
    if t.len() % 4 != 0 && !t.contains('+') && !t.contains('/') {
        return false;
    }
    is_plausible_b64_candidate(t)
}

fn split_b64_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| {
        !c.is_ascii_alphanumeric() && c != '+' && c != '/' && c != '-' && c != '_' && c != '='
    })
    .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_plain_query() {
        assert!(!text_may_contain_b64_payload(
            "tenant=coretest1&mchOrderNo=M1902534480440528898"
        ));
    }

    #[test]
    fn detects_embedded_b64() {
        assert!(text_may_contain_b64_payload("x=PGlmcmFtZQ=="));
        assert!(text_may_contain_b64_payload("name=PGlmcmFtZQ==&x=1"));
    }
}
