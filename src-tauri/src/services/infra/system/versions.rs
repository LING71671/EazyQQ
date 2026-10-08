//! Runtime version discovery and SemVer ordering; unknown versions stay unknown.
use std::path::Path;

pub fn newer(latest: &str, installed: &str) -> bool {
    let parse = |s: &str| semver::Version::parse(s.trim().trim_start_matches('v')).ok();
    matches!((parse(latest), parse(installed)), (Some(latest), Some(installed)) if latest > installed)
}

pub fn napcat(dir: &Path) -> String {
    if let Ok(bytes) = std::fs::read(dir.join("package.json")) {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(version) = value["version"].as_str().filter(|s| *s != "0.0.1") {
                if semver::Version::parse(version).is_ok() {
                    return version.into();
                }
            }
        }
    }
    if let Ok(bundle) = std::fs::read_to_string(dir.join("napcat.mjs")) {
        for line in bundle.lines() {
            if let Some(prefix) = line
                .split(" || \"1.0.0-dev\"")
                .next()
                .filter(|_| line.contains(" || \"1.0.0-dev\""))
            {
                if let Some(version) = prefix.split('"').rev().nth(1) {
                    if semver::Version::parse(version).is_ok() {
                        return version.into();
                    }
                }
            }
        }
    }
    if let Ok(version) = std::fs::read_to_string(dir.join("version.txt")) {
        if semver::Version::parse(version.trim().trim_start_matches('v')).is_ok() {
            return version.trim().trim_start_matches('v').into();
        }
    }
    "unknown".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_do_not_offer_downgrades_or_repeat_identical_prereleases() {
        assert!(!newer("0.3.3-beta", "0.3.3-beta"));
        assert!(!newer("0.3.2", "0.3.3-beta"));
        assert!(newer("0.3.3", "0.3.3-beta"));
        assert!(newer("4.18.100", "4.18.33"));
        assert!(!newer("garbage", "4.18.33"));
    }
}
