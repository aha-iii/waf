//! Human challenge tokens plus challenge-page HTML.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub const CHALLENGE_COOKIE: &str = "nerve_challenge";
pub const CHALLENGE_HEADER: &str = "x-nerve-challenge-token";

pub struct ChallengeStore {
    tokens: RwLock<HashMap<String, Instant>>,
    ttl: Duration,
}

impl ChallengeStore {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn issue(&self, client_key: &str) -> String {
        let token = format!("nc_{:x}", rand_hash(client_key));
        self.tokens
            .write()
            .unwrap()
            .insert(token.clone(), Instant::now());
        token
    }

    pub fn verify(&self, token: &str) -> bool {
        let mut tokens = self.tokens.write().unwrap();
        if let Some(ts) = tokens.get(token) {
            if ts.elapsed() < self.ttl {
                tokens.remove(token);
                return true;
            }
            tokens.remove(token);
        }
        false
    }

    /// Extract verified token from Cookie / Header string (format check only; verify is API-layer).
    pub fn extract_token(headers: &str, cookie_header: Option<&str>) -> Option<String> {
        if let Some(h) = headers.lines().find(|l| l.to_lowercase().starts_with(&format!("{CHALLENGE_HEADER}:"))) {
            let t = h.splitn(2, ':').nth(1)?.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        if let Some(cookie) = cookie_header {
            for part in cookie.split(';') {
                let part = part.trim();
                if let Some(v) = part.strip_prefix(&format!("{CHALLENGE_COOKIE}=")) {
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }

    /// Return JS challenge page embeddable in Gateway 403 responses.
    pub fn challenge_html(waf_api: &str, return_url: &str, client_key: &str) -> String {
        format!(
            r#"<!DOCTYPE html><html><head><meta charsett="utf-8"><title>Security Check</title></head>
<body><h3>Nerve WAF 安全验证</h3><p>请完成验证后继续访问。</p>
<script>
(function(){{
  var api="{waf_api}".replace(/\/$/,'');
  var key="{client_key}";
  fetch(api+"/api/challenge/issue",{{method:"POST",headers:{{"Content-Type":"application/json"}},body:JSON.stringify({{client_key:key}})}})
    .then(function(r){{return r.json();}})
    .then(function(d){{
      document.cookie="{cookie}="+d.token+"; path=/; max-age=300";
      setTimeout(function(){{ window.location.href="{return_url}"; }}, 800);
    }});
}})();
</script></body></html>"#,
            waf_api = waf_api,
            client_key = client_key.replace('"', ""),
            return_url = return_url.replace('"', "%22"),
            cookie = CHALLENGE_COOKIE,
        )
    }
}

fn rand_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    Instant::now().hash(&mut h);
    h.finish()
}

impl Default for ChallengeStore {
    fn default() -> Self {
        Self::new(300)
    }
}
