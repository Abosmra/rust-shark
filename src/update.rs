//! Once-per-start check for a newer GitHub release. Never downloads anything.

use std::sync::{Arc, OnceLock};

const LATEST: &str = "https://api.github.com/repos/Abosmra/rust-shark/releases/latest";
const RELEASE_URL_PREFIX: &str = "https://github.com/Abosmra/rust-shark/releases/";

/// A newer release: (tag, release page URL).
pub type Found = Arc<OnceLock<(String, String)>>;

fn is_valid_release_url(url: &str) -> bool {
    url.starts_with(RELEASE_URL_PREFIX)
}

/// Ask GitHub in the background; fills the result only if the release is newer than this build.
pub fn check(ctx: egui::Context) -> Found {
    let found: Found = Arc::default();
    let out = found.clone();
    std::thread::spawn(move || {
        let Ok(resp) = ureq::get(LATEST)
            .set("User-Agent", concat!("rust-shark/", env!("CARGO_PKG_VERSION")))
            .set("Accept", "application/vnd.github+json")
            .timeout(std::time::Duration::from_secs(10))
            .call()
        else {
            return;
        };
        let Ok(v) = resp.into_json::<serde_json::Value>() else { return };
        let (Some(tag), Some(url)) = (v["tag_name"].as_str(), v["html_url"].as_str()) else { return };
        if is_newer(tag, env!("CARGO_PKG_VERSION")) && is_valid_release_url(url) {
            let _ = out.set((tag.to_string(), url.to_string()));
            ctx.request_repaint();
        }
    });
    found
}

/// "v0.3.0" > "0.2.0"? Missing or non-numeric parts count as 0.
fn is_newer(tag: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> { s.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parse(tag) > parse(current)
}

#[cfg(test)]
mod tests {
    #[test]
    fn compares_versions() {
        assert!(super::is_newer("v0.3.0", "0.2.0"));
        assert!(super::is_newer("v0.10.0", "0.9.1"));
        assert!(!super::is_newer("v0.2.0", "0.2.0"));
        assert!(!super::is_newer("v0.1.0", "0.2.0"));
        assert!(super::is_newer("v1.0.1", "1.0.0"));
    }

    #[test]
    fn validates_release_urls() {
        assert!(super::is_valid_release_url("https://github.com/Abosmra/rust-shark/releases/tag/v1.0.1"));
        assert!(!super::is_valid_release_url("http://github.com/Abosmra/rust-shark/releases/tag/v1.0.1"));
        assert!(!super::is_valid_release_url("https://evil.example.com/rust-shark/releases/tag/v1.0.1"));
        assert!(!super::is_valid_release_url("https://github.com/other/repo/releases/tag/v1.0.1"));
    }

    /// Network: `cargo test -- --ignored` checks the real GitHub request works.
    #[test]
    #[ignore]
    fn github_answers() {
        let v: serde_json::Value = ureq::get(super::LATEST).call().unwrap().into_json().unwrap();
        assert!(v["tag_name"].as_str().unwrap().starts_with('v'));
    }
}
