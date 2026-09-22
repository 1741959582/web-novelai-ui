use crate::nai::{
    http_client_no_redirect, http_client_no_redirect_direct, proxy_is_configured,
};
use crate::store::load_settings;
use crate::wd_tagger::{LabelConfidence, LabelGroup, WdTagResult};
use base64::Engine;
use futures_util::StreamExt;
use image::imageops::FilterType;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

const REPO: &str = "cella110n/cl_tagger_v2";
const VERSION: &str = "v2_00";
const INPUT_SIZE: u32 = 384;
const FILES: [&str; 3] = ["model.onnx", "model.onnx.data", "model_vocabulary.json"];
const MAX_FILE_BYTES: u64 = 3 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClTaggerDownloadProgress {
    pub file: String,
    pub received: u64,
    pub total: u64,
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClTaggerStatus {
    pub enabled: bool,
    pub downloaded: bool,
    pub version: String,
    pub dir: String,
    pub missing: Vec<String>,
}

struct OrtSession {
    session: ort::session::Session,
    idx_to_tag: Vec<String>,
    tag_to_category: HashMap<String, String>,
}

fn session_slot() -> &'static Mutex<Option<OrtSession>> {
    static SLOT: OnceLock<Mutex<Option<OrtSession>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn drop_session() {
    if let Ok(mut guard) = session_slot().lock() {
        *guard = None;
    }
}

fn app_root() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        return Ok(manifest
            .parent()
            .map(PathBuf::from)
            .unwrap_or(manifest));
    }
    let exe = std::env::current_exe().map_err(|e| format!("无法定位程序目录：{e}"))?;
    Ok(exe
        .parent()
        .map(PathBuf::from)
        .unwrap_or(exe))
}

pub fn model_dir() -> Result<PathBuf, String> {
    let dir = app_root()?.join("models").join("cl_tagger_v2").join(VERSION);
    fs::create_dir_all(&dir).map_err(|e| {
        format!("无法写入模型目录 {}：{e}。请把软件装到可写位置，或用管理员运行。", dir.display())
    })?;
    Ok(dir)
}

pub fn status() -> Result<ClTaggerStatus, String> {
    let dir = model_dir()?;
    let missing: Vec<String> = FILES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| !looks_complete(path))
        .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    let settings = load_settings();
    Ok(ClTaggerStatus {
        enabled: settings.local_cl_tagger_enabled,
        downloaded: missing.is_empty(),
        version: VERSION.into(),
        dir: dir.to_string_lossy().into_owned(),
        missing,
    })
}

const HF_OFFICIAL: &str = "https://huggingface.co";
const HF_MIRROR: &str = "https://hf-mirror.com";

fn min_file_bytes(file: &str) -> u64 {
    if file.ends_with(".onnx.data") {
        50 * 1024 * 1024
    } else {
        256
    }
}

fn looks_like_lfs_pointer(bytes: &[u8]) -> bool {
    let n = bytes.len().min(80);
    String::from_utf8_lossy(&bytes[..n]).starts_with("version https://git-lfs")
}

fn looks_like_html(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(256)])
        .trim_start()
        .to_ascii_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}

fn file_head(path: &Path) -> Vec<u8> {
    let mut buf = [0u8; 256];
    let Ok(mut file) = fs::File::open(path) else {
        return Vec::new();
    };
    let n = file.read(&mut buf).unwrap_or(0);
    buf[..n].to_vec()
}

fn looks_complete(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if meta.len() < min_file_bytes(name) {
        return false;
    }
    let head = file_head(path);
    !looks_like_html(&head) && !looks_like_lfs_pointer(&head)
}

fn hf_endpoints() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(ep) = std::env::var("HF_ENDPOINT") {
        let ep = ep.trim().trim_end_matches('/').to_string();
        if !ep.is_empty() {
            out.push(ep);
        }
    }
    out.push(HF_OFFICIAL.into());
    out.push(HF_MIRROR.into());
    out.dedup();
    out
}

