use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

fn registry() -> &'static Mutex<HashMap<String, Instant>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn remaining_seconds(target_id: &str, cooldown_seconds: i32) -> i64 {
    if cooldown_seconds <= 0 {
        return 0;
    }
    let Ok(map) = registry().lock() else {
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

pub fn try_acquire(target_id: &str, cooldown_seconds: i32) -> Result<(), i64> {
    if cooldown_seconds <= 0 {
        return Ok(());
    }

    match registry().lock() {
        Ok(mut map) => {
            if let Some(last) = map.get(target_id) {
                let window = Duration::from_secs(cooldown_seconds as u64);
                if last.elapsed() < window {
                    return Err((window - last.elapsed()).as_secs().max(1) as i64);
                }
            }
            map.insert(target_id.to_string(), Instant::now());
            Ok(())
        }
        Err(_) => Err(1),
    }
}

pub fn reset(target_id: &str) {
    if let Ok(mut map) = registry().lock() {
        map.remove(target_id);
    }
}
