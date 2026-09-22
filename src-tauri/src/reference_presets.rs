use crate::nai::http_client;
use crate::store::data_dir;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use uuid::Uuid;

const CATALOG_URLS: &[&str] = &[
    "https://2786886095.github.io/novelai-image-desktop/reference-catalog/index.json",
    "https://raw.githubusercontent.com/2786886095/novelai-reference-assets/main/catalog/index.json",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VibeSnap {
    pub preview_url: String,
    pub base64: String,
    pub info_extracted: f64,
    pub strength: f64,
    pub enabled: bool,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreciseSnap {
    pub preview_url: String,
    pub base64: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub strength: f64,
    pub fidelity: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePreset {
    pub id: String,
    pub name: String,
    pub group: String,
    pub created_at: String,
    #[serde(default)]
    pub vibe_images: Vec<VibeSnap>,
    #[serde(default)]
    pub precise_references: Vec<PreciseSnap>,
    #[serde(default)]
    pub normalize_vibe: bool,
    #[serde(default)]
    pub source_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Library {
    #[serde(default)]
    presets: Vec<ReferencePreset>,
}

fn library_path() -> Result<std::path::PathBuf, String> {
    Ok(data_dir()?.join("reference-presets.json"))
}

fn load() -> Result<Library, String> {
    let path = library_path()?;
    if !path.exists() {
        return Ok(Library::default());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("读取参考预设失败：{e}"))
}

fn save(lib: &Library) -> Result<(), String> {
    let path = library_path()?;
    fs::write(path, serde_json::to_vec_pretty(lib).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reference_preset_list() -> Result<Vec<ReferencePreset>, String> {
    Ok(load()?.presets)
}

#[tauri::command]
pub fn reference_preset_save(preset: ReferencePreset) -> Result<ReferencePreset, String> {
    let mut lib = load()?;
    let mut next = preset;
    if next.name.trim().is_empty() {
        return Err("请填写预设名称".into());
    }
    if next.vibe_images.is_empty() && next.precise_references.is_empty() {
        return Err("当前没有可保存的氛围或精准参考".into());
    }
    if next.id.trim().is_empty() {
        next.id = Uuid::new_v4().to_string();
        next.created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        lib.presets.insert(0, next.clone());
    } else if let Some(old) = lib.presets.iter_mut().find(|p| p.id == next.id) {
        next.created_at = old.created_at.clone();
        *old = next.clone();
    } else {
        lib.presets.insert(0, next.clone());
    }
    save(&lib)?;
    Ok(next)
}

#[tauri::command]
pub fn reference_preset_delete(id: String) -> Result<(), String> {
    let mut lib = load()?;
    lib.presets.retain(|p| p.id != id);
    save(&lib)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGame {
    pub id: String,
    pub name: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAsset {
    pub id: String,
    pub game: String,
    pub category: String,
    pub name: String,
    pub search: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub thumbnail_url: String,
    pub download_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogManifest {
    pub generated_at: String,
    pub provider: String,
    pub games: Vec<CatalogGame>,
    pub assets: Vec<CatalogAsset>,
}

fn usable_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")) && !lower.contains("gitee.com")
}

fn pick_urls(primary: &str, mirrors: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let github = mirrors.get("github").and_then(|v| v.as_str()).unwrap_or("");
    for item in [github, primary] {
        if usable_url(item) && !out.iter().any(|x| x == item) {
            out.push(item.to_string());
        }
    }
    out
}

fn catalog_cache_path() -> Result<std::path::PathBuf, String> {
    Ok(data_dir()?.join("reference-catalog.json"))
}

fn local_catalog_candidates() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        for rel in [
            "public/reference-catalog/index.json",
            "../novelai-image-desktop-main/public/reference-catalog/index.json",
            "../../novelai-image-desktop-main/public/reference-catalog/index.json",
            "novelai-image-desktop-main/public/reference-catalog/index.json",
        ] {
            out.push(cwd.join(rel));
        }
        let mut dir = cwd;
        for _ in 0..5 {
            out.push(
                dir.join("novelai-image-desktop-main")
                    .join("public")
                    .join("reference-catalog")
                    .join("index.json"),
            );
            if !dir.pop() {
                break;
            }
        }
    }
    out.push(std::path::PathBuf::from(
        r"D:\project\novelai-image-desktop-main\public\reference-catalog\index.json",
    ));
    out
}

fn slim_catalog(raw: &Value) -> Result<CatalogManifest, String> {
    if raw.get("schema").and_then(|v| v.as_str()) != Some("langbai-reference-catalog/v1") {
        return Err("在线参考目录格式无效".into());
    }
    let games = raw
        .get("games")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|g| {
            let id = g.get("id")?.as_str()?.to_string();
            let name = g
                .pointer("/names/zh-CN")
                .and_then(|v| v.as_str())
                .unwrap_or(&id)
                .to_string();
            let categories = g
                .get("categories")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| c.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Some(CatalogGame { id, name, categories })
        })
        .collect::<Vec<_>>();
    let assets = raw
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or("在线参考目录没有资源")?
        .iter()
        .filter_map(|a| {
            let id = a.get("id")?.as_str()?.to_string();
            let game = a.get("game")?.as_str()?.to_string();
            let category = a.get("category").and_then(|v| v.as_str()).unwrap_or("角色资源").to_string();
            let name = a
                .pointer("/names/zh-CN")
                .and_then(|v| v.as_str())
                .or_else(|| a.get("roleId").and_then(|v| v.as_str()))
                .unwrap_or(&id)
                .to_string();
            let mut search = vec![
                game.clone(),
                category.clone(),
                name.clone(),
                a.get("roleId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            ];
            if let Some(aliases) = a.get("searchAliases").and_then(|v| v.as_array()) {
                search.extend(aliases.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
            }
            if let Some(names) = a.get("names").and_then(|v| v.as_object()) {
                search.extend(names.values().filter_map(|v| v.as_str().map(|s| s.to_string())));
            }
            let thumbs = pick_urls(
                a.get("thumbnailUrl").and_then(|v| v.as_str()).unwrap_or(""),
                a.get("thumbnailMirrors").unwrap_or(&Value::Null),
            );
            let downloads = pick_urls(
                a.get("downloadUrl").and_then(|v| v.as_str()).unwrap_or(""),
                a.get("downloadMirrors").unwrap_or(&Value::Null),
            );
            Some(CatalogAsset {
                id,
                game,
                category,
                name,
                search: search.join(" ").to_lowercase(),
                width: a.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                height: a.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                bytes: a.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0),
                thumbnail_url: thumbs.first().cloned().unwrap_or_default(),
                download_urls: if downloads.is_empty() { thumbs } else { downloads },
            })
        })
        .collect::<Vec<_>>();
    if assets.is_empty() {
        return Err("在线参考目录是空的".into());
    }
    Ok(CatalogManifest {
        generated_at: raw.get("generatedAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        provider: raw.get("provider").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        games,
        assets,
    })
}

#[tauri::command]
pub async fn reference_catalog_load(refresh: Option<bool>) -> Result<CatalogManifest, String> {
    let force = refresh.unwrap_or(false);
    let cache = catalog_cache_path()?;
    if !force {
        if let Ok(bytes) = fs::read(&cache) {
            if let Ok(parsed) = serde_json::from_slice::<CatalogManifest>(&bytes) {
                if !parsed.assets.is_empty() {
                    return Ok(parsed);
                }
            }
        }
    }
    let client = http_client()?;
    let mut last = "在线参考目录不可用".to_string();
    for url in CATALOG_URLS {
        match client.get(*url).send().await {
            Ok(res) if res.status().is_success() => {
                let raw: Value = res.json().await.map_err(|e| format!("解析目录失败：{e}"))?;
                let slim = slim_catalog(&raw)?;
                let _ = fs::write(&cache, serde_json::to_vec(&slim).unwrap_or_default());
                return Ok(slim);
            }
            Ok(res) => last = format!("读取目录失败：{}", res.status()),
            Err(e) => last = format!("读取目录失败：{e}"),
        }
    }
    for path in local_catalog_candidates() {
        if !path.exists() {
            continue;
        }
        match fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string()))
            .and_then(|raw| slim_catalog(&raw))
        {
            Ok(slim) => {
                let _ = fs::write(&cache, serde_json::to_vec(&slim).unwrap_or_default());
                return Ok(slim);
            }
            Err(e) => last = format!("读取本地目录失败：{e}"),
        }
    }
    Err(last)
}

#[tauri::command]
pub async fn reference_catalog_download(urls: Vec<String>) -> Result<String, String> {
    let client = http_client()?;
    let mut last = "参考图下载失败".to_string();
    for url in urls.into_iter().filter(|u| usable_url(u)) {
        match client.get(&url).send().await {
            Ok(res) if res.status().is_success() => {
                let bytes = res.bytes().await.map_err(|e| e.to_string())?;
                if bytes.len() < 32 {
                    last = "参考图内容为空".into();
                    continue;
                }
                return Ok(base64::engine::general_purpose::STANDARD.encode(bytes));
            }
            Ok(res) => last = format!("下载失败：{}", res.status()),
            Err(e) => last = format!("下载失败：{e}"),
        }
    }
    Err(last)
}
