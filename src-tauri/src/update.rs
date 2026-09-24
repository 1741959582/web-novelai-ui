use crate::nai::{format_reqwest, http_client};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const REPO: &str = "caiweida/web-novelai-ui";
const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub html_url: String,
    pub setup_url: Option<String>,
    pub portable_url: Option<String>,
    pub available: bool,
    pub portable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateProgress {
    pub received: u64,
    pub total: u64,
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn parse_version(value: &str) -> [u32; 3] {
    let mut out = [0u32; 3];
    let trimmed = value.trim().trim_start_matches('v').trim_start_matches('V');
    for (i, part) in trimmed.split('.').take(3).enumerate() {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        out[i] = digits.parse().unwrap_or(0);
    }
    out
}

pub fn version_newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

pub fn is_portable_install() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .map(|dir| !dir.join("uninstall.exe").is_file())
        .unwrap_or(true)
}

fn pick_setup(assets: &[GhAsset]) -> Option<String> {
    assets
        .iter()
        .find(|a| {
            let name = a.name.to_ascii_lowercase();
            name.ends_with(".exe") && (name.contains("setup") || name.contains("-setup-"))
        })
        .map(|a| a.browser_download_url.clone())
}

fn pick_portable(assets: &[GhAsset]) -> Option<String> {
    assets
        .iter()
        .find(|a| {
            let name = a.name.to_ascii_lowercase();
            name.ends_with(".zip") && name.contains("portable")
        })
        .map(|a| a.browser_download_url.clone())
}

fn filename_from_url(url: &str) -> String {
    url.split('?')
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .unwrap_or("update.bin")
        .to_string()
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("无法打开链接".into())
    }
}

#[tauri::command]
pub fn app_version() -> String {
    CURRENT.to_string()
}

fn tag_from_location(location: &str) -> Option<String> {
    let path = location.split(['?', '#']).next().unwrap_or(location);
    if !path.contains("/releases/tag/") {
        return None;
    }
    let tag = path.rsplit('/').next().unwrap_or("").trim();
    if tag.is_empty() { None } else { Some(tag.to_string()) }
}

fn info_from_tag(tag: &str) -> AppUpdateInfo {
    let version = tag.trim().trim_start_matches(['v', 'V']);
    let download = format!("https://github.com/{REPO}/releases/download/v{version}");
    AppUpdateInfo {
        current: CURRENT.into(),
        latest: version.into(),
        notes: String::new(),
        html_url: format!("https://github.com/{REPO}/releases/tag/v{version}"),
        setup_url: Some(format!("{download}/NAI-Studio-Web-UI-Setup-{version}.exe")),
        portable_url: Some(format!("{download}/NAI-Studio-Web-UI-{version}-portable.zip")),
        available: version_newer(version, CURRENT),
        portable: is_portable_install(),
    }
}

async fn fetch_latest_from_page(use_proxy: bool) -> Result<AppUpdateInfo, String> {
    let client = crate::nai::update_check_client_no_redirect(use_proxy)?;
    let url = format!("https://github.com/{REPO}/releases/latest");
    let res = client
        .get(&url)
        .header("User-Agent", format!("Langbai-NovelAI-Studio/{CURRENT}"))
        .send()
        .await
        .map_err(format_reqwest)?;
    let location = res
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let tag = tag_from_location(location).ok_or_else(|| "发行页没有给出版本号".to_string())?;
    Ok(info_from_tag(&tag))
}

async fn fetch_latest(client: &reqwest::Client) -> Result<AppUpdateInfo, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let res = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", format!("Langbai-NovelAI-Studio/{CURRENT}"))
        .send()
        .await
        .map_err(format_reqwest)?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!(
            "读取 GitHub 发行版失败 HTTP {status}。{}",
            body.chars().take(120).collect::<String>()
        ));
    }
    let release: GhRelease = res.json().await.map_err(format_reqwest)?;
    let latest = release.tag_name.trim().to_string();
    Ok(AppUpdateInfo {
        current: CURRENT.into(),
        latest: latest.trim_start_matches('v').trim_start_matches('V').into(),
        notes: release.body.unwrap_or_default(),
        html_url: release.html_url,
        setup_url: pick_setup(&release.assets),
        portable_url: pick_portable(&release.assets),
        available: version_newer(&latest, CURRENT),
        portable: is_portable_install(),
    })
}

#[tauri::command]
pub async fn check_app_update() -> Result<AppUpdateInfo, String> {
    let mut modes = vec![false];
    if crate::nai::proxy_is_configured() {
        modes.push(true);
    }
    let mut last = String::from("无法连接 GitHub");
    for use_proxy in modes {
        let client = match crate::nai::update_check_client(use_proxy) {
            Ok(client) => client,
            Err(err) => {
                last = err;
                continue;
            }
        };
        match fetch_latest(&client).await {
            Ok(info) => return Ok(info),
            Err(err) => {
                let limited = err.contains("rate limit") || err.contains("HTTP 403") || err.contains("HTTP 429");
                last = err;
                if limited {
                    if let Ok(info) = fetch_latest_from_page(use_proxy).await {
                        return Ok(info);
                    }
                }
            }
        }
    }
    Err(last)
}

