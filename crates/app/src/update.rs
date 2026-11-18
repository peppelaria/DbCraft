//! Update notice, as in ArtCraft's launcher (https://github.com/storytold/craft-launcher,
//! MIT OR Apache-2.0) and OpenSlate: GitHub is asked for DbCraft's latest release and, if it is
//! newer than this one, a bar under the toolbar says so ("Aggiorna", "Novità", "Salta questa
//! versione").
//!
//! The app updates itself (`selfupdate`): it downloads, checks and installs the new version in
//! place of this one, then restarts.
//!
//! Updating does not touch the data: saved connections and settings live in the user's config
//! folder (macOS: ~/Library/Application Support/dbcraft), not inside the app; passwords live in
//! the system keychain. Replacing the app leaves them where they are.

use std::time::Duration;

use serde_json::Value;

/// The repository with the releases.
pub const REPO: &str = "rmpaolillo/DbCraft";
/// How often to check again while the app is open.
pub const EVERY: Duration = Duration::from_secs(12 * 3600);

/// A release newer than this one.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Version without the "v" (0.2.5).
    pub version: String,
    /// The release page on GitHub (what's new).
    pub page: String,
    /// The file to download for this system (or the page, if there is none).
    pub download: String,
    /// Name of the file for this system, if any (for the in-app update).
    pub asset: Option<String>,
    /// The SHA-256 checksums of the files (`SHA256SUMS.txt`), if the release has them.
    pub sums: Option<String>,
}

/// This app's version (to try the notice: DBCRAFT_PRETEND_VERSION=0.0.1).
pub fn current() -> &'static str {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| std::env::var("DBCRAFT_PRETEND_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()))
}

/// The app comes from a release (DMG, .deb, zip…), not built from source: only then does it
/// make sense to compare it with the releases. From source the Cargo.toml version (e.g. 0.1.0)
/// is always "old" compared with the tags: update with `git pull`. Try anyway with
/// DBCRAFT_UPDATE_TEST=1.
pub fn from_release() -> bool {
    option_env!("DBCRAFT_RELEASE").is_some() || std::env::var_os("DBCRAFT_UPDATE_TEST").is_some()
}

/// `0.2.5`, `v0.2.5` → (0, 2, 5). Pre-releases (`0.3.0-beta`) do not count.
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    if v.contains('-') {
        return None;
    }
    let v = v.split('+').next()?;
    let mut n = v.split('.').map(|x| x.parse::<u64>());
    let a = n.next()?.ok()?;
    let b = n.next().unwrap_or(Ok(0)).ok()?;
    let c = n.next().unwrap_or(Ok(0)).ok()?;
    Some((a, b, c))
}

/// Is `latest` newer than `installed`?
pub fn is_newer(latest: &str, installed: &str) -> bool {
    matches!((parse(latest), parse(installed)), (Some(l), Some(i)) if l > i)
}

pub(crate) fn agent(connect: u64, read: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(connect))
        .timeout_read(Duration::from_secs(read))
        .user_agent(&format!("DbCraft/{}", current()))
        .build()
}

/// Asks GitHub for the latest release. `Ok(None)`: this is already the latest.
pub fn check() -> Result<Option<Release>, String> {
    // for tests: GitHub's answer from a file
    if let Some(f) = std::env::var_os("DBCRAFT_UPDATE_JSON") {
        let bytes = std::fs::read(f).map_err(|e| e.to_string())?;
        let json: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        return Ok(from_json(&json, current(), std::env::consts::OS, deb_installed()));
    }
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = agent(5, 20).get(&url).set("Accept", "application/vnd.github+json").set("X-GitHub-Api-Version", "2022-11-28").call();
    let json: Value = match resp {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        // no release published
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(ureq::Error::Status(403 | 429, _)) => return Err("GitHub: troppe richieste, riprovo più tardi".into()),
        Err(ureq::Error::Status(code, _)) => return Err(format!("GitHub: HTTP {code}")),
        Err(e) => return Err(format!("GitHub: {e}")),
    };
    Ok(from_json(&json, current(), std::env::consts::OS, deb_installed()))
}