fn hf_resolve_url(base: &str, file: &str) -> String {
    format!(
        "{}/{REPO}/resolve/main/{VERSION}/{file}",
        base.trim_end_matches('/')
    )
}

fn host_needs_hf_token(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return true;
    };
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    if host.contains("cdn-lfs")
        || host.contains("cas-bridge")
        || host.contains("xethub")
        || host.contains("cloudfront")
        || host.contains("amazonaws")
        || host.contains("azureedge")
    {
        return false;
    }
    host.contains("huggingface") || host.contains("hf-mirror") || host.ends_with(".hf.co")
}

fn should_retry_message(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("timeout")
        || m.contains("timed out")
        || m.contains("connect")
        || m.contains("10060")
        || m.contains("10061")
        || m.contains("10054")
        || m.contains("connection reset")
        || m.contains("connection refused")
        || m.contains("error sending request")
        || m.contains("dns")
        || m.contains("请求超时")
        || m.contains("无法连接")
        || m.contains("network is unreachable")
}

fn format_hf_reqwest(err: reqwest::Error) -> String {
    let mut msg = err.to_string();
    let mut src = std::error::Error::source(&err);
    while let Some(s) = src {
        msg.push_str(" → ");
        msg.push_str(&s.to_string());
        src = s.source();
    }
    if err.is_timeout() || err.is_connect() {
        format!("连不上 Hugging Face。{msg}")
    } else {
        msg
    }
}

fn resolve_hf_token(override_token: Option<&str>) -> String {
    let from_req = override_token.unwrap_or("").trim().trim_start_matches("Bearer ").trim();
    if !from_req.is_empty() && !from_req.eq_ignore_ascii_case("configured") {
        return from_req.to_string();
    }
    let stored = crate::store::get_huggingface_token();
    stored.trim().trim_start_matches("Bearer ").trim().to_string()
}

fn emit_download(app: &AppHandle, file: &str, received: u64, total: u64, message: &str) {
    let percent = if total > 0 {
        (received as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let _ = app.emit(
        "cl-tagger-download",
        ClTaggerDownloadProgress {
            file: file.into(),
            received,
            total,
            percent,
            message: message.into(),
        },
    );
}

fn hf_failure(file: &str, status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap, body: &str) -> String {
    let header = headers
        .get("x-error-message")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim();
    let body = body.trim().chars().take(180).collect::<String>();
    let detail = if !header.is_empty() { header } else { body.as_str() };
    let detail_lower = detail.to_ascii_lowercase();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        if detail_lower.contains("restricted") || detail_lower.contains("gated") || detail_lower.contains("access") || detail.is_empty() {
            return format!(
                "无法下载 {file}。请用创建这个 token 的账号打开 https://huggingface.co/{REPO} ，点 Agree 同意许可。Token 需要有 Read 权限。"
            );
        }
        return format!("无法下载 {file}（HTTP {status}）。{detail}");
    }
    format!("下载 {file} 失败：HTTP {status} {detail}")
}

async fn hf_fetch(
    client: &reqwest::Client,
    token: &str,
    start: &str,
    resume_from: u64,
) -> Result<reqwest::Response, String> {
    let mut url = start.to_string();
    for _ in 0..8 {
        let mut req = client.get(&url).header("Accept", "*/*");
        if host_needs_hf_token(&url) && !token.is_empty() {
            req = req.bearer_auth(token);
        }
        if resume_from > 0 {
            req = req.header("Range", format!("bytes={resume_from}-"));
        }
        let response = req.send().await.map_err(format_hf_reqwest)?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .trim()
                .to_string();
            if location.is_empty() {
                return Err("Hugging Face 没有给出下载地址。".into());
            }
            let base = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
            url = base.join(&location).map_err(|e| e.to_string())?.to_string();
            continue;
        }
        return Ok(response);
    }
    Err("Hugging Face 下载跳转次数过多。".into())
}

