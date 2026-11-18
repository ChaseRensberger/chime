use serde::Deserialize;
use std::{sync::Mutex, time::Duration};
use tauri::utils::{config::BundleType, platform::bundle_type};
use tauri_plugin_opener::OpenerExt;

const REPO: &str = "https://github.com/ChaseRensberger/chime";

#[derive(Default)]
pub struct Updates(Mutex<Option<String>>);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
}

fn matching_release(
    current: &semver::Version,
    release: Release,
    arch: &str,
    format: &str,
) -> Option<(String, String)> {
    let tag = release.tag_name;
    let version = semver::Version::parse(tag.strip_prefix('v')?).ok()?;
    if release.draft
        || release.prerelease
        || !version.pre.is_empty()
        || !version.cmp_precedence(current).is_gt()
    {
        return None;
    }
    let filename = format!("Chime_{version}_{arch}.{format}");
    release
        .assets
        .iter()
        .any(|a| a.state == "uploaded" && a.name == filename)
        .then(|| {
            (
                tag.clone(),
                format!("{REPO}/releases/download/{tag}/{filename}"),
            )
        })
}

#[tauri::command]
pub async fn update_check(
    app: tauri::AppHandle,
    updates: tauri::State<'_, Updates>,
) -> Result<Option<String>, String> {
    let format = match bundle_type() {
        Some(BundleType::App) => "dmg",
        Some(BundleType::AppImage) => "AppImage",
        Some(BundleType::Deb) => "deb",
        _ => return Ok(None),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Ok(None),
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Chime release check")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get("https://api.github.com/repos/ChaseRensberger/chime/releases/latest")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let release = serde_json::from_slice(&response.bytes().await.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let available = matching_release(&app.package_info().version, release, arch, format);
    let (tag, url) = available.unzip();
    *updates.0.lock().unwrap() = url;
    Ok(tag)
}

#[tauri::command]
pub fn update_download(
    app: tauri::AppHandle,
    updates: tauri::State<'_, Updates>,
) -> Result<(), String> {
    let url = updates
        .0
        .lock()
        .unwrap()
        .clone()
        .ok_or("No update available")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offers_only_newer_stable_matching_packages() {
        let offer = |tag: &str, arch, format| {
            matching_release(
                &"0.0.9".parse().unwrap(),
                Release {
                    tag_name: tag.into(),
                    draft: false,
                    prerelease: false,
                    assets: vec![Asset {
                        name: format!("Chime_{}_arm64.AppImage", &tag[1..]),
                        state: "uploaded".into(),
                    }],
                },
                arch,
                format,
            )
        };
        assert!(offer("v0.0.10", "arm64", "AppImage").is_some());
        for tag in ["v0.0.8", "v0.0.9", "v0.1.0-beta.1", "v0.0.9+local"] {
            assert!(offer(tag, "arm64", "AppImage").is_none());
        }
        assert!(offer("v0.0.10", "amd64", "AppImage").is_none());
        assert!(offer("v0.0.10", "arm64", "deb").is_none());
    }
}
