//! 酒馆（AI 对话 + 生图）后端：
//! - 本地 JSON 存储（角色卡、预设、聊天记录）
//! - OpenAI 兼容的 Chat Completions 调用（xAI / DeepSeek / 自定义），支持流式输出与中止
//! - 模型列表获取、文件导出

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;

use base64::Engine;

fn tavern_dir() -> Result<PathBuf, String> {
    let dir = crate::store::data_dir()?.join("tavern");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// 只允许字母数字、下划线、短横线，防止路径穿越。
fn safe_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 96 {
        return Err("无效的存储名".into());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("无效的存储名: {name}"));
    }
    Ok(name.to_string())
}

fn file_for(name: &str) -> Result<PathBuf, String> {
    Ok(tavern_dir()?.join(format!("{}.json", safe_name(name)?)))
}

#[tauri::command]
pub fn tavern_read(name: String) -> Result<Value, String> {
    let path = file_for(&name)?;
    if !path.exists() {
        return Ok(Value::Null);
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("读取 {name} 失败：{e}"))
}

#[tauri::command]
pub fn tavern_write(name: String, data: Value) -> Result<(), String> {
    let path = file_for(&name)?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tavern_remove(name: String) -> Result<(), String> {
    let path = file_for(&name)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn tavern_open_dir() -> Result<(), String> {
    let dir = tavern_dir()?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
    Ok(())
}

/// 弹出保存对话框，把 base64 内容写入用户选择的位置。返回保存路径（取消时为 None）。
#[tauri::command]
pub async fn tavern_export_file(app: AppHandle, file_name: String, base64_data: String) -> Result<Option<String>, String> {
    let raw = base64_data
        .split_once(',')
        .map(|(_, b)| b.to_string())
        .unwrap_or(base64_data);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .map_err(|e| format!("导出数据无效：{e}"))?;
    let picked = app.dialog().file().set_file_name(&file_name).blocking_save_file();
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

// ---------------------------------------------------------------------------
// LLM
// ---------------------------------------------------------------------------

fn cancel_set() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn is_cancelled(id: &str) -> bool {
    cancel_set().lock().map(|s| s.contains(id)).unwrap_or(false)
}

fn clear_cancel(id: &str) {
    if let Ok(mut s) = cancel_set().lock() {
        s.remove(id);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmRequest {
    pub request_id: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub messages: Vec<Value>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LlmDelta {
    request_id: String,
    content: String,
    reasoning: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmResult {
    pub content: String,
    pub reasoning: String,
    pub finish_reason: String,
}

fn endpoint(base: &str, path: &str) -> Result<String, String> {
    let base = base.trim().trim_end_matches('/');
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err("API 地址需要以 http:// 或 https:// 开头".into());
    }
    Ok(format!("{base}/{path}"))
}

fn status_error(status: reqwest::StatusCode, body: &str) -> String {
    let short: String = body.chars().take(600).collect();
    match status.as_u16() {
        401 | 403 => format!("API Key 无效或无权限（HTTP {status}）：{short}"),
        402 => format!("账户余额不足（HTTP {status}）：{short}"),
        404 => format!("接口或模型不存在，请检查 API 地址和模型名（HTTP {status}）：{short}"),
        429 => format!("请求过于频繁或额度用尽（HTTP {status}）：{short}"),
        _ => format!("对话请求失败 HTTP {status}：{short}"),
    }
}

fn str_at<'a>(v: &'a Value, path: &[&str]) -> &'a str {
    let mut cur = v;
    for key in path {
        cur = match cur.get(*key) {
            Some(next) => next,
            None => return "",
        };
    }
    cur.as_str().unwrap_or("")
}

fn delta_parts(choice: &Value, key: &str) -> (String, String) {
    let obj = choice.get(key).cloned().unwrap_or(Value::Null);
    let content = str_at(&obj, &["content"]).to_string();
    // DeepSeek: reasoning_content；部分兼容服务：reasoning
    let mut reasoning = str_at(&obj, &["reasoning_content"]).to_string();
    if reasoning.is_empty() {
        reasoning = str_at(&obj, &["reasoning"]).to_string();
    }
    (content, reasoning)
}

#[tauri::command]
pub fn tavern_llm_cancel(request_id: String) {
    if let Ok(mut s) = cancel_set().lock() {
        s.insert(request_id);
    }
}

#[tauri::command]
pub async fn tavern_llm_chat(app: AppHandle, request: LlmRequest) -> Result<LlmResult, String> {
    let id = request.request_id.clone();
    let result = run_chat(&app, &request).await;
    clear_cancel(&id);
    result
}

async fn run_chat(app: &AppHandle, request: &LlmRequest) -> Result<LlmResult, String> {
    if request.api_key.trim().is_empty() {
        return Err("请先在酒馆「接口」里填写 API Key".into());
    }
    if request.model.trim().is_empty() {
        return Err("请先填写模型名".into());
    }
    let url = endpoint(&request.base_url, "chat/completions")?;
    let stream = request.stream.unwrap_or(true);
    let mut body = json!({
        "model": request.model.trim(),
        "messages": request.messages,
        "stream": stream,
    });
    if let Some(t) = request.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(p) = request.top_p {
        body["top_p"] = json!(p);
    }
    if let Some(m) = request.max_tokens {
        if m > 0 {
            body["max_tokens"] = json!(m);
        }
    }

    let client = crate::nai::http_client_no_redirect()?;
    let res = client
        .post(&url)
        .bearer_auth(request.api_key.trim())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let msg = crate::nai::format_reqwest(e);
            msg.replace("无法连接 NovelAI", "无法连接对话 API")
        })?;
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(status_error(status, &text));
    }

    if !stream {
        let data: Value = res.json().await.map_err(crate::nai::format_reqwest)?;
        let choice = data.get("choices").and_then(|c| c.get(0)).cloned().unwrap_or(Value::Null);
        let (content, reasoning) = delta_parts(&choice, "message");
        let finish_reason = str_at(&choice, &["finish_reason"]).to_string();
        return Ok(LlmResult { content, reasoning, finish_reason });
    }

    let mut content = String::new();
    let mut reasoning = String::new();
    let mut finish_reason = String::new();
    let mut buf: Vec<u8> = Vec::new();
    let mut bytes = res.bytes_stream();
    'outer: while let Some(chunk) = bytes.next().await {
        if is_cancelled(&request.request_id) {
            finish_reason = "cancelled".into();
            break;
        }
        let chunk = chunk.map_err(crate::nai::format_reqwest)?;
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" {
                break 'outer;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            if let Some(err) = v.get("error") {
                let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("未知错误");
                return Err(format!("对话 API 返回错误：{msg}"));
            }
            let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else { continue };
            let (c, r) = delta_parts(choice, "delta");
            let fr = str_at(choice, &["finish_reason"]);
            if !fr.is_empty() {
                finish_reason = fr.to_string();
            }
            if c.is_empty() && r.is_empty() {
                continue;
            }
            content.push_str(&c);
            reasoning.push_str(&r);
            let _ = app.emit(
                "tavern-llm-delta",
                LlmDelta { request_id: request.request_id.clone(), content: c, reasoning: r },
            );
        }
    }
    Ok(LlmResult { content, reasoning, finish_reason })
}

#[tauri::command]
pub async fn tavern_llm_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    if api_key.trim().is_empty() {
        return Err("请先填写 API Key".into());
    }
    let url = endpoint(&base_url, "models")?;
    let client = crate::nai::http_client()?;
    let res = client
        .get(&url)
        .bearer_auth(api_key.trim())
        .send()
        .await
        .map_err(crate::nai::format_reqwest)?;
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(status_error(status, &text));
    }
    let data: Value = res.json().await.map_err(crate::nai::format_reqwest)?;
    let mut ids: Vec<String> = data
        .get("data")
        .or_else(|| data.get("models"))
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn rejects_bad_names() {
        assert!(safe_name("state").is_ok());
        assert!(safe_name("chat_abc-123").is_ok());
        assert!(safe_name("../x").is_err());
        assert!(safe_name("a/b").is_err());
        assert!(safe_name("").is_err());
    }
}