async fn download_from(
    app: &AppHandle,
    client: &reqwest::Client,
    token: &str,
    file: &str,
    dest: &Path,
    url: &str,
) -> Result<(), String> {
    let tmp = dest.with_extension("part");
    let existing = fs::metadata(&tmp).map(|meta| meta.len()).unwrap_or(0);
    let response = hf_fetch(client, token, url, existing).await?;
    let status = response.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && existing > 0 {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{file} 续传范围无效，已清除临时文件，请再试一次。"));
    }
    if !status.is_success() {
        let headers = response.headers().clone();
        let body = response.text().await.unwrap_or_default();
        return Err(hf_failure(file, status, &headers, &body));
    }
    let resume = status == reqwest::StatusCode::PARTIAL_CONTENT && existing > 0;
    let remaining = response.content_length().unwrap_or(0);
    let total = if resume { existing.saturating_add(remaining) } else { remaining };
    if total > MAX_FILE_BYTES || remaining > MAX_FILE_BYTES {
        return Err(format!("{file} 超过 3GB 上限，已中止。"));
    }
    let mut file_handle = if resume {
        fs::OpenOptions::new()
            .append(true)
            .open(&tmp)
            .map_err(|e| format!("无法续写 {file}：{e}"))?
    } else {
        fs::File::create(&tmp).map_err(|e| format!("无法写入 {file}：{e}"))?
    };
    let mut stream = response.bytes_stream();
    let mut received = if resume { existing } else { 0 };
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(format_hf_reqwest)?;
        received += chunk.len() as u64;
        if received > MAX_FILE_BYTES {
            return Err(format!("{file} 超过 3GB 上限，已中止。"));
        }
        file_handle
            .write_all(&chunk)
            .map_err(|e| format!("写入 {file} 失败：{e}"))?;
        emit_download(
            app,
            file,
            received,
            total,
            &format!("正在下载 {file}（{received} / {total}）"),
        );
    }
    drop(file_handle);
    if total > 0 && received < total {
        return Err(format!("{file} 下载中断（{received}/{total}），再点下载会从断点续传。"));
    }
    if received < min_file_bytes(file) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{file} 下载内容过小，已放弃。"));
    }
    let head = file_head(&tmp);
    if looks_like_html(&head) || looks_like_lfs_pointer(&head) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{file} 下到的不是模型文件（页面或 Git LFS 指针）。请检查 token 和许可。"));
    }
    fs::rename(&tmp, dest).map_err(|e| format!("保存 {file} 失败：{e}"))?;
    emit_download(app, file, received, received.max(total), &format!("{file} 已完成"));
    Ok(())
}

async fn download_file(app: &AppHandle, token: &str, file: &str, dest: &Path) -> Result<(), String> {
    if looks_complete(dest) {
        emit_download(app, file, 1, 1, &format!("{file} 已存在，跳过"));
        return Ok(());
    }
    let mut clients = vec![("当前网络", http_client_no_redirect()?)];
    if proxy_is_configured() {
        clients.push(("直连", http_client_no_redirect_direct()?));
    }
    let mut errors = Vec::new();
    for (net_label, client) in &clients {
        for base in hf_endpoints() {
            let url = hf_resolve_url(&base, file);
            emit_download(app, file, 0, 0, &format!("正在从 {base} 下载 {file}…"));
            match download_from(app, client, token, file, dest, &url).await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    let retry = should_retry_message(&err)
                        || err.contains("HTTP 5")
                        || err.contains("HTTP 429")
                        || err.contains("HTTP 408");
                    errors.push(format!("{base}（{net_label}）：{err}"));
                    if retry {
                        continue;
                    }
                    if err.contains("Agree") || err.contains("HTTP 401") || err.contains("HTTP 403") {
                        return Err(err);
                    }
                }
            }
        }
    }
    Err(format!(
        "无法下载 {file}。已尝试 huggingface.co 和 hf-mirror.com。请检查网络，或在本页填写代理（如 http://127.0.0.1:7890）。{}",
        if errors.is_empty() {
            String::new()
        } else {
            format!("\n{}", errors.join("\n"))
        }
    ))
}

