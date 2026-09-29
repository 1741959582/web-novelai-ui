use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde_json::Value;

use crate::nai::http_client_timeout;
use crate::store::{default_output_dir, get_reapi_token};

const API: &str = "https://reapi.ai/api/v1";

fn token() -> Result<String, String> {
    let token = get_reapi_token();
    if token.is_empty() {
        return Err("先在设置里填写 reAPI Key（rk_live_...）".into());
    }
    Ok(token)
}

fn api_error(status: reqwest::StatusCode, body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        let message = value
            .pointer("/error/message")
            .and_then(|item| item.as_str())
            .or_else(|| value.get("message").and_then(|item| item.as_str()));
        if let Some(message) = message {
            return format!("HTTP {status}: {message}");
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {}", trimmed.chars().take(400).collect::<String>())
    }
}

fn task_id(id: &str) -> Result<String, String> {
    let id = id.trim();
    if id.is_empty() || id.len() > 80 || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
        return Err("任务编号无效".into());
    }
    Ok(id.to_string())
}

#[tauri::command]
pub async fn reapi_video_submit(body: Value) -> Result<Value, String> {
    let token = token()?;
    let client = http_client_timeout(60)?;
    let response = client
        .post(format!("{API}/videos/generations"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    serde_json::from_str(&text).map_err(|error| format!("返回不是 JSON：{error}"))
}

#[tauri::command]
pub async fn reapi_video_task(id: String) -> Result<Value, String> {
    let token = token()?;
    let id = task_id(&id)?;
    let client = http_client_timeout(30)?;
    let response = client
        .get(format!("{API}/tasks/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    serde_json::from_str(&text).map_err(|error| format!("返回不是 JSON：{error}"))
}

#[tauri::command]
pub async fn reapi_publish_image(source: String) -> Result<String, String> {
    let source = source.trim();
    if source.starts_with("https://") || source.starts_with("http://") {
        return Ok(source.to_string());
    }
    let (bytes, name, mime) = image_bytes(source)?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("图片超过 10MB".into());
    }
    let client = http_client_timeout(120)?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(name)
        .mime_str(&mime)
        .map_err(|error| error.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("outputFormat", "auto")
        .text("cdn_domain", "default")
        .text("storage_destination", "local")
        .part("image", part);
    let response = client
        .post("https://img.scdn.io/api/upload.php")
        .header("accept", "*/*")
        .header("origin", "https://img.scdn.io")
        .header("referer", "https://img.scdn.io/")
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("上传参考图失败：{error}"))?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("上传参考图失败：HTTP {status}"));
    }
    let body: Value = serde_json::from_str(&text).map_err(|error| format!("上传返回不是 JSON：{error}"))?;
    if body.get("success").and_then(|item| item.as_bool()) == Some(false) {
        let message = body.get("message").and_then(|item| item.as_str()).unwrap_or("上传失败");
        return Err(message.to_string());
    }
    let url = body
        .get("url")
        .and_then(|item| item.as_str())
        .or_else(|| body.pointer("/data/url").and_then(|item| item.as_str()))
        .unwrap_or("")
        .trim()
        .to_string();
    if !url.starts_with("http") {
        return Err("上传成功但没有图片地址".into());
    }
    Ok(url)
}

fn image_bytes(source: &str) -> Result<(Vec<u8>, String, String), String> {
    if let Some(rest) = source.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',').ok_or("图片数据不完整")?;
        if !meta.contains("base64") {
            return Err("图片数据不完整".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|error| format!("图片数据读不出来：{error}"))?;
        let ext = if meta.contains("jpeg") || meta.contains("jpg") {
            "jpg"
        } else if meta.contains("webp") {
            "webp"
        } else if meta.contains("gif") {
            "gif"
        } else {
            "png"
        };
        return Ok((bytes, format!("frame.{ext}"), mime_for(ext)));
    }
    let path = PathBuf::from(source);
    let bytes = std::fs::read(&path).map_err(|error| format!("读不到图片：{error}"))?;
    let name = path
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("image.png")
        .to_string();
    let ext = path.extension().and_then(|item| item.to_str()).unwrap_or("png");
    Ok((bytes, name, mime_for(ext)))
}

fn mime_for(ext: &str) -> String {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "image/png",
    }
    .into()
}

#[tauri::command]
pub async fn reapi_save_video(url: String) -> Result<String, String> {
    let url = url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("视频地址无效".into());
    }
    let client = http_client_timeout(600)?;
    let response = client.get(url).send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("下载视频失败：HTTP {status}"));
    }
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    let dir = default_output_dir().join("videos");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|item| item.as_secs()).unwrap_or(0);
    let path = dir.join(format!("seedance-{stamp}.mp4"));
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
