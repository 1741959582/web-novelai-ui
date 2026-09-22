use crate::store::data_dir;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseTag {
    pub id: String,
    pub label: String,
    pub confidence: f64,
    pub kind: String,
    pub found: Option<bool>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseRating {
    pub label: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseJob {
    pub id: String,
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub path: Option<String>,
    pub status: String,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub progress_msg: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub tags: Vec<ReverseTag>,
    #[serde(default)]
    pub rating: Vec<ReverseRating>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseTaskSummary {
    pub id: String,
    pub saved_at: String,
    pub name: String,
    pub image_count: u32,
    pub done_count: u32,
    pub thumbnail_path: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseTask {
    pub id: String,
    pub saved_at: String,
    pub name: String,
    pub model: String,
    pub general_thresh: f64,
    pub general_mcut: bool,
    pub character_thresh: f64,
    pub character_mcut: bool,
    pub jobs: Vec<ReverseJob>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseTaskSave {
    pub id: Option<String>,
    pub name: Option<String>,
    pub model: String,
    pub general_thresh: f64,
    pub general_mcut: bool,
    pub character_thresh: f64,
    pub character_mcut: bool,
    pub jobs: Vec<ReverseJob>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    current_id: Option<String>,
    tasks: Vec<ReverseTaskSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredJob {
    id: String,
    name: String,
    file: String,
    #[serde(default)]
    path: Option<String>,
    status: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    tags: Vec<ReverseTag>,
    #[serde(default)]
    rating: Vec<ReverseRating>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTask {
    id: String,
    saved_at: String,
    name: String,
    model: String,
    general_thresh: f64,
    general_mcut: bool,
    character_thresh: f64,
    character_mcut: bool,
    jobs: Vec<StoredJob>,
}

fn root_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join("reverse-tasks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn index_path() -> Result<PathBuf, String> {
    Ok(root_dir()?.join("index.json"))
}

fn task_dir(id: &str) -> Result<PathBuf, String> {
    let dir = root_dir()?.join(id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn load_index() -> IndexFile {
    index_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_index(data: &IndexFile) -> Result<(), String> {
    let path = index_path()?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(data).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

fn decode_data_url(value: &str) -> Result<Vec<u8>, String> {
    let raw = value
        .split(',')
        .next_back()
        .unwrap_or(value)
        .replace(['\n', '\r', ' '], "");
    base64::engine::general_purpose::STANDARD
        .decode(raw.as_bytes())
        .map_err(|e| format!("图片解码失败：{e}"))
}

fn encode_data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(&[0x52, 0x49, 0x46, 0x46]) {
        "image/webp"
    } else {
        "image/png"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn ext_for(image: &str, bytes: &[u8]) -> &'static str {
    if image.contains("image/jpeg") || bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if image.contains("image/webp") || bytes.starts_with(&[0x52, 0x49, 0x46, 0x46]) {
        "webp"
    } else {
        "png"
    }
}

fn persist_status(status: &str) -> String {
    match status {
        "running" => "idle".into(),
        other => other.to_string(),
    }
}

fn write_task(id: &str, payload: &ReverseTaskSave, now: &str) -> Result<(StoredTask, ReverseTaskSummary), String> {
    let dir = task_dir(id)?;
    let name = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("反推 · {} 张", payload.jobs.len()));
    let mut stored_jobs = Vec::new();
    let mut done_count = 0u32;
    let mut thumb = String::new();
    for job in &payload.jobs {
        if job.status == "done" {
            done_count += 1;
        }
        let file_name = if job.image.starts_with("data:image/") {
            let bytes = decode_data_url(&job.image)?;
            let file = format!("{}.{}", job.id, ext_for(&job.image, &bytes));
            fs::write(dir.join(&file), bytes).map_err(|e| format!("写入反推图片失败：{e}"))?;
            file
        } else if !job.image.is_empty() && PathBuf::from(&job.image).exists() {
            PathBuf::from(&job.image)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image.png")
                .to_string()
        } else {
            continue;
        };
        if thumb.is_empty() {
            thumb = dir.join(&file_name).to_string_lossy().to_string();
        }
        stored_jobs.push(StoredJob {
            id: job.id.clone(),
            name: job.name.clone(),
            file: file_name,
            path: job.path.clone(),
            status: persist_status(&job.status),
            error: job.error.clone(),
            tags: job.tags.clone(),
            rating: job.rating.clone(),
        });
    }
    let stored = StoredTask {
        id: id.to_string(),
        saved_at: now.to_string(),
        name,
        model: payload.model.clone(),
        general_thresh: payload.general_thresh,
        general_mcut: payload.general_mcut,
        character_thresh: payload.character_thresh,
        character_mcut: payload.character_mcut,
        jobs: stored_jobs,
    };
    fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写入反推任务失败：{e}"))?;
    let summary = ReverseTaskSummary {
        id: id.to_string(),
        saved_at: now.to_string(),
        name: stored.name.clone(),
        image_count: stored.jobs.len() as u32,
        done_count,
        thumbnail_path: thumb,
        model: stored.model.clone(),
    };
    Ok((stored, summary))
}

fn read_stored(id: &str) -> Result<StoredTask, String> {
    let path = root_dir()?.join(id).join("meta.json");
    let text = fs::read_to_string(path).map_err(|_| "找不到该反推任务".to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("反推任务损坏：{e}"))
}

fn hydrate(stored: StoredTask) -> Result<ReverseTask, String> {
    let dir = root_dir()?.join(&stored.id);
    let mut jobs = Vec::new();
    for job in stored.jobs {
        let bytes = fs::read(dir.join(&job.file)).map_err(|e| format!("读取反推图片失败：{e}"))?;
        jobs.push(ReverseJob {
            id: job.id,
            name: job.name,
            image: encode_data_url(&bytes),
            path: job.path,
            status: job.status,
            progress: if job.tags.is_empty() { 0.0 } else { 1.0 },
            progress_msg: if job.tags.is_empty() { String::new() } else { "反推完成".into() },
            error: job.error,
            tags: job.tags,
            rating: job.rating,
        });
    }
    Ok(ReverseTask {
        id: stored.id,
        saved_at: stored.saved_at,
        name: stored.name,
        model: stored.model,
        general_thresh: stored.general_thresh,
        general_mcut: stored.general_mcut,
        character_thresh: stored.character_thresh,
        character_mcut: stored.character_mcut,
        jobs,
    })
}

#[tauri::command]
pub fn reverse_tasks_list() -> Vec<ReverseTaskSummary> {
    let mut data = load_index();
    data.tasks.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    data.tasks
}

#[tauri::command]
pub fn reverse_task_current() -> Option<String> {
    load_index().current_id
}

#[tauri::command]
pub fn reverse_task_save(payload: ReverseTaskSave) -> Result<ReverseTaskSummary, String> {
    if payload.jobs.is_empty() {
        return Err("当前没有可保存的反推图片。".into());
    }
    let now = chrono::Local::now().to_rfc3339();
    let id = payload
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let (_stored, summary) = write_task(&id, &payload, &now)?;
    let mut data = load_index();
    data.tasks.retain(|item| item.id != id);
    data.tasks.insert(0, summary.clone());
    data.current_id = Some(id);
    save_index(&data)?;
    Ok(summary)
}

#[tauri::command]
pub fn reverse_task_load(id: String) -> Result<ReverseTask, String> {
    let stored = read_stored(&id)?;
    let mut data = load_index();
    data.current_id = Some(id);
    let _ = save_index(&data);
    hydrate(stored)
}

#[tauri::command]
pub fn reverse_task_new() -> Result<(), String> {
    let mut data = load_index();
    data.current_id = None;
    save_index(&data)
}

#[tauri::command]
pub fn reverse_task_delete(id: String) -> Result<(), String> {
    let mut data = load_index();
    data.tasks.retain(|item| item.id != id);
    if data.current_id.as_deref() == Some(id.as_str()) {
        data.current_id = None;
    }
    save_index(&data)?;
    let dir = root_dir()?.join(id);
    let _ = fs::remove_dir_all(dir);
    Ok(())
}