#[tauri::command]
pub async fn cl_tagger_status() -> Result<ClTaggerStatus, String> {
    status()
}

#[tauri::command]
pub async fn cl_tagger_download(app: AppHandle, token: Option<String>) -> Result<ClTaggerStatus, String> {
    let token = resolve_hf_token(token.as_deref());
    if token.is_empty() {
        return Err(format!(
            "本地模型需要 Hugging Face token。请先在 huggingface.co/{REPO} 同意许可，再把 token 填进设置。"
        ));
    }
    let dir = model_dir()?;
    for file in FILES {
        download_file(&app, &token, file, &dir.join(file)).await?;
    }
    drop_session();
    status()
}

#[tauri::command]
pub fn cl_tagger_open_dir() -> Result<(), String> {
    let dir = model_dir()?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }
    Ok(())
}

pub fn tag_image(image_base64: &str, general_thresh: f64, character_thresh: f64) -> Result<WdTagResult, String> {
    let settings = load_settings();
    if !settings.local_cl_tagger_enabled {
        return Err("未启用本地 CL Tagger。请先在设置里检测显卡并打开开关。".into());
    }
    let info = status()?;
    if !info.downloaded {
        return Err(format!(
            "本地 CL Tagger 模型未下载（缺 {}）。请先在设置里下载 v2_00 权重。",
            info.missing.join("、")
        ));
    }
    let pixels = preprocess_image(image_base64)?;
    let mut guard = session_slot()
        .lock()
        .map_err(|_| "本地打标引擎正忙，请稍后再试。".to_string())?;
    if guard.is_none() {
        *guard = Some(load_session()?);
    }
    let engine = guard.as_mut().ok_or_else(|| "本地打标引擎未就绪。".to_string())?;
    run_session(engine, &pixels, general_thresh, character_thresh)
}

fn load_session() -> Result<OrtSession, String> {
    let dir = model_dir()?;
    let onnx = dir.join("model.onnx");
    let vocab_bytes = fs::read(dir.join("model_vocabulary.json")).map_err(|e| format!("读取词表失败：{e}"))?;
    let (idx_to_tag, tag_to_category) = parse_vocab(&vocab_bytes)?;
    let session = open_onnx(&onnx)?;
    Ok(OrtSession {
        session,
        idx_to_tag,
        tag_to_category,
    })
}

#[cfg(windows)]
fn prefer_app_dlls() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let resources = dir.join("resources");
    let dll_dir = if resources.join("DirectML.dll").is_file() {
        resources
    } else {
        dir.to_path_buf()
    };
    let mut wide: Vec<u16> = std::os::windows::ffi::OsStrExt::encode_wide(dll_dir.as_os_str()).collect();
    wide.push(0);
    unsafe extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }
    unsafe {
        SetDllDirectoryW(wide.as_ptr());
    }
}

fn open_onnx(onnx: &std::path::Path) -> Result<ort::session::Session, String> {
    #[cfg(windows)]
    prefer_app_dlls();
    let builder = ort::session::Session::builder().map_err(|e| format!("ONNX 初始化失败：{e}"))?;
    #[cfg(windows)]
    let builder = {
        use ort::execution_providers::{CPUExecutionProvider, DirectMLExecutionProvider};
        match builder.with_execution_providers([
            DirectMLExecutionProvider::default().build(),
            CPUExecutionProvider::default().build(),
        ]) {
            Ok(next) => next,
            Err(_) => ort::session::Session::builder().map_err(|e| format!("ONNX 初始化失败：{e}"))?,
        }
    };
    builder
        .commit_from_file(onnx)
        .map_err(|e| format!("加载 ONNX 失败：{e}。请确认 model.onnx 与 model.onnx.data 在同一目录。"))
}