#[tauri::command]
pub fn open_latest_release(url: String) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return open_url(&format!("https://github.com/{REPO}/releases/latest"));
    }
    open_url(url)
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle, info: AppUpdateInfo) -> Result<(), String> {
    let portable = info.portable;
    let (url, kind) = if portable {
        info.portable_url
            .as_deref()
            .map(|u| (u, "portable"))
            .or_else(|| info.setup_url.as_deref().map(|u| (u, "setup")))
    } else {
        info.setup_url
            .as_deref()
            .map(|u| (u, "setup"))
            .or_else(|| info.portable_url.as_deref().map(|u| (u, "portable")))
    }
    .ok_or_else(|| "发行版里没有找到安装包，请打开 GitHub 手动下载。".to_string())?;
    let dest = download_asset(&app, url).await?;
    if kind == "setup" && dest.extension().and_then(|s| s.to_str()).unwrap_or("").eq_ignore_ascii_case("exe") {
        emit(&app, 100, 100, "正在打开安装程序…");
        std::process::Command::new(&dest)
            .spawn()
            .map_err(|e| format!("无法启动安装程序：{e}"))?;
        app.exit(0);
        return Ok(());
    }
    apply_portable(&app, &dest)?;
    Ok(())
}

async fn download_asset(app: &AppHandle, url: &str) -> Result<PathBuf, String> {
    let client = http_client()?;
    emit(app, 0, 0, "正在从 GitHub 下载新版本…");
    let res = client
        .get(url)
        .header("User-Agent", format!("Langbai-NovelAI-Studio/{CURRENT}"))
        .send()
        .await
        .map_err(format_reqwest)?;
    if !res.status().is_success() {
        return Err(format!("下载安装包失败 HTTP {}", res.status()));
    }
    let total = res.content_length().unwrap_or(0);
    let name = filename_from_url(url);
    let dest = std::env::temp_dir().join(name);
    let tmp = dest.with_extension("part");
    let mut file = fs::File::create(&tmp).map_err(|e| format!("无法写入临时文件：{e}"))?;
    let mut stream = res.bytes_stream();
    let mut received = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(format_reqwest)?;
        received += chunk.len() as u64;
        file.write_all(&chunk).map_err(|e| format!("写入安装包失败：{e}"))?;
        emit(app, received, total, "正在下载安装包…");
    }
    drop(file);
    fs::rename(&tmp, &dest).map_err(|e| format!("保存安装包失败：{e}"))?;
    Ok(dest)
}

fn apply_portable(app: &AppHandle, zip_path: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dest_dir = exe.parent().ok_or("无法定位程序目录")?.to_path_buf();
    let stage = std::env::temp_dir().join(format!("nai-studio-update-{}", std::process::id()));
    if stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    extract_zip(zip_path, &stage)?;
    let payload = find_payload(&stage).ok_or("压缩包里没有找到程序文件")?;
    let bat = std::env::temp_dir().join("nai-studio-apply-update.bat");
    let bat_body = format!(
        "@echo off\r\nsetlocal\r\nset EXE=web-novelai-ui.exe\r\n:wait\r\ntasklist /FI \"IMAGENAME eq %EXE%\" | find /I \"%EXE%\" >nul\r\nif not errorlevel 1 (\r\n  ping 127.0.0.1 -n 2 >nul\r\n  goto wait\r\n)\r\nxcopy /E /Y /Q \"{src}\\*\" \"{dst}\\\" >nul\r\nstart \"\" \"{dst}\\%EXE%\"\r\ndel \"%~f0\"\r\n",
        src = payload.display(),
        dst = dest_dir.display(),
    );
    fs::write(&bat, bat_body).map_err(|e| e.to_string())?;
    emit(app, 100, 100, "正在替换程序文件…");
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &bat.to_string_lossy()])
        .spawn()
        .map_err(|e| format!("无法启动更新脚本：{e}"))?;
    app.exit(0);
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("无法解压更新包：{e}"))?;
    for i in 0..archive.len() {
        let mut item = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = item.enclosed_name() else { continue };
        let out = dest.join(name);
        if item.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut target = fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut item, &mut target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_payload(root: &Path) -> Option<PathBuf> {
    if root.join("web-novelai-ui.exe").is_file() {
        return Some(root.to_path_buf());
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("web-novelai-ui.exe").is_file() {
            return Some(path);
        }
    }
    None
}

fn emit(app: &AppHandle, received: u64, total: u64, message: &str) {
    let percent = if total > 0 {
        (received as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let _ = app.emit(
        "app-update",
        AppUpdateProgress {
            received,
            total,
            percent,
            message: message.into(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{info_from_tag, parse_version, pick_portable, pick_setup, tag_from_location, version_newer, GhAsset};

    #[test]
    fn compares_semver_tags() {
        assert!(version_newer("v0.1.2", "0.1.1"));
        assert!(!version_newer("0.1.1", "0.1.1"));
        assert!(!version_newer("0.1.0", "0.1.1"));
        assert_eq!(parse_version("v0.1.2"), [0, 1, 2]);
    }

    #[test]
    fn picks_release_assets() {
        let assets = vec![
            GhAsset {
                name: "NAI-Studio-Web-UI-Setup-0.1.2.exe".into(),
                browser_download_url: "https://example/setup.exe".into(),
            },
            GhAsset {
                name: "NAI-Studio-Web-UI-0.1.2-portable.zip".into(),
                browser_download_url: "https://example/portable.zip".into(),
            },
        ];
        assert_eq!(pick_setup(&assets).as_deref(), Some("https://example/setup.exe"));
        assert_eq!(pick_portable(&assets).as_deref(), Some("https://example/portable.zip"));
    }

    #[test]
    fn reads_version_from_release_redirect() {
        let tag = tag_from_location("https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.13").unwrap();
        assert_eq!(tag, "v0.1.13");
        let info = info_from_tag(&tag);
        assert_eq!(info.latest, "0.1.13");
        assert_eq!(
            info.setup_url.as_deref(),
            Some("https://github.com/caiweida/web-novelai-ui/releases/download/v0.1.13/NAI-Studio-Web-UI-Setup-0.1.13.exe")
        );
        assert!(tag_from_location("https://github.com/caiweida/web-novelai-ui").is_none());
    }
}
