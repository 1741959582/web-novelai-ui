use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub has_onboarded: bool,
    pub output_dir: String,
    pub api_base_url: String,
    pub image_base_url: String,
    pub allow_custom_endpoint: bool,
    pub allow_custom_endpoint_fallback: bool,
    pub proxy_url: String,
    pub keep_image_metadata: bool,
    pub theme: String,
    #[serde(default)]
    pub token: String,
    #[serde(default = "default_true")]
    pub stream_preview_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            has_onboarded: false,
            output_dir: String::new(),
            api_base_url: "https://image.novelai.net".into(),
            image_base_url: "https://image.novelai.net".into(),
            allow_custom_endpoint: false,
            allow_custom_endpoint_fallback: false,
            proxy_url: String::new(),
            keep_image_metadata: true,
            theme: "dark".into(),
            token: String::new(),
            stream_preview_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub has_token: bool,
    pub tier_name: String,
    pub tier_level: Option<i32>,
    pub anlas_balance: Option<i64>,
    pub expires_at: Option<String>,
    pub has_active_subscription: bool,
}

impl Default for AccountSummary {
    fn default() -> Self {
        Self {
            has_token: false,
            tier_name: "未知".into(),
            tier_level: None,
            anlas_balance: None,
            expires_at: None,
            has_active_subscription: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub model: String,
    pub prompt: String,
    pub negative_prompt: String,
    pub seed: u32,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub sampler: String,
    pub kind: String,
    #[serde(default)]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub saved_at: String,
    pub image_count: u32,
    pub thumbnail_path: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub characters: Value,
    #[serde(default)]
    pub i2i_strength: f64,
    #[serde(default = "default_batch")]
    pub batch_count: u32,
    #[serde(default)]
    pub image_ids: Vec<String>,
    #[serde(default)]
    pub image_paths: Vec<String>,
}

fn default_batch() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpsert {
    pub id: Option<String>,
    pub params: Value,
    pub characters: Value,
    pub i2i_strength: f64,
    pub batch_count: u32,
    pub image_ids: Vec<String>,
    pub image_paths: Vec<String>,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistFile {
    settings: AppSettings,
    account: AccountSummary,
    history: Vec<HistoryItem>,
    #[serde(default)]
    sessions: Vec<SessionRecord>,
    #[serde(default)]
    current_session_id: Option<String>,
}

fn data_dir() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or("无法定位用户数据目录")?;
    let dir = base.join("langbai-novelai-studio");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn persist_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("state.json"))
}

fn load_file() -> PersistFile {
    persist_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_file(data: &PersistFile) -> Result<(), String> {
    let path = persist_path()?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(data).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

pub fn load_settings() -> AppSettings {
    load_file().settings
}

pub fn save_settings(mut next: AppSettings) -> Result<AppSettings, String> {
    let mut data = load_file();
    let incoming = next.token.trim();
    if incoming.is_empty() || incoming.eq_ignore_ascii_case("configured") {
        next.token = data.settings.token.clone();
    }
    data.settings = next.clone();
    save_file(&data)?;
    Ok(public_settings(next))
}

pub fn public_settings(mut settings: AppSettings) -> AppSettings {
    settings.token = if settings.token.is_empty() {
        String::new()
    } else {
        "configured".into()
    };
    settings
}

pub fn get_token() -> String {
    let token = load_file().settings.token.trim().to_string();
    if token.eq_ignore_ascii_case("configured") {
        String::new()
    } else {
        token
    }
}

pub fn set_token(token: String) -> Result<(), String> {
    let mut data = load_file();
    data.settings.token = token;
    data.account.has_token = true;
    save_file(&data)
}

pub fn set_account(account: AccountSummary) -> Result<(), String> {
    let mut data = load_file();
    data.account = account;
    save_file(&data)
}

pub fn get_account() -> AccountSummary {
    let mut acc = load_file().account;
    acc.has_token = !get_token().is_empty();
    acc
}

pub fn default_output_dir() -> PathBuf {
    let settings = load_settings();
    if !settings.output_dir.trim().is_empty() {
        return PathBuf::from(settings.output_dir);
    }
    dirs::picture_dir()
        .unwrap_or_else(|| data_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("Langbai NovelAI")
}

pub fn add_history(item: HistoryItem) -> Result<(), String> {
    let mut data = load_file();
    data.history.insert(0, item);
    if data.history.len() > 500 {
        data.history.truncate(500);
    }
    save_file(&data)
}

pub fn list_history() -> Vec<HistoryItem> {
    load_file().history
}

pub fn delete_history(id: &str) -> Result<(), String> {
    let mut data = load_file();
    if let Some(item) = data.history.iter().find(|h| h.id == id) {
        let _ = fs::remove_file(&item.path);
    }
    data.history.retain(|h| h.id != id);
    save_file(&data)
}

fn unique_path(dir: &Path, name: &str, ext: &str) -> PathBuf {
    let mut dest = dir.join(format!("{name}.{ext}"));
    let mut n = 2;
    while dest.exists() {
        dest = dir.join(format!("{name}_{n}.{ext}"));
        n += 1;
    }
    dest
}

pub fn save_image_bytes(
    bytes: &[u8],
    prefix: &str,
    seed: u32,
    ext: &str,
) -> Result<PathBuf, String> {
    let dir = default_output_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let stem = if prefix.trim().is_empty() {
        format!("{stamp}_seed{seed}")
    } else {
        format!("{}_{stamp}_seed{seed}", sanitize(prefix))
    };
    let dest = unique_path(&dir, &stem, ext);
    fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    Ok(dest)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .chars()
        .take(40)
        .collect()
}

#[tauri::command]
pub fn settings_get() -> AppSettings {
    public_settings(load_settings())
}

#[tauri::command]
pub fn settings_save(settings: AppSettings) -> Result<AppSettings, String> {
    save_settings(settings)
}

#[tauri::command]
pub fn history_list() -> Vec<HistoryItem> {
    list_history()
}

#[tauri::command]
pub fn history_delete(id: String) -> Result<(), String> {
    delete_history(&id)
}

#[tauri::command]
pub fn account_get() -> AccountSummary {
    get_account()
}

#[tauri::command]
pub async fn pick_output_dir(app: AppHandle) -> Result<Option<String>, String> {
    let folder = app.dialog().file().blocking_pick_folder();
    Ok(folder.map(|p| p.into_path().ok().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()).filter(|s| !s.is_empty()))
}

#[tauri::command]
pub fn open_output_dir() -> Result<(), String> {
    let dir = default_output_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
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

#[tauri::command]
pub fn read_image_data_url(path: String) -> Result<String, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else {
        "application/octet-stream"
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    ))
}

fn sort_sessions(sessions: &mut [SessionRecord]) {
    sessions.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
}

#[tauri::command]
pub fn sessions_list() -> Vec<SessionRecord> {
    let mut data = load_file();
    sort_sessions(&mut data.sessions);
    data.sessions
}

#[tauri::command]
pub fn session_upsert(payload: SessionUpsert) -> Result<SessionRecord, String> {
    let mut data = load_file();
    let now = chrono::Local::now().to_rfc3339();
    let incoming_id = payload
        .id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(id) = incoming_id {
        if let Some(existing) = data.sessions.iter_mut().find(|s| s.id == id) {
            existing.saved_at = now;
            existing.params = payload.params;
            existing.characters = payload.characters;
            existing.i2i_strength = payload.i2i_strength;
            existing.batch_count = payload.batch_count.max(1);
            for (idx, image_id) in payload.image_ids.iter().enumerate() {
                if !existing.image_ids.contains(image_id) {
                    existing.image_ids.insert(0, image_id.clone());
                    if let Some(path) = payload.image_paths.get(idx) {
                        existing.image_paths.insert(0, path.clone());
                    }
                }
            }
            existing.image_count = existing.image_ids.len() as u32;
            if existing.thumbnail_path.is_empty() {
                if let Some(path) = payload.thumbnail_path.filter(|s| !s.is_empty()) {
                    existing.thumbnail_path = path;
                } else if let Some(path) = existing.image_paths.first() {
                    existing.thumbnail_path = path.clone();
                }
            }
            data.current_session_id = Some(id);
            let cloned = existing.clone();
            save_file(&data)?;
            return Ok(cloned);
        }
    }

    let id = Uuid::new_v4().to_string();
    let thumbnail = payload
        .thumbnail_path
        .filter(|s| !s.is_empty())
        .or_else(|| payload.image_paths.first().cloned())
        .unwrap_or_default();
    let rec = SessionRecord {
        id: id.clone(),
        saved_at: now,
        image_count: payload.image_ids.len() as u32,
        thumbnail_path: thumbnail,
        params: payload.params,
        characters: payload.characters,
        i2i_strength: payload.i2i_strength,
        batch_count: payload.batch_count.max(1),
        image_ids: payload.image_ids,
        image_paths: payload.image_paths,
    };
    data.sessions.insert(0, rec.clone());
    data.current_session_id = Some(id);
    save_file(&data)?;
    Ok(rec)
}

#[tauri::command]
pub fn session_load(id: String) -> Result<SessionRecord, String> {
    let mut data = load_file();
    let rec = data
        .sessions
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| "找不到该会话".to_string())?;
    data.current_session_id = Some(id);
    save_file(&data)?;
    Ok(rec)
}

#[tauri::command]
pub fn session_delete(id: String) -> Result<(), String> {
    let mut data = load_file();
    data.sessions.retain(|s| s.id != id);
    if data.current_session_id.as_deref() == Some(id.as_str()) {
        data.current_session_id = None;
    }
    save_file(&data)
}

#[tauri::command]
pub fn session_new() -> Result<String, String> {
    let mut data = load_file();
    data.current_session_id = None;
    save_file(&data)?;
    Ok(String::new())
}