fn run_session(
    engine: &mut OrtSession,
    pixels: &[f32],
    general_thresh: f64,
    character_thresh: f64,
) -> Result<WdTagResult, String> {
    let tensor = ort::value::Tensor::from_array((
        [1usize, 3, INPUT_SIZE as usize, INPUT_SIZE as usize],
        pixels.to_vec(),
    ))
    .map_err(|e| format!("构造输入失败：{e}"))?;
    let input_name = engine
        .session
        .inputs
        .first()
        .map(|item| item.name.clone())
        .unwrap_or_else(|| "pixel_values".into());
    let outputs = engine
        .session
        .run(ort::inputs![input_name => tensor])
        .map_err(|e| format!("本地推理失败：{e}"))?;
    let mut logits = None;
    for (name, value) in outputs.iter() {
        let lower = name.to_ascii_lowercase();
        let matched = lower.contains("logit") || lower == "output";
        if let Ok((_shape, data)) = value.try_extract_tensor::<f32>() {
            if matched || logits.is_none() {
                logits = Some(data.to_vec());
            }
            if matched {
                break;
            }
        }
    }
    let logits = logits.ok_or_else(|| "ONNX 没有输出 logits。".to_string())?;
    Ok(score_logits(
        &logits,
        &engine.idx_to_tag,
        &engine.tag_to_category,
        general_thresh,
        character_thresh,
    ))
}

pub fn preprocess_image(image_base64: &str) -> Result<Vec<f32>, String> {
    let raw = image_base64
        .split(',')
        .last()
        .unwrap_or(image_base64)
        .trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|_| "图片不是有效的 Base64。".to_string())?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("无法解码图片：{e}"))?
        .resize_exact(INPUT_SIZE, INPUT_SIZE, FilterType::Triangle)
        .to_rgb8();
    let plane = (INPUT_SIZE * INPUT_SIZE) as usize;
    let mut pixels = vec![0f32; plane * 3];
    for (i, pixel) in img.pixels().enumerate() {
        pixels[i] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
        pixels[plane + i] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
        pixels[plane * 2 + i] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
    }
    Ok(pixels)
}

fn parse_vocab(bytes: &[u8]) -> Result<(Vec<String>, HashMap<String, String>), String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| format!("词表 JSON 无效：{e}"))?;
    let mut idx_to_tag = Vec::new();
    if let Some(map) = value.get("idx_to_tag").and_then(|v| v.as_object()) {
        let mut pairs: Vec<(usize, String)> = map
            .iter()
            .filter_map(|(k, v)| {
                let idx = k.parse::<usize>().ok()?;
                let tag = v.as_str()?.to_string();
                Some((idx, tag))
            })
            .collect();
        pairs.sort_by_key(|(idx, _)| *idx);
        let last = pairs.last().map(|(idx, _)| *idx).unwrap_or(0);
        idx_to_tag.resize(last.saturating_add(1), String::new());
        for (idx, tag) in pairs {
            idx_to_tag[idx] = tag;
        }
    } else if let Some(arr) = value.get("idx_to_tag").and_then(|v| v.as_array()) {
        idx_to_tag = arr
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
    }
    if idx_to_tag.is_empty() {
        return Err("词表缺少 idx_to_tag。".into());
    }
    let mut tag_to_category = HashMap::new();
    if let Some(map) = value.get("tag_to_category").and_then(|v| v.as_object()) {
        for (tag, cat) in map {
            if let Some(cat) = cat.as_str() {
                tag_to_category.insert(tag.clone(), cat.to_string());
            }
        }
    }
    Ok((idx_to_tag, tag_to_category))
}

pub fn sigmoid(x: f32) -> f64 {
    1.0 / (1.0 + (-x as f64).exp())
}

fn threshold_for(category: &str, general_thresh: f64, character_thresh: f64) -> f64 {
    match category {
        "Character" | "Copyright" => character_thresh.clamp(0.01, 0.99),
        _ => general_thresh.clamp(0.01, 0.99),
    }
}

