//! Local username/password login: issue HS256 JWT; no Keycloak dependency.

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

const ISSUER: &str = "nerve-local";

fn local_auth_enabled_from_env() -> bool {
    if let Ok(mode) = std::env::var("NERVE_AUTH_MODE") {
        return mode.trim().eq_ignore_ascii_case("local");
    }
    match std::env::var("NERVE_AUTH_LOCAL_ENABLED") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "0" | "false" | "no" | "off")
        }
        Err(_) => true,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalPublicConfig {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub role: String,
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LocalClaims {
    sub: String,
    role: String,
    exp: usize,
    iss: String,
}

pub struct LocalAuth {
    jwt_secret: Vec<u8>,
    token_ttl_secs: u64,
}

impl LocalAuth {
    pub fn from_env() -> Option<Self> {
        if !local_auth_enabled_from_env() {
            return None;
        }
        let jwt_secret = std::env::var("NERVE_AUTH_JWT_SECRET")
            .ok()
            .map(|s| s.into_bytes())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                warn!(
                    "nerve-waf-auth: NERVE_AUTH_JWT_SECRET unset — using dev-only default; set a strong secret in production"
                );
                b"nerve-waf-local-dev-secret-change-me".to_vec()
            });
        let token_ttl_secs = std::env::var("NERVE_AUTH_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86_400);
        Some(Self {
            jwt_secret,
            token_ttl_secs,
        })
    }

    pub fn public_config(&self) -> LocalPublicConfig {
        LocalPublicConfig { enabled: true }
    }

    pub fn issue_token(&self, username: &str, role: &str) -> Result<LoginResponse, String> {
        let role = if role.trim().is_empty() {
            "admin"
        } else {
            role.trim()
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let exp = now.saturating_add(self.token_ttl_secs) as usize;
        let claims = LocalClaims {
            sub: username.to_string(),
            role: role.to_string(),
            exp,
            iss: ISSUER.into(),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(&self.jwt_secret),
        )
        .map_err(|e| e.to_string())?;
        Ok(LoginResponse {
            token,
            username: username.to_string(),
            role: role.to_string(),
            expires_at: exp as i64,
        })
    }

    pub fn claims_from_token(&self, token: &str) -> Option<(String, String)> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[ISSUER]);
        validation.validate_aud = false;
        let data = decode::<LocalClaims>(
            token,
            &DecodingKey::from_secret(&self.jwt_secret),
            &validation,
        )
        .ok()?;
        Some((data.claims.sub, data.claims.role))
    }

    pub fn validate_token(&self, token: &str) -> bool {
        self.claims_from_token(token).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_token_roundtrip() {
        let auth = LocalAuth {
            jwt_secret: b"test-secret-key-at-least-32-bytes!!".to_vec(),
            token_ttl_secs: 3600,
        };
        let resp = auth.issue_token("admin", "admin").expect("issue");
        assert!(auth.validate_token(&resp.token));
        assert_eq!(resp.role, "admin");
        assert_eq!(
            auth.claims_from_token(&resp.token),
            Some(("admin".into(), "admin".into()))
        );
    }
}
