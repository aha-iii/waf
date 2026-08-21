//! Simple in-process rate limit for AI recompute APIs (single instance).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum AiRateLimitKind {
    PolicySim,
    IncidentReport,
}

impl AiRateLimitKind {
    pub fn max_per_window(self) -> u32 {
        match self {
            Self::PolicySim => 10,
            Self::IncidentReport => 20,
        }
    }

    fn window(self) -> Duration {
        Duration::from_secs(60)
    }
}

struct Bucket {
    count: u32,
    reset_at: Instant,
}

static BUCKETS: Mutex<Option<HashMap<AiRateLimitKind, Bucket>>> = Mutex::new(None);

fn map_mut() -> std::sync::MutexGuard<'static, Option<HashMap<AiRateLimitKind, Bucket>>> {
    let mut guard = BUCKETS.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    guard
}

/// Returns Err(retry_after_sec) when limited.
pub fn check_expensive_ai(kind: AiRateLimitKind) -> Result<(), u64> {
    let max = kind.max_per_window();
    let window = kind.window();
    let now = Instant::now();
    let mut guard = map_mut();
    let map = guard.as_mut().expect("bucket map");
    let entry = map.entry(kind).or_insert(Bucket {
        count: 0,
        reset_at: now + window,
    });
    if now >= entry.reset_at {
        entry.count = 0;
        entry.reset_at = now + window;
    }
    if entry.count >= max {
        let retry = entry.reset_at.saturating_duration_since(now).as_secs().max(1);
        return Err(retry);
    }
    entry.count += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_expensive_ai, AiRateLimitKind};

    #[test]
    fn rate_limit_allows_under_cap() {
        for _ in 0..AiRateLimitKind::PolicySim.max_per_window() {
            assert!(check_expensive_ai(AiRateLimitKind::PolicySim).is_ok());
        }
        assert!(check_expensive_ai(AiRateLimitKind::PolicySim).is_err());
    }
}
