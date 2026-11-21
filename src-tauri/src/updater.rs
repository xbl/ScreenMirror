use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: Option<String>,
    pub release_url: String,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f64,
    pub status: String, // "downloading" | "extracting" | "ready" | "error"
    pub message: Option<String>,
}

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Checks whether `latest` is semantically newer than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let cur_str = current.trim().trim_start_matches('v');
    let lat_str = latest.trim().trim_start_matches('v');

    match (
        semver::Version::parse(cur_str),
        semver::Version::parse(lat_str),
    ) {
        (Ok(cur), Ok(lat)) => lat > cur,
        _ => {
            // Fallback: simple numeric split comparison (e.g. 0.1.2 vs 0.1.1)
            let cur_parts: Vec<u64> = cur_str.split('.').filter_map(|s| s.parse().ok()).collect();
            let lat_parts: Vec<u64> = lat_str.split('.').filter_map(|s| s.parse().ok()).collect();
            lat_parts > cur_parts
        }
    }
}

/// Queries GitHub Releases for the latest version.
pub async fn check_for_updates(current_version: &str) -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("ScreenMirror-App/{}", current_version))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let url = "https://api.github.com/repos/xbl/ScreenMirror/releases/latest";
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("network request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned status: {}", resp.status()));
    }

    let release: GitHubRelease = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse GitHub release JSON: {e}"))?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let has_update = is_newer_version(current_version, &latest_version);

    // Pick the best asset:
    // 1. ScreenMirror-*-aarch64.app.zip for native in-place update
    // 2. ScreenMirror_*_aarch64.dmg for disk image download
    // 3. First asset or none
    let chosen_asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with("aarch64.app.zip"))
        .or_else(|| release.assets.iter().find(|a| a.name.ends_with(".dmg")))
        .or_else(|| release.assets.first());

    let (download_url, asset_name, asset_size) = match chosen_asset {
        Some(asset) => (
            Some(asset.browser_download_url.clone()),
            Some(asset.name.clone()),
            Some(asset.size),
        ),
        None => (None, None, None),
    };

    Ok(UpdateInfo {
        current_version: current_version.to_string(),
        latest_version,
        has_update,
        release_notes: release.body,
        release_url: release.html_url,
        published_at: release.published_at,
        download_url,
        asset_name,
        asset_size,
    })
}

static IS_DOWNLOADING: AtomicBool = AtomicBool::new(false);

