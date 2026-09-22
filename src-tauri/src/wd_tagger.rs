use crate::nai::{format_reqwest, http_client};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const SPACE: &str = "https://smilingwolf-wd-tagger.hf.space";
const CANCELLED: &str = "已取消反推";

#[derive(Default)]
struct JobGate {
    active: HashSet<String>,
    cancelled: HashSet<String>,
}

fn gate() -> &'static Mutex<JobGate> {
    static GATE: OnceLock<Mutex<JobGate>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(JobGate::default()))
}

fn lock_gate() -> std::sync::MutexGuard<'static, JobGate> {
    gate().lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTagRequest {
    pub image_base64: String,
    pub model: Option<String>,
    pub general_thresh: Option<f64>,
    pub general_mcut: Option<bool>,
    pub character_thresh: Option<f64>,
    pub character_mcut: Option<bool>,
    pub job_id: Option<String>,
    pub hf_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelConfidence {
    pub label: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LabelGroup {
    pub label: Option<String>,
    pub confidences: Vec<LabelConfidence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTagResult {
    pub prompt: String,
    pub rating: LabelGroup,
    pub characters: LabelGroup,
    pub tags: LabelGroup,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WdTagProgress {
    pub job_id: String,
    pub progress: f64,
    pub phase: String,
    pub message: String,
}

fn emit_wd(app: &AppHandle, job_id: &str, progress: f64, phase: &str, message: &str) {
    let _ = app.emit(
        "wd-tag-progress",
        WdTagProgress {
            job_id: job_id.into(),
            progress: progress.clamp(0.0, 1.0),
            phase: phase.into(),
            message: message.into(),
        },
    );
}

fn set_floor(floor: &AtomicU32, value: f64) {
    floor.store((value.clamp(0.0, 0.95) * 1000.0).round() as u32, Ordering::Relaxed);
}

fn strip_data_url(value: &str) -> String {
    value
        .split(',')
        .next_back()
        .unwrap_or(value)
        .replace(['\n', '\r', ' '], "")
}

fn parse_group(value: &Value) -> LabelGroup {
    let label = value
        .get("label")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let confidences = value
        .get("confidences")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(LabelConfidence {
                        label: item.get("label")?.as_str()?.to_string(),
                        confidence: item.get("confidence")?.as_f64().unwrap_or(0.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    LabelGroup { label, confidences }
}

fn parse_result(data: &Value) -> Result<WdTagResult, String> {
    let arr = data.as_array().ok_or("反推结果格式无效")?;
    if arr.len() < 4 {
        return Err("反推结果不完整".into());
    }
    Ok(WdTagResult {
        prompt: arr[0].as_str().unwrap_or("").to_string(),
        rating: parse_group(&arr[1]),
        characters: parse_group(&arr[2]),
        tags: parse_group(&arr[3]),
    })
}

fn try_parse_sse(body: &str) -> Result<WdTagResult, String> {
    let mut event = String::new();
    let mut last_error = "反推没有返回结果".to_string();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            let payload = rest.trim();
            if payload.is_empty() {
                continue;
            }
            if event == "error" {
                last_error = payload.to_string();
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<Value>(payload) {
                if let Ok(result) = parse_result(&parsed) {
                    return Ok(result);
                }
                if let Some(inner) = parsed.get("data") {
                    if let Ok(result) = parse_result(inner) {
                        return Ok(result);
                    }
                }
            }
        }
    }
    Err(last_error)
}

fn begin_job(id: &str) {
    let mut g = lock_gate();
    g.cancelled.remove(id);
    g.active.insert(id.to_string());
}

fn finish_job(id: &str) {
    let mut g = lock_gate();
    g.active.remove(id);
}

fn is_cancelled(id: &str) -> bool {
    lock_gate().cancelled.contains(id)
}

async fn wait_cancel(id: String) {
    while !is_cancelled(&id) {
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

async fn or_cancel<T>(id: &str, fut: impl std::future::Future<Output = T>) -> Result<T, String> {
    tokio::select! {
        _ = wait_cancel(id.to_string()) => Err(CANCELLED.into()),
        result = fut => Ok(result),
    }
}

const CL_SPACE: &str = "https://cella110n-cl-tagger-v2.hf.space";
const CL_CATS: [&str; 7] = ["Quality", "Rating", "Character", "Copyright", "General", "Meta", "Model"];

fn is_cl_tagger(model: &str) -> bool {
    let name = model.to_ascii_lowercase();
    name.contains("cl_tagger") || name.contains("cl-tagger")
}

fn clamp_thr(value: f64) -> f64 {
    (value.clamp(0.01, 0.99) * 100.0).round() / 100.0
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn last_category(window: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    for name in CL_CATS {
        for needle in [format!(">{name}<"), format!(">{name} (")] {
            if let Some(pos) = window.rfind(&needle) {
                if best.map(|(_, at)| pos >= at).unwrap_or(true) {
                    best = Some((name, pos));
                }
            }
        }
    }
    best.map(|(name, _)| name)
}

fn parse_cl_html(html: &str) -> Result<WdTagResult, String> {
    if html.contains("Model not loaded") {
        return Err("CL Tagger 模型还没加载".into());
    }
    if html.contains("Upload an image") {
        return Err("CL Tagger 没有收到图片".into());
    }
    let prompt = html
        .find("data-tags=\"")
        .and_then(|at| {
            let start = at + "data-tags=\"".len();
            html[start..].find('"').map(|end| html_unescape(&html[start..start + end]))
        })
        .unwrap_or_default();
    let mut category = "General";
    let mut cursor = 0usize;
    let mut characters = Vec::new();
    let mut rating = Vec::new();
    let mut tags = Vec::new();
    while let Some(rel) = html[cursor..].find("data-raw=\"") {
        let at = cursor + rel;
        if let Some(next) = last_category(&html[cursor..at]) {
            category = next;
        }
        let num_at = at + "data-raw=\"".len();
        let Some(num_end) = html[num_at..].find('"') else { break };
        let pct: f64 = html[num_at..num_at + num_end].parse().unwrap_or(0.0);
        let after = num_at + num_end;
        let Some(title_rel) = html[after..].find("title=\"") else { break };
        let title_at = after + title_rel + "title=\"".len();
        let Some(title_end) = html[title_at..].find('"') else { break };
        let label = html_unescape(&html[title_at..title_at + title_end]);
        let item = LabelConfidence {
            label,
            confidence: (pct / 100.0).clamp(0.0, 1.0),
        };
        match category {
            "Character" => characters.push(item),
            "Rating" => rating.push(item),
            _ => tags.push(item),
        }
        cursor = title_at + title_end;
    }
    if characters.is_empty() && rating.is_empty() && tags.is_empty() {
        if html.contains("No tags above threshold") {
            return Ok(WdTagResult {
                prompt,
                rating: LabelGroup::default(),
                characters: LabelGroup::default(),
                tags: LabelGroup::default(),
            });
        }
        let snippet: String = html.chars().take(160).collect();
        return Err(format!("CL Tagger 没有识别出标签：{snippet}"));
    }
    Ok(WdTagResult {
        prompt,
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
    })
}

#[cfg(test)]
fn sse_payload_complete(body: &str, event: &str) -> bool {
    let marker = format!("event: {event}");
    let Some(pos) = body.rfind(&marker) else {
        return false;
    };
    let rest = &body[pos + marker.len()..];
    let Some(rel) = rest.find("data:") else {
        return false;
    };
    let payload = rest[rel + "data:".len()..].trim_start();
    if payload.is_empty() {
        return false;
    }
    let mut de = serde_json::Deserializer::from_str(payload);
    Value::deserialize(&mut de).is_ok() && de.end().is_ok()
}

#[cfg(test)]
fn sse_error(body: &str) -> Option<String> {
    let mut event = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if event == "error" {
            if let Some(rest) = line.strip_prefix("data:") {
                let text = rest.trim().trim_matches('"').to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

#[cfg(test)]
fn sse_html(body: &str) -> Result<String, String> {
    if let Some(err) = sse_error(body) {
        return Err(err);
    }
    let mut event = String::new();
    let mut last = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
            continue;
        }
        let Some(rest) = line.strip_prefix("data:") else { continue };
        let payload = rest.trim();
        if payload.is_empty() || event == "error" {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(payload) {
            if let Some(text) = parsed.as_str() {
                last = text.to_string();
            } else if let Some(text) = parsed.as_array().and_then(|items| items.first()).and_then(|v| v.as_str()) {
                last = text.to_string();
            } else if let Some(text) = parsed.pointer("/0").and_then(|v| v.as_str()) {
                last = text.to_string();
            }
        }
    }
    if last.is_empty() {
        return Err("CL Tagger 没有返回结果，空间可能在冷启动。".into());
    }
    Ok(last)
}

fn explain_cl_error(err: &str) -> String {
    let err = err.trim().trim_matches('"');
    if err.is_empty() || err == "null" {
        return "CL Tagger 没有返回结果。免费 GPU 额度可能已用完，填入 Hugging Face token 后再试。".into();
    }
    if err.contains("ZeroGPU") || err.to_ascii_lowercase().contains("quota") {
        let wait = err
            .split("Try again in ")
            .nth(1)
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .trim();
        if wait.is_empty() || wait.starts_with("0:00:00") {
            return "CL Tagger 的免费 GPU 额度用完了。打开 huggingface.co/settings/tokens 建一个 read token，填到反推页后再试。".into();
        }
        return format!("CL Tagger 的免费 GPU 额度用完了，大约 {wait} 后恢复。填入 Hugging Face token 可以马上继续。");
    }
    format!("CL Tagger 失败：{err}")
}

fn hf_token(raw: &str) -> String {
    raw.trim().trim_start_matches("Bearer ").trim().to_string()
}

fn with_hf_token(builder: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    let token = hf_token(token);
    if token.is_empty() {
        builder
    } else {
        builder.header("Authorization", format!("Bearer {token}"))
    }
}

fn cl_image_value(raw_b64: &str) -> Value {
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw_b64).ok();
    if let Some(bytes) = decoded {
        if let Ok(img) = image::load_from_memory(&bytes) {
            let max_side = 1280u32;
            let (w, h) = (img.width(), img.height());
            let long = w.max(h).max(1);
            let rgb = if long > max_side {
                let nw = ((w as f32) * (max_side as f32) / (long as f32)).round().max(1.0) as u32;
                let nh = ((h as f32) * (max_side as f32) / (long as f32)).round().max(1.0) as u32;
                img.resize(nw, nh, image::imageops::FilterType::Triangle).to_rgb8()
            } else {
                img.to_rgb8()
            };
            let mut buf = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut buf);
            if image::DynamicImage::ImageRgb8(rgb)
                .write_to(&mut cursor, image::ImageFormat::Jpeg)
                .is_ok()
            {
                let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf);
                return json!({
                    "url": format!("data:image/jpeg;base64,{encoded}"),
                    "orig_name": "reverse.jpg",
                    "meta": { "_type": "gradio.FileData" }
                });
            }
        }
    }
    json!({
        "url": format!("data:image/png;base64,{raw_b64}"),
        "orig_name": "reverse.png",
        "meta": { "_type": "gradio.FileData" }
    })
}

async fn cl_fn_index(client: &reqwest::Client, token: &str, api_name: &str) -> Result<i64, String> {
    let response = with_hf_token(
        client
            .get(format!("{CL_SPACE}/config"))
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(60)),
        token,
    )
    .send()
    .await
    .map_err(format_reqwest)?;
    if !response.status().is_success() {
        return Err(format!("CL Tagger 配置失败 HTTP {}", response.status()));
    }
    let config: Value = response.json().await.map_err(format_reqwest)?;
    let deps = config
        .get("dependencies")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "CL Tagger 配置里没有接口列表".to_string())?;
    deps.iter()
        .position(|dep| dep.get("api_name").and_then(|v| v.as_str()) == Some(api_name))
        .map(|index| index as i64)
        .ok_or_else(|| format!("CL Tagger 没有 {api_name} 接口"))
}

fn queue_line_value(line: &str) -> Option<Value> {
    let payload = line.trim().strip_prefix("data:")?.trim();
    if payload.is_empty() {
        return None;
    }
    serde_json::from_str(payload).ok()
}

fn queue_result(msg: &Value) -> Result<Option<Value>, String> {
    let kind = msg.get("msg").and_then(|v| v.as_str()).unwrap_or("");
    if kind != "process_completed" {
        return Ok(None);
    }
    if msg.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let err = msg
            .pointer("/output/error")
            .and_then(|v| v.as_str())
            .or_else(|| msg.get("title").and_then(|v| v.as_str()))
            .unwrap_or("null");
        return Err(explain_cl_error(err));
    }
    Ok(Some(msg.pointer("/output/data").cloned().unwrap_or(Value::Null)))
}

async fn cl_queue(
    client: &reqwest::Client,
    app: &AppHandle,
    id: &str,
    stop: &AtomicBool,
    token: &str,
    fn_index: i64,
    data: Value,
) -> Result<Value, String> {
    if is_cancelled(id) {
        return Err(stop_cancelled(app, stop, id));
    }
    let session = Uuid::new_v4().simple().to_string();
    let payload = json!({
        "data": data,
        "fn_index": fn_index,
        "session_hash": session,
        "trigger_id": fn_index
    });
    let started = or_cancel(
        id,
        with_hf_token(
            client
                .post(format!("{CL_SPACE}/gradio_api/queue/join"))
                .header("Accept", "application/json")
                .timeout(Duration::from_secs(300))
                .json(&payload),
            token,
        )
        .send(),
    )
    .await?
    .map_err(format_reqwest)?;
    if !started.status().is_success() {
        return Err(format!("CL Tagger 启动失败 HTTP {}", started.status()));
    }
    let stream = or_cancel(
        id,
        with_hf_token(
            client
                .get(format!("{CL_SPACE}/gradio_api/queue/data?session_hash={session}"))
                .header("Accept", "text/event-stream")
                .timeout(Duration::from_secs(300)),
            token,
        )
        .send(),
    )
    .await?
    .map_err(format_reqwest)?;
    if !stream.status().is_success() {
        return Err(format!("CL Tagger 结果失败 HTTP {}", stream.status()));
    }
    let mut chunks = stream.bytes_stream();
    let mut pending = String::new();
    loop {
        let item = tokio::select! {
            _ = wait_cancel(id.to_string()) => {
                return Err(stop_cancelled(app, stop, id));
            }
            item = chunks.next() => item,
        };
        let Some(item) = item else { break };
        let chunk = item.map_err(|e| e.to_string())?;
        pending.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(index) = pending.find('\n') {
            let line = pending.drain(..=index).collect::<String>();
            let Some(msg) = queue_line_value(line.trim()) else { continue };
            if msg.get("msg").and_then(|v| v.as_str()) == Some("estimation") {
                emit_wd(app, id, 0.2, "queue", "正在排队使用 CL Tagger…");
            }
            if let Some(data) = queue_result(&msg)? {
                return Ok(data);
            }
        }
    }
    if is_cancelled(id) {
        return Err(stop_cancelled(app, stop, id));
    }
    if let Some(msg) = queue_line_value(pending.trim()) {
        if let Some(data) = queue_result(&msg)? {
            return Ok(data);
        }
    }
    Err(explain_cl_error("null"))
}

fn first_text(data: &Value) -> String {
    data.as_array()
        .and_then(|items| items.first())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

async fn cl_tag_image(
    client: &reqwest::Client,
    app: &AppHandle,
    id: &str,
    floor: &AtomicU32,
    stop: &AtomicBool,
    raw: &str,
    general_thresh: f64,
    character_thresh: f64,
    token: &str,
) -> Result<WdTagResult, String> {
    let image = cl_image_value(raw);
    let general = clamp_thr(general_thresh);
    let character = clamp_thr(character_thresh);
    let predict_data = json!([
        image,
        "Per Category",
        general,
        general,
        character,
        false,
        "jeffreys",
        0.5,
        10.0,
        "Fixed threshold",
        false,
        0.3,
        0.05,
        true
    ]);
    emit_wd(app, id, 0.14, "upload", "正在提交到 CL Tagger v2…");
    set_floor(floor, 0.14);
    let predict_index = cl_fn_index(client, token, "_run_predict").await?;
    let mut html = first_text(&cl_queue(client, app, id, stop, token, predict_index, predict_data.clone()).await?);
    if html.contains("Model not loaded") {
        emit_wd(app, id, 0.22, "queue", "正在加载 CL Tagger v2…");
        set_floor(floor, 0.22);
        let load_index = cl_fn_index(client, token, "_load_and_reset").await?;
        let loaded = first_text(
            &cl_queue(
                client,
                app,
                id,
                stop,
                token,
                load_index,
                json!(["cella110n/cl_tagger_v2", "v2_01a"]),
            )
            .await?,
        );
        if loaded.contains("Download failed") || loaded.contains("Load failed") {
            return Err("CL Tagger 模型下载失败，请稍后再试。".into());
        }
        emit_wd(app, id, 0.4, "waiting", "模型已加载，正在识别标签…");
        set_floor(floor, 0.4);
        html = first_text(&cl_queue(client, app, id, stop, token, predict_index, predict_data).await?);
    }
    if html.is_empty() {
        return Err(explain_cl_error("null"));
    }
    emit_wd(app, id, 0.9, "parsing", "正在解析标签…");
    parse_cl_html(&html)
}

fn stop_cancelled(app: &AppHandle, stop: &AtomicBool, id: &str) -> String {
    stop.store(true, Ordering::Relaxed);
    finish_job(id);
    emit_wd(app, id, 0.0, "cancelled", CANCELLED);
    CANCELLED.into()
}

#[tauri::command]
pub fn wd_tag_cancel(job_id: Option<String>) {
    let mut g = lock_gate();
    if let Some(id) = job_id.filter(|item| !item.is_empty()) {
        g.cancelled.insert(id);
        return;
    }
    let active: Vec<String> = g.active.iter().cloned().collect();
    for id in active {
        g.cancelled.insert(id);
    }
}

fn start_ticker(app: AppHandle, floor: Arc<AtomicU32>, stop: Arc<AtomicBool>, id: String) {
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        while !stop.load(Ordering::Relaxed) && !is_cancelled(&id) {
            tokio::time::sleep(Duration::from_millis(180)).await;
            if stop.load(Ordering::Relaxed) || is_cancelled(&id) {
                break;
            }
            let base = floor.load(Ordering::Relaxed) as f64 / 1000.0;
            let t = started.elapsed().as_secs_f64();
            let progress = (base + (1.0 - (-t / 26.0).exp()) * (0.88 - base)).clamp(base, 0.88);
            let message = if t < 8.0 {
                "正在提交并排队…"
            } else if t < 25.0 {
                "正在识别标签…"
            } else {
                "空间可能在冷启动，仍在等待…"
            };
            emit_wd(&app, &id, progress, "waiting", message);
        }
    });
}

#[tauri::command]
pub async fn wd_tag_image(app: AppHandle, request: WdTagRequest) -> Result<WdTagResult, String> {
    let raw = strip_data_url(&request.image_base64);
    if raw.is_empty() {
        return Err("请先选择要反推的图片。".into());
    }
    let id = request
        .job_id
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    begin_job(&id);
    let floor = Arc::new(AtomicU32::new(80));
    let stop = Arc::new(AtomicBool::new(false));
    emit_wd(&app, &id, 0.06, "prepare", "正在准备图片…");
    start_ticker(app.clone(), floor.clone(), stop.clone(), id.clone());
    let model = request
        .model
        .unwrap_or_else(|| "SmilingWolf/wd-swinv2-tagger-v3".into());
    let general_thresh = request.general_thresh.unwrap_or(0.35);
    let character_thresh = request.character_thresh.unwrap_or(0.85);
    if is_cl_tagger(&model) {
        let local_enabled = crate::store::load_settings().local_cl_tagger_enabled;
        if local_enabled {
            emit_wd(&app, &id, 0.18, "local", "正在使用本机 CL Tagger v2…");
            set_floor(&floor, 0.18);
            if is_cancelled(&id) {
                return Err(stop_cancelled(&app, &stop, &id));
            }
            let raw_owned = raw.clone();
            let tagged = tokio::task::spawn_blocking(move || {
                crate::cl_tagger_local::tag_image(&raw_owned, general_thresh, character_thresh)
            })
            .await
            .map_err(|e| {
                stop.store(true, Ordering::Relaxed);
                finish_job(&id);
                format!("本地打标线程失败：{e}")
            })?;
            return match tagged {
                Ok(result) => {
                    if is_cancelled(&id) {
                        return Err(stop_cancelled(&app, &stop, &id));
                    }
                    stop.store(true, Ordering::Relaxed);
                    finish_job(&id);
                    emit_wd(&app, &id, 1.0, "done", "本地反推完成");
                    Ok(result)
                }
                Err(err) => {
                    stop.store(true, Ordering::Relaxed);
                    finish_job(&id);
                    Err(err)
                }
            };
        }
        let client = match http_client() {
            Ok(c) => c,
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                finish_job(&id);
                return Err(e);
            }
        };
        let hf_token = request
            .hf_token
            .as_deref()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .unwrap_or_else(crate::store::get_huggingface_token);
        return match cl_tag_image(
            &client,
            &app,
            &id,
            &floor,
            &stop,
            &raw,
            general_thresh,
            character_thresh,
            &hf_token,
        )
        .await
        {
            Ok(result) => {
                if is_cancelled(&id) {
                    return Err(stop_cancelled(&app, &stop, &id));
                }
                stop.store(true, Ordering::Relaxed);
                finish_job(&id);
                emit_wd(&app, &id, 1.0, "done", "反推完成");
                Ok(result)
            }
            Err(err) if err == CANCELLED => Err(err),
            Err(err) => {
                stop.store(true, Ordering::Relaxed);
                finish_job(&id);
                Err(err)
            }
        };
    }
    let image = json!({
        "url": format!("data:image/png;base64,{raw}"),
        "orig_name": "reverse.png",
        "meta": { "_type": "gradio.FileData" }
    });
    let client = match http_client() {
        Ok(c) => c,
        Err(e) => {
            finish_job(&id);
            return Err(e);
        }
    };
    let payload = json!({
        "data": [
            image,
            model,
            general_thresh,
            request.general_mcut.unwrap_or(false),
            character_thresh,
            request.character_mcut.unwrap_or(false)
        ]
    });
    emit_wd(&app, &id, 0.14, "upload", "正在提交到 Tagger…");
    set_floor(&floor, 0.14);
    if is_cancelled(&id) {
        return Err(stop_cancelled(&app, &stop, &id));
    }
    let started = or_cancel(
        &id,
        client
            .post(format!("{SPACE}/gradio_api/call/predict"))
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(180))
            .json(&payload)
            .send(),
    )
    .await
    .map_err(|e| {
        stop.store(true, Ordering::Relaxed);
        finish_job(&id);
        e
    })?
    .map_err(|e| {
        stop.store(true, Ordering::Relaxed);
        finish_job(&id);
        format_reqwest(e)
    })?;
    if is_cancelled(&id) {
        return Err(stop_cancelled(&app, &stop, &id));
    }
    if !started.status().is_success() {
        stop.store(true, Ordering::Relaxed);
        finish_job(&id);
        return Err(format!("反推启动失败 HTTP {}", started.status()));
    }
    let started_json: Value = or_cancel(&id, started.json())
        .await
        .map_err(|e| {
            stop.store(true, Ordering::Relaxed);
            finish_job(&id);
            e
        })?
        .map_err(|e| {
            stop.store(true, Ordering::Relaxed);
            finish_job(&id);
            format_reqwest(e)
        })?;
    if is_cancelled(&id) {
        return Err(stop_cancelled(&app, &stop, &id));
    }
    let event_id = started_json
        .get("event_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            stop.store(true, Ordering::Relaxed);
            finish_job(&id);
            "反推队列没有返回 event_id，空间可能在冷启动，请稍后再试。".to_string()
        })?;
    emit_wd(&app, &id, 0.28, "queue", "已进入队列，等待识别…");
    set_floor(&floor, 0.28);
    let stream = or_cancel(
        &id,
        client
            .get(format!("{SPACE}/gradio_api/call/predict/{event_id}"))
            .header("Accept", "text/event-stream")
            .timeout(std::time::Duration::from_secs(180))
            .send(),
    )
    .await
    .map_err(|e| {
        stop.store(true, Ordering::Relaxed);
        finish_job(&id);
        e
    })?
    .map_err(|e| {
        stop.store(true, Ordering::Relaxed);
        finish_job(&id);
        format_reqwest(e)
    })?;
    if is_cancelled(&id) {
        return Err(stop_cancelled(&app, &stop, &id));
    }
    if !stream.status().is_success() {
        stop.store(true, Ordering::Relaxed);
        finish_job(&id);
        return Err(format!("反推结果失败 HTTP {}", stream.status()));
    }
    emit_wd(&app, &id, 0.36, "waiting", "正在识别标签…");
    set_floor(&floor, 0.36);
    let mut chunks = stream.bytes_stream();
    let mut body = String::new();
    loop {
        let item = tokio::select! {
            _ = wait_cancel(id.clone()) => {
                return Err(stop_cancelled(&app, &stop, &id));
            }
            item = chunks.next() => item,
        };
        let Some(item) = item else { break };
        let chunk = item.map_err(|e| {
            stop.store(true, Ordering::Relaxed);
            finish_job(&id);
            e.to_string()
        })?;
        body.push_str(&String::from_utf8_lossy(&chunk));
        if let Ok(result) = try_parse_sse(&body) {
            if is_cancelled(&id) {
                return Err(stop_cancelled(&app, &stop, &id));
            }
            stop.store(true, Ordering::Relaxed);
            finish_job(&id);
            emit_wd(&app, &id, 0.95, "parsing", "正在解析标签…");
            emit_wd(&app, &id, 1.0, "done", "反推完成");
            return Ok(result);
        }
    }
    if is_cancelled(&id) {
        return Err(stop_cancelled(&app, &stop, &id));
    }
    stop.store(true, Ordering::Relaxed);
    let result = try_parse_sse(&body).inspect_err(|_| finish_job(&id))?;
    finish_job(&id);
    emit_wd(&app, &id, 0.95, "parsing", "正在解析标签…");
    emit_wd(&app, &id, 1.0, "done", "反推完成");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{explain_cl_error, parse_cl_html, queue_result, sse_html, sse_payload_complete};

    #[test]
    fn splits_cl_html_by_category() {
        let html = r#"<button data-tags="best_quality, sensitive, hatsune_miku, 1girl">
<div style="font-size:11px">Quality</div>
<div class="tag-bar" data-raw="91.20"><span title="best_quality">best_quality</span></div>
<div style="font-size:11px">Rating</div>
<div class="tag-bar" data-raw="80.00"><span title="sensitive">sensitive</span></div>
<summary>Character (1)</summary>
<div class="tag-bar" data-raw="77.50"><span title="hatsune_miku">hatsune_miku</span></div>
<summary>General (1)</summary>
<div class="tag-bar" data-raw="66.00"><span title="1girl">1girl</span></div>"#;
        let parsed = parse_cl_html(html).unwrap();
        assert_eq!(parsed.prompt, "best_quality, sensitive, hatsune_miku, 1girl");
        assert_eq!(parsed.rating.confidences[0].label, "sensitive");
        assert_eq!(parsed.characters.confidences[0].label, "hatsune_miku");
        assert_eq!(parsed.tags.confidences[0].label, "best_quality");
        assert_eq!(parsed.tags.confidences[1].label, "1girl");
        assert!((parsed.tags.confidences[0].confidence - 0.912).abs() < 0.001);
    }

    #[test]
    fn cl_sse_waits_until_json_finishes() {
        let partial = "event: complete\ndata: [\"<div data-raw=\\\"86";
        assert!(!sse_payload_complete(partial, "complete"));
        let full = "event: heartbeat\ndata: null\n\nevent: complete\ndata: [\"<button data-tags=\\\"1girl\\\"><div class=\\\"tag-bar\\\" data-raw=\\\"80.00\\\"><span title=\\\"1girl\\\">1girl</span></div>\"]\n";
        assert!(sse_payload_complete(full, "complete"));
        let html = sse_html(full).unwrap();
        let parsed = parse_cl_html(&html).unwrap();
        assert_eq!(parsed.tags.confidences[0].label, "1girl");
    }

    #[test]
    fn cl_quota_error_is_readable() {
        let msg = serde_json::json!({
            "msg": "process_completed",
            "success": false,
            "output": { "error": "You have exceeded your ZeroGPU quota (90s requested vs. 0s left). Try again in 0:00:00." }
        });
        let err = queue_result(&msg).unwrap_err();
        assert!(err.contains("免费 GPU 额度"));
        assert!(explain_cl_error("null").contains("Hugging Face token"));
    }
}