/// The release from GitHub's JSON, if it is newer than `installed`.
pub fn from_json(j: &Value, installed: &str, os: &str, deb: bool) -> Option<Release> {
    if j["draft"].as_bool() == Some(true) || j["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = j["tag_name"].as_str()?;
    if !is_newer(tag, installed) {
        return None;
    }
    let page = j["html_url"].as_str().map(String::from).unwrap_or_else(|| format!("https://github.com/{REPO}/releases/tag/{tag}"));
    let assets: Vec<(&str, &str)> = j["assets"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| Some((x["name"].as_str()?, x["browser_download_url"].as_str()?))).collect())
        .unwrap_or_default();
    let want = |name: &str| -> bool {
        match os {
            "macos" => name.ends_with(".dmg"),
            "linux" if deb => name.ends_with("_amd64.deb"),
            "linux" => name.ends_with("-linux-x86_64.tar.gz"),
            "windows" => name.ends_with(".zip") && name.contains("windows"),
            _ => false,
        }
    };
    // Mac: first the file for this processor, then the universal one
    let mac_arch = if cfg!(target_arch = "aarch64") { "-macos-arm64.dmg" } else { "-macos-intel.dmg" };
    let found = if os == "macos" {
        assets
            .iter()
            .find(|(n, _)| n.ends_with(mac_arch))
            .or_else(|| assets.iter().find(|(n, _)| n.ends_with("-macos-universal.dmg")))
            .or_else(|| if cfg!(target_arch = "aarch64") { assets.iter().find(|(n, _)| want(n)) } else { None })
    } else {
        assets.iter().find(|(n, _)| want(n))
    };
    let download = found.map(|(_, u)| u.to_string()).unwrap_or_else(|| page.clone());
    let asset = found.map(|(n, _)| n.to_string());
    let sums = assets.iter().find(|(n, _)| *n == "SHA256SUMS.txt").map(|(_, u)| u.to_string());
    Some(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), page, download, asset, sums })
}

/// Linux: installed with the .deb package (then the .deb is offered).
pub fn deb_installed() -> bool {
    cfg!(target_os = "linux") && std::env::current_exe().is_ok_and(|p| p.starts_with("/usr"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn versions() {
        assert_eq!(parse("v0.2.5"), Some((0, 2, 5)));
        assert_eq!(parse("0.3"), Some((0, 3, 0)));
        assert_eq!(parse("0.3.0-beta.1"), None);
        assert!(is_newer("v0.2.10", "0.2.9"));
        assert!(!is_newer("v0.2.3", "0.2.3"));
        assert!(!is_newer("v0.2.2", "0.2.3"));
        assert!(is_newer("v1.0.0", "0.9.9"));
    }

    #[test]
    fn github_release() {
        let j = json!({
            "tag_name": "v0.2.7", "html_url": "https://github.com/rmpaolillo/DbCraft/releases/tag/v0.2.7",
            "draft": false, "prerelease": false,
            "assets": [
                {"name": "DbCraft-0.2.7-macos-arm64.dmg", "browser_download_url": "https://x/DbCraft-0.2.7-macos-arm64.dmg"},
                {"name": "dbcraft_0.2.7_amd64.deb", "browser_download_url": "https://x/dbcraft_0.2.7_amd64.deb"},
                {"name": "dbcraft-0.2.7-linux-x86_64.tar.gz", "browser_download_url": "https://x/dbcraft-0.2.7-linux-x86_64.tar.gz"},
                {"name": "dbcraft-0.2.7-windows-x86_64.zip", "browser_download_url": "https://x/dbcraft-0.2.7-windows-x86_64.zip"},
                {"name": "SHA256SUMS.txt", "browser_download_url": "https://x/SHA256SUMS.txt"}
            ]
        });
        let r = from_json(&j, "0.2.5", "macos", false).unwrap();
        assert_eq!(r.version, "0.2.7");
        assert_eq!(r.sums.as_deref(), Some("https://x/SHA256SUMS.txt"));
        if cfg!(target_arch = "aarch64") {
            assert!(r.download.ends_with("macos-arm64.dmg"));
        } else {
            // an Intel Mac cannot run the Apple-silicon build: the release page
            assert_eq!(r.download, r.page);
        }
        assert!(from_json(&j, "0.2.5", "linux", true).unwrap().download.ends_with(".deb"));
        assert!(from_json(&j, "0.2.5", "linux", false).unwrap().download.ends_with(".tar.gz"));
        assert!(from_json(&j, "0.2.5", "windows", false).unwrap().download.ends_with(".zip"));
        // already up to date, or a pre-release
        assert!(from_json(&j, "0.2.7", "macos", false).is_none());
        let mut beta = j.clone();
        beta["prerelease"] = json!(true);
        assert!(from_json(&beta, "0.2.5", "macos", false).is_none());
        // no file for the system: the release page
        assert_eq!(from_json(&j, "0.2.5", "freebsd", false).unwrap().download, r.page);
    }
}
