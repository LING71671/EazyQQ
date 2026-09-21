//! Per-target reply cooldown.
//!
//! `contact_rules.cooldown_seconds` was stored and editable but never enforced, so an
//! `auto_reply` group could be answered on every single message - including a burst of
//! them. This module is the missing rate limit.
//!
//! State is intentionally in-memory: a cooldown is about pacing the current run, and
//! losing it on restart is harmless. A persisted timestamp would also mean a stale
//! "recently answered" entry could survive for hours across sessions.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

fn registry() -> &'static Mutex<HashMap<String, Instant>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Seconds still to wait for a target, or 0 when it is free to be processed.
pub fn remaining_seconds(target_id: &str, cooldown_seconds: i32) -> i64 {
    if cooldown_seconds <= 0 {
        return 0;
    }
    let Ok(map) = registry().lock() else {
        // A poisoned lock must not block replies forever.
        return 0;
    };
    match map.get(target_id) {
        Some(last) => {
            let elapsed = last.elapsed();
            let window = Duration::from_secs(cooldown_seconds as u64);
            if elapsed >= window {
                0
            } else {
                (window - elapsed).as_secs().max(1) as i64
            }
        }
        None => 0,
    }
}

/// Claim the right to process a target now.
///
/// `Ok(())` means go ahead (and the cooldown clock starts). `Err(seconds)` means the
/// target is still cooling down.
pub fn try_acquire(target_id: &str, cooldown_seconds: i32) -> Result<(), i64> {
    if cooldown_seconds <= 0 {
        return Ok(());
    }

    let wait = remaining_seconds(target_id, cooldown_seconds);
    if wait > 0 {
        return Err(wait);
    }

    match registry().lock() {
        Ok(mut map) => {
            map.insert(target_id.to_string(), Instant::now());
            Ok(())
        }
        // Never let a lock failure silently suppress replies.
        Err(_) => Ok(()),
    }
}

/// Forget a target's cooldown (used by tests and by manual "reply now" actions).
pub fn reset(target_id: &str) {
    if let Ok(mut map) = registry().lock() {
        map.remove(target_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_call_is_allowed_and_second_is_blocked() {
        let target = "cooldown_test_basic";
        reset(target);

        assert!(try_acquire(target, 60).is_ok(), "first claim must succeed");
        let blocked = try_acquire(target, 60);
        assert!(blocked.is_err(), "second claim must be rate limited");
        let remaining = blocked.unwrap_err();
        assert!((1..=60).contains(&remaining), "got {remaining}");

        reset(target);
    }

    #[test]
    fn zero_cooldown_never_blocks() {
        let target = "cooldown_test_zero";
        reset(target);
        for _ in 0..5 {
            assert!(try_acquire(target, 0).is_ok());
        }
        assert_eq!(remaining_seconds(target, 0), 0);
        reset(target);
    }

    #[test]
    fn negative_cooldown_is_treated_as_disabled() {
        let target = "cooldown_test_negative";
        reset(target);
        assert!(try_acquire(target, -5).is_ok());
        assert!(try_acquire(target, -5).is_ok());
        reset(target);
    }

    #[test]
    fn reset_clears_the_window() {
        let target = "cooldown_test_reset";
        reset(target);
        assert!(try_acquire(target, 300).is_ok());
        assert!(try_acquire(target, 300).is_err());
        reset(target);
        assert!(try_acquire(target, 300).is_ok(), "reset must free the target");
        reset(target);
    }

    #[test]
    fn targets_are_independent() {
        let a = "cooldown_test_a";
        let b = "cooldown_test_b";
        reset(a);
        reset(b);

        assert!(try_acquire(a, 120).is_ok());
        assert!(try_acquire(b, 120).is_ok(), "a different target must not be affected");
        assert!(try_acquire(a, 120).is_err());

        reset(a);
        reset(b);
    }

    #[test]
    fn remaining_is_zero_for_an_unseen_target() {
        assert_eq!(remaining_seconds("cooldown_test_unseen", 30), 0);
    }
}
