use crate::png_meta::{inject_text_chunks, inspect_bytes};
use crate::store::{add_history, data_dir, save_image_bytes, HistoryItem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

const REPO: &str = "DaniilSokolyuk/video2dlssnr";
const ASSET: &str = "video2dlssnr_release.zip";
const FILES: [&str; 4] = [
    "nvngx_dlssnr.dll",
    "nvngx_dlss.dll",
    "nvngx.dll_dlssnr.dll",
    "video2dlssnr.exe",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DlssStatus {
    pub installed: bool,
    pub windows: bool,
    pub tag: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DlssEnhanceRequest {
    pub source_path: String,
    pub scale: f64,
    pub style: u8,
    pub intensity: f64,
    pub local_structure: f64,
    pub local_tone: f64,
    pub detail: f64,
    pub color: f64,
    pub skin: f64,
    pub auto_mask: bool,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    #[serde(default)]
    pub seed: u32,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub group_id: String,
}

#[derive(Deserialize)]
struct GhRelease {
    draft: Option<bool>,
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

struct ChosenRelease {
    tag: String,
    url: String,
    size: u64,
    digest: String,
}

fn runtime_dir() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("dlssnr").join("runtime"))
}

fn windows_host() -> bool {
    cfg!(windows)
}

fn status_message(installed: bool) -> String {
    if !windows_host() {
        return "DLSSNR 只在 Windows 上使用 NVIDIA 显卡处理。".into();
    }
    if installed {
        "运行库已就绪。图像在本机处理，增强结果另存，不覆盖原图。".into()
    } else {
        "还没有运行库。点「安装运行库」会从 video2dlssnr 的公开发行包下载，需要 NVIDIA 显卡。".into()
    }
}

fn installed_tag(dir: &Path) -> Option<String> {
    if !FILES.iter().all(|name| dir.join(name).is_file()) {
        return None;
    }
    let manifest = fs::read_to_string(dir.join("runtime.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&manifest).ok()?;
    json.get("tag").and_then(|v| v.as_str()).map(|s| s.to_string())
}

#[tauri::command]
pub fn dlssnr_status() -> DlssStatus {
    let tag = runtime_dir().ok().and_then(|dir| installed_tag(&dir)).unwrap_or_default();
    let installed = windows_host() && !tag.is_empty();
    DlssStatus {
        installed,
        windows: windows_host(),
        tag,
        message: status_message(installed),
    }
}

#[tauri::command]
pub async fn dlssnr_install() -> Result<DlssStatus, String> {
    if !windows_host() {
        return Err(status_message(false));
    }
    let chosen = fetch_release().await?;
    let root = data_dir()?.join("dlssnr");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let zip_path = root.join("release.zip");
    download_verified(&chosen, &zip_path).await?;
    let staging = root.join("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let extracted = match extract_runtime(&zip_path, &staging) {
        Ok(files) => files,
        Err(err) => {
            let _ = fs::remove_dir_all(&staging);
            let _ = fs::remove_file(&zip_path);
            return Err(err);
        }
    };
    let manifest = serde_json::json!({
        "tag": chosen.tag,
        "digest": chosen.digest,
        "source": format!("https://github.com/{REPO}"),
        "files": extracted,
    });
    fs::write(
        staging.join("runtime.json"),
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let runtime = root.join("runtime");
    if runtime.exists() {
        fs::remove_dir_all(&runtime).map_err(|e| e.to_string())?;
    }
    fs::rename(&staging, &runtime).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&zip_path);
    Ok(dlssnr_status())
}

#[tauri::command]
pub fn dlssnr_enhance(request: DlssEnhanceRequest) -> Result<HistoryItem, String> {
    if !windows_host() {
        return Err(status_message(false));
    }
    let runtime = runtime_dir()?;
    let exe = runtime.join("video2dlssnr.exe");
    if installed_tag(&runtime).is_none() || !exe.is_file() {
        return Err("还没有安装 DLSSNR 运行库。".into());
    }
    let source = PathBuf::from(request.source_path.trim());
    if !source.is_file() {
        return Err("找不到要增强的图片。".into());
    }
    let scale = request.scale;
    let style = request.style;
    let intensity = request.intensity;
    let structure = request.local_structure;
    let tone = request.local_tone;
    let detail = request.detail;
    let color = request.color;
    let skin = request.skin;
    if !(1.0..=8.0).contains(&scale)
        || style > 2
        || !(0.0..=2.0).contains(&intensity)
        || !(0.0..=4.0).contains(&structure)
        || !(0.0..=4.0).contains(&tone)
        || !(0.0..=4.0).contains(&detail)
        || !(0.0..=1.0).contains(&color)
        || !(-1.0..=4.0).contains(&skin)
    {
        return Err("增强参数超出可用范围。".into());
    }

    let job_path = data_dir()?.join("dlssnr").join("jobs").join(Uuid::new_v4().to_string());
    fs::create_dir_all(&job_path).map_err(|e| e.to_string())?;
    let _job = JobDir(job_path.clone());
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let ext = if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
        ext
    } else {
        "png".into()
    };
    let input = job_path.join(format!("source.{ext}"));
    let out_dir = job_path.join("out");
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    fs::copy(&source, &input).map_err(|e| format!("读原图失败：{e}"))?;

    let mut args = vec![
        "--nr-run".into(),
        "--in".into(),
        input.to_string_lossy().into(),
        "--out".into(),
        out_dir.to_string_lossy().into(),
        "--nr-scale".into(),
        format!("{scale}"),
        "--nr-style".into(),
        style.to_string(),
        "--nr-intensity".into(),
        format!("{intensity}"),
        "--nr-local-structure".into(),
        format!("{structure}"),
        "--nr-local-tone".into(),
        format!("{tone}"),
        "--nr-detail".into(),
        format!("{detail}"),
        "--nr-color".into(),
        format!("{color}"),
        "--nr-skin".into(),
        format!("{skin}"),
    ];
    if request.auto_mask {
        args.push("--nr-auto-mask".into());
    }

    let mut cmd = Command::new(&exe);
    cmd.current_dir(&runtime)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("启动 DLSSNR 失败：{e}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("DLSSNR 处理超过 3 分钟，已停止。".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    let output = newest_nr_png(&out_dir);
    if !status.success() || output.is_none() {
        return Err(explain_failure(&stderr, status.code().unwrap_or(-1)));
    }
    let output = output.unwrap();
    let mut bytes = fs::read(&output).map_err(|e| e.to_string())?;
    if let Ok(source_bytes) = fs::read(&source) {
        bytes = copy_comment(&source_bytes, &bytes);
    }
    let saved = save_image_bytes(&bytes, "dlssnr", request.seed, "png")?;
    let (width, height) = image::image_dimensions(&saved).unwrap_or((0, 0));
    let item = HistoryItem {
        id: Uuid::new_v4().to_string(),
        path: saved.to_string_lossy().into(),
        created_at: chrono::Local::now().to_rfc3339(),
        model: if request.model.trim().is_empty() {
            "dlssnr".into()
        } else {
            request.model
        },
        prompt: request.prompt,
        negative_prompt: request.negative_prompt,
        seed: request.seed,
        width,
        height,
        steps: 0,
        sampler: "dlssnr".into(),
        kind: "dlssnr".into(),
        session_id: String::new(),
        group_id: request.group_id,
    };
    add_history(item.clone())?;
    Ok(item)
}

fn copy_comment(source: &[u8], output: &[u8]) -> Vec<u8> {
    if !output.starts_with(b"\x89PNG") {
        return output.to_vec();
    }
    let Ok(report) = inspect_bytes(source) else {
        return output.to_vec();
    };
    let texts: Vec<(&str, String)> = report
        .entries
        .iter()
        .filter(|(key, value)| {
            !value.is_empty() && (key.eq_ignore_ascii_case("comment") || key.eq_ignore_ascii_case("description"))
        })
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    if texts.is_empty() {
        output.to_vec()
    } else {
        inject_text_chunks(output, &texts)
    }
}

fn newest_nr_png(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if !name.ends_with("_nr.png") {
            continue;
        }
        let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if best.as_ref().map(|(time, _)| modified >= *time).unwrap_or(true) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

fn explain_failure(stderr: &str, code: i32) -> String {
    if stderr.contains("too old for DLSS Neural Rendering") {
        let current = stderr
            .split("NVIDIA driver ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or("当前版本");
        return format!(
            "增强没有生效。这台电脑的 NVIDIA 驱动是 {current}，DLSS 神经渲染需要 616.56 或更新的驱动。请到 NVIDIA 官网安装新驱动后再试。这次没有生成新图，左右两边都是原图。"
        );
    }
    let tail = stderr_tail(stderr);
    if tail.is_empty() {
        format!("DLSSNR 没有写出结果（退出码 {code}）。确认这台电脑有可用的 NVIDIA 显卡。")
    } else {
        format!("DLSSNR 没有写出结果（退出码 {code}）。{tail}")
    }
}

fn stderr_tail(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 500 {
        trimmed.to_string()
    } else {
        chars[chars.len() - 500..].iter().collect()
    }
}

struct JobDir(PathBuf);
impl Drop for JobDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_window(_cmd: &mut Command) {}

async fn fetch_release() -> Result<ChosenRelease, String> {
    let client = reqwest::Client::builder()
        .user_agent("langbai-novelai-studio")
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=20");
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("读不到 DLSSNR 发行列表：{e}"))?;
    if !response.status().is_success() {
        return Err(format!("读不到 DLSSNR 发行列表（HTTP {}）。", response.status()));
    }
    let releases: Vec<GhRelease> = response.json().await.map_err(|e| format!("发行列表格式不对：{e}"))?;
    choose_release(&releases).ok_or_else(|| "公开发行包里没有带校验值的 video2dlssnr_release.zip。".into())
}

fn choose_release(releases: &[GhRelease]) -> Option<ChosenRelease> {
    for release in releases {
        if release.draft == Some(true) || !tag_ok(&release.tag_name) {
            continue;
        }
        let Some(asset) = release.assets.iter().find(|asset| asset.name == ASSET) else {
            continue;
        };
        let digest = asset.digest.as_deref().unwrap_or("");
        let Some(hex) = digest.strip_prefix("sha256:") else {
            continue;
        };
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) || asset.size == 0 {
            continue;
        }
        let expected = format!(
            "https://github.com/{REPO}/releases/download/{}/{}",
            release.tag_name, ASSET
        );
        if asset.browser_download_url != expected {
            continue;
        }
        return Some(ChosenRelease {
            tag: release.tag_name.clone(),
            url: asset.browser_download_url.clone(),
            size: asset.size,
            digest: hex.to_ascii_lowercase(),
        });
    }
    None
}

fn tag_ok(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

async fn download_verified(release: &ChosenRelease, dest: &Path) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("langbai-novelai-studio")
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(&release.url)
        .send()
        .await
        .map_err(|e| format!("下载运行库失败：{e}"))?;
    if !response.status().is_success() {
        return Err(format!("下载运行库失败（HTTP {}）。", response.status()));
    }
    let mut file = File::create(dest).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut got = 0u64;
    let mut stream = response;
    while let Some(chunk) = stream.chunk().await.map_err(|e| format!("下载中断：{e}"))? {
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        got += chunk.len() as u64;
        if got > release.size {
            let _ = fs::remove_file(dest);
            return Err("下载到的运行库比发行说明更大。".into());
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    let digest = format!("{:x}", hasher.finalize());
    if got != release.size || digest != release.digest {
        let _ = fs::remove_file(dest);
        return Err("运行库校验失败，没有安装。".into());
    }
    Ok(())
}

fn extract_runtime(zip_path: &Path, dest: &Path) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("运行库压缩包打不开：{e}"))?;
    let mut found = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let raw_name = entry.name().replace('\\', "/");
        if raw_name.split('/').any(|part| part == "..") {
            return Err("运行库压缩包路径不合法。".into());
        }
        let base = raw_name.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        if !FILES.contains(&base.as_str()) {
            continue;
        }
        if found.iter().any(|name: &String| name == &base) {
            return Err(format!("运行库里有重复文件：{base}"));
        }
        let size = entry.size();
        if !(64..=512 * 1024 * 1024).contains(&size) {
            return Err(format!("运行库文件大小异常：{base}"));
        }
        let target = dest.join(&base);
        let mut out = File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        let written = fs::metadata(&target).map_err(|e| e.to_string())?.len();
        if written != size {
            return Err(format!("运行库文件不完整：{base}"));
        }
        let mut header = [0u8; 2];
        File::open(&target)
            .and_then(|mut f| f.read_exact(&mut header))
            .map_err(|e| e.to_string())?;
        if &header != b"MZ" {
            return Err(format!("运行库文件不是 Windows 程序：{base}"));
        }
        found.push(base);
    }
    for name in FILES {
        if !found.iter().any(|item| item == name) {
            return Err(format!("运行库缺少 {name}。"));
        }
    }
    let mut map = serde_json::Map::new();
    for name in &found {
        map.insert(name.clone(), serde_json::Value::Bool(true));
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_verified_release_asset() {
        let body = r#"[
          {"draft": false, "tag_name": "v9.9.9", "assets": [
            {"name": "other.zip", "browser_download_url": "https://example.invalid/other.zip", "size": 10, "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
          ]},
          {"draft": false, "tag_name": "v1.2.3", "assets": [
            {"name": "video2dlssnr_release.zip", "browser_download_url": "https://github.com/DaniilSokolyuk/video2dlssnr/releases/download/v1.2.3/video2dlssnr_release.zip", "size": 42, "digest": "sha256:ABCDEF0123456789abcdef0123456789ABCDEF0123456789abcdef0123456789"}
          ]}
        ]"#;
        let releases: Vec<GhRelease> = serde_json::from_str(body).unwrap();
        let chosen = choose_release(&releases).unwrap();
        assert_eq!(chosen.tag, "v1.2.3");
        assert_eq!(chosen.size, 42);
        assert_eq!(chosen.digest, "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
    }

    #[test]
    fn explains_old_nvidia_driver() {
        let message = explain_failure(
            "ERROR: NVIDIA driver 596.36 is too old for DLSS Neural Rendering: install driver 616.56 or newer",
            1,
        );
        assert!(message.contains("596.36"));
        assert!(message.contains("616.56"));
        assert!(message.contains("原图"));
    }

    #[test]
    fn rejects_asset_without_digest() {
        let body = r#"[{"draft": false, "tag_name": "v1", "assets": [
          {"name": "video2dlssnr_release.zip", "browser_download_url": "https://github.com/DaniilSokolyuk/video2dlssnr/releases/download/v1/video2dlssnr_release.zip", "size": 42, "digest": null}
        ]}]"#;
        let releases: Vec<GhRelease> = serde_json::from_str(body).unwrap();
        assert!(choose_release(&releases).is_none());
    }
}