/// Downloads the update archive and installs or stages it.
pub async fn download_and_install_update(
    app: AppHandle,
    download_url: String,
) -> Result<(), String> {
    if IS_DOWNLOADING.swap(true, Ordering::SeqCst) {
        return Err("Another download is already in progress".into());
    }

    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            IS_DOWNLOADING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("download server returned: {}", resp.status()));
    }

    let total_bytes = resp.content_length();
    let temp_dir = std::env::temp_dir().join(format!("screenmirror-update-{}", std::process::id()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("failed to create temp dir: {e}"))?;

    let filename = download_url
        .split('/')
        .last()
        .unwrap_or("update.zip")
        .to_string();
    let dest_file = temp_dir.join(&filename);

    let mut file = tokio::fs::File::create(&dest_file)
        .await
        .map_err(|e| format!("failed to create destination file: {e}"))?;

    let mut downloaded_bytes: u64 = 0;
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let mut stream = resp.bytes_stream();
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("error downloading chunk: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("error writing file: {e}"))?;

        downloaded_bytes += chunk.len() as u64;

        // Throttle progress events to ~20 Hz
        if last_emit.elapsed() >= std::time::Duration::from_millis(50) {
            last_emit = std::time::Instant::now();
            let percent = if let Some(total) = total_bytes {
                (downloaded_bytes as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                "update-download-progress",
                DownloadProgress {
                    downloaded_bytes,
                    total_bytes,
                    percent,
                    status: "downloading".into(),
                    message: None,
                },
            );
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("flush failed: {e}"))?;
    drop(file);

    let _ = app.emit(
        "update-download-progress",
        DownloadProgress {
            downloaded_bytes,
            total_bytes,
            percent: 100.0,
            status: "extracting".into(),
            message: Some("正在解压更新包…".into()),
        },
    );

    // If it's a zip archive containing ScreenMirror.app
    if filename.ends_with(".zip") {
        let extract_dir = temp_dir.join("extracted");
        let status = Command::new("ditto")
            .arg("-x")
            .arg("-k")
            .arg(&dest_file)
            .arg(&extract_dir)
            .status()
            .map_err(|e| format!("failed to run ditto unpack: {e}"))?;

        if !status.success() {
            return Err("failed to extract zip archive via ditto".into());
        }

        // Locate ScreenMirror.app inside extract_dir
        let new_app_path = find_app_bundle(&extract_dir)
            .ok_or_else(|| "Could not find ScreenMirror.app in extracted update".to_string())?;

        // Clear quarantine flags
        let _ = Command::new("xattr").arg("-cr").arg(&new_app_path).status();

        // Check if running from an existing /Applications/ScreenMirror.app
        if let Ok(current_exe) = std::env::current_exe() {
            let exe_str = current_exe.to_string_lossy();
            if let Some(idx) = exe_str.find(".app/Contents/MacOS/") {
                let app_bundle_str = &exe_str[..idx + 4];
                let current_app_path = PathBuf::from(app_bundle_str);

                tracing::info!("Target app bundle to replace: {:?}", current_app_path);
                if current_app_path.exists() {
                    let backup_path = temp_dir.join("ScreenMirror.old.app");
                    let _ = std::fs::remove_dir_all(&backup_path);

                    // Atomic swap
                    if let Err(e) = std::fs::rename(&current_app_path, &backup_path) {
                        tracing::warn!("Failed to rename old app, attempting direct copy: {e}");
                    }

                    let copy_status = Command::new("cp")
                        .arg("-R")
                        .arg(&new_app_path)
                        .arg(&current_app_path)
                        .status();

                    if let Ok(s) = copy_status {
                        if s.success() {
                            let _ = Command::new("xattr")
                                .arg("-cr")
                                .arg(&current_app_path)
                                .status();
                            let _ = std::fs::remove_dir_all(&backup_path);
                        } else {
                            // Rollback
                            let _ = std::fs::rename(&backup_path, &current_app_path);
                            return Err("Failed to copy new app into Applications folder".into());
                        }
                    } else {
                        let _ = std::fs::rename(&backup_path, &current_app_path);
                        return Err("Failed to execute copy command".into());
                    }
                }
            } else {
                tracing::info!("App is not running inside a .app bundle (e.g. dev build); update staged at {:?}", new_app_path);
            }
        }

        let _ = app.emit(
            "update-download-progress",
            DownloadProgress {
                downloaded_bytes,
                total_bytes,
                percent: 100.0,
                status: "ready".into(),
                message: Some("更新已就绪，立即重启应用以完成更新。".into()),
            },
        );
    } else if filename.ends_with(".dmg") {
        // Open DMG with macOS `open` command
        let _ = Command::new("open").arg(&dest_file).status();
        let _ = app.emit(
            "update-download-progress",
            DownloadProgress {
                downloaded_bytes,
                total_bytes,
                percent: 100.0,
                status: "ready".into(),
                message: Some("DMG 安装包已下载并打开，请拖入应用程序文件夹。".into()),
            },
        );
    }

    Ok(())
}

fn find_app_bundle(dir: &Path) -> Option<PathBuf> {
    if dir.extension().and_then(|e| e.to_str()) == Some("app") {
        return Some(dir.to_path_buf());
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.extension().and_then(|e| e.to_str()) == Some("app") {
                    return Some(p);
                }
                if let Some(nested) = find_app_bundle(&p) {
                    return Some(nested);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.1.1", "0.1.2"));
        assert!(is_newer_version("v0.1.1", "v0.1.2"));
        assert!(is_newer_version("0.1.1", "v0.2.0"));
        assert!(is_newer_version("0.1.1", "1.0.0"));

        assert!(!is_newer_version("0.1.1", "0.1.1"));
        assert!(!is_newer_version("v0.1.1", "v0.1.1"));
        assert!(!is_newer_version("0.1.2", "0.1.1"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
    }
}