pub fn score_logits(
    logits: &[f32],
    idx_to_tag: &[String],
    tag_to_category: &HashMap<String, String>,
    general_thresh: f64,
    character_thresh: f64,
) -> WdTagResult {
    let mut rating = Vec::new();
    let mut characters = Vec::new();
    let mut tags = Vec::new();
    let mut prompt_parts = Vec::new();
    let n = logits.len().min(idx_to_tag.len());
    for i in 0..n {
        let tag = idx_to_tag[i].trim();
        if tag.is_empty() {
            continue;
        }
        let conf = sigmoid(logits[i]);
        let category = tag_to_category
            .get(tag)
            .map(|s| s.as_str())
            .unwrap_or("General");
        let thr = threshold_for(category, general_thresh, character_thresh);
        if conf < thr {
            continue;
        }
        let item = LabelConfidence {
            label: tag.to_string(),
            confidence: conf,
        };
        match category {
            "Rating" => rating.push(item),
            "Character" => {
                prompt_parts.push(tag.to_string());
                characters.push(item);
            }
            _ => {
                prompt_parts.push(tag.to_string());
                tags.push(item);
            }
        }
    }
    rating.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    characters.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    tags.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    WdTagResult {
        prompt: prompt_parts.join(", "),
        rating: LabelGroup {
            label: Some("rating".into()),
            confidences: rating,
        },
        characters: LabelGroup {
            label: Some("character".into()),
            confidences: characters,
        },
        tags: LabelGroup {
            label: Some("general".into()),
            confidences: tags,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigmoid_midpoint() {
        let v = sigmoid(0.0);
        assert!((v - 0.5).abs() < 1e-6);
    }

    #[test]
    fn scores_by_category_and_threshold() {
        let idx = vec!["1girl".into(), "hatsune_miku".into(), "explicit".into(), "low".into()];
        let mut cats = HashMap::new();
        cats.insert("1girl".into(), "General".into());
        cats.insert("hatsune_miku".into(), "Character".into());
        cats.insert("explicit".into(), "Rating".into());
        let logits = [2.2f32, 2.2, 2.2, -4.0];
        let out = score_logits(&logits, &idx, &cats, 0.55, 0.60);
        assert!(out.prompt.contains("1girl"));
        assert!(out.prompt.contains("hatsune_miku"));
        assert_eq!(out.characters.confidences[0].label, "hatsune_miku");
        assert_eq!(out.rating.confidences[0].label, "explicit");
        assert!(out.tags.confidences.iter().all(|t| t.label != "low"));
    }

    #[test]
    fn parses_idx_map_vocab() {
        let json = br#"{
            "idx_to_tag": {"0": "1girl", "2": "solo"},
            "tag_to_category": {"1girl": "General", "solo": "General"}
        }"#;
        let (idx, cats) = parse_vocab(json).unwrap();
        assert_eq!(idx[0], "1girl");
        assert_eq!(idx[2], "solo");
        assert_eq!(cats.get("1girl").map(String::as_str), Some("General"));
    }

    #[test]
    fn endpoints_include_official_and_mirror() {
        let list = hf_endpoints();
        assert!(list.iter().any(|u| u.contains("huggingface.co")));
        assert!(list.iter().any(|u| u.contains("hf-mirror.com")));
    }

    #[test]
    fn skips_token_on_cdn_hosts() {
        assert!(host_needs_hf_token("https://huggingface.co/cella110n/cl_tagger_v2/resolve/main/v2_00/model.onnx"));
        assert!(host_needs_hf_token("https://hf-mirror.com/cella110n/cl_tagger_v2/resolve/main/v2_00/model.onnx"));
        assert!(!host_needs_hf_token("https://cdn-lfs.huggingface.co/repos/xx/model.onnx"));
        assert!(!host_needs_hf_token("https://cas-bridge.xethub.hf.co/xet/foo"));
    }

    #[test]
    fn rejects_html_and_lfs_pointer() {
        assert!(looks_like_html(b"<!DOCTYPE html><html>"));
        assert!(looks_like_lfs_pointer(b"version https://git-lfs.github.com/spec/v1\noid sha256:abc"));
        assert!(!looks_like_html(b"{\"idx_to_tag\":{}}"));
        assert!(should_retry_message("tcp connect error (os error 10060)"));
    }
}
