use crate::nai::http_client;
use crate::store::data_dir;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const SITE: &str = "https://novelai.quicktagcloud.com/";
const DATA_SOURCE: &str = "https://novelai.quicktagcloud.com/data-source.json";
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct Timed<T> {
    at: Instant,
    value: T,
}

struct Cache {
    catalog: Option<Timed<Catalog>>,
    books: HashMap<String, Timed<Value>>,
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(Cache {
            catalog: None,
            books: HashMap::new(),
        })
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCollection {
    pub id: String,
    pub title: String,
    pub author: String,
    pub version: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub entry_count: u32,
    pub imaged_count: u32,
    pub nsfw: bool,
    pub cover: String,
    pub cover_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCategory {
    pub path: Vec<String>,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickChar {
    pub label: String,
    pub prompt: String,
    #[serde(default)]
    pub negative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickImage {
    pub preview_url: String,
    pub original_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickEntry {
    pub id: String,
    pub collection_id: String,
    pub title: String,
    pub path: Vec<String>,
    pub prompt: String,
    pub negative: String,
    pub note: String,
    pub nsfw: bool,
    pub character_prompts: Vec<QuickChar>,
    pub images: Vec<QuickImage>,
    pub cover_url: String,
    pub source_url: String,
    pub model: String,
    pub sampler: String,
    pub steps: Option<u32>,
    pub seed: Option<i64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub cfg_scale: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPage {
    pub release: String,
    pub collection_id: String,
    pub collection_title: String,
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
    pub categories: Vec<QuickCategory>,
    pub items: Vec<QuickEntry>,
}

#[derive(Clone)]
struct Catalog {
    release: String,
    release_base: String,
    media_base: String,
    image_prefix: String,
    original_prefix: String,
    collections: Vec<QuickCollection>,
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn boolish(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_u64() == Some(1),
        Value::String(s) => matches!(s.as_str(), "1" | "true" | "yes"),
        _ => false,
    }
}

fn num_u32(v: &Value) -> u32 {
    v.as_u64().unwrap_or(0) as u32
}

fn list<'a>(v: &'a Value) -> &'a [Value] {
    v.as_array().map(|a| a.as_slice()).unwrap_or(&[])
}

async fn get_json(url: &str) -> Result<Value, String> {
    let client = http_client()?;
    let res = client
        .get(url)
        .header("Referer", SITE)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;
    if !res.status().is_success() {
        return Err(format!("请求失败：{} {}", res.status(), url));
    }
    res.json().await.map_err(|e| format!("解析失败：{e}"))
}

fn asset_url(media_base: &str, prefix: &str, collection: &str, file: &str) -> String {
    let file = file.trim();
    if file.is_empty() {
        return String::new();
    }
    if file.starts_with("https://") {
        return file.to_string();
    }
    let enc = |s: &str| {
        s.split('/')
            .map(|p| {
                let mut out = String::new();
                for b in p.as_bytes() {
                    match *b {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
                        _ => out.push_str(&format!("%{b:02X}")),
                    }
                }
                out
            })
            .collect::<Vec<_>>()
            .join("/")
    };
    format!(
        "{}/{}/{}/{}",
        media_base.trim_end_matches('/'),
        enc(prefix),
        enc(collection),
        enc(file)
    )
}

fn source_url(collection: &str, entry: &str) -> String {
    let mut url = format!("{SITE}?c={collection}");
    if !entry.is_empty() {
        url.push_str("&entry=");
        url.push_str(&url::form_urlencoded::byte_serialize(entry.as_bytes()).collect::<String>());
    }
    url
}

fn normalize_collection(item: &Value, media_base: &str, image_prefix: &str) -> QuickCollection {
    let id = text(&item["id"]);
    let cover = text(&item["cover"]);
    QuickCollection {
        id: id.clone(),
        title: text(&item["title"]),
        author: text(&item["author"]),
        version: text(&item["version"]),
        kind: {
            let t = text(&item["type"]);
            if t.is_empty() { "other".into() } else { t }
        },
        entry_count: num_u32(&item["entryCount"]),
        imaged_count: num_u32(&item["imagedCount"]),
        nsfw: boolish(&item["nsfw"]),
        cover: cover.clone(),
        cover_url: asset_url(media_base, image_prefix, &id, &cover),
    }
}

async fn load_catalog() -> Result<Catalog, String> {
    if let Ok(guard) = cache().lock() {
        if let Some(hit) = &guard.catalog {
            if hit.at.elapsed() < CACHE_TTL {
                return Ok(hit.value.clone());
            }
        }
    }
    let source = get_json(DATA_SOURCE).await?;
    let base = text(&source["baseUrl"]).trim_end_matches('/').to_string();
    let pointer = text(&source["pointer"]);
    if base.is_empty() || pointer.is_empty() {
        return Err("法典数据源无效".into());
    }
    let pointer_json = get_json(&format!("{base}/{pointer}")).await?;
    let release = text(&pointer_json["release"]);
    if release.is_empty() {
        return Err("法典版本无效".into());
    }
    let release_base = format!("{base}/releases/{release}/");
    let media = get_json(&format!("{release_base}media.json")).await?;
    let media_base = {
        let v = text(&media["baseUrl"]);
        if v.is_empty() {
            "https://assets.quicktagcloud.com".into()
        } else {
            v.trim_end_matches('/').to_string()
        }
    };
    let image_prefix = {
        let v = text(&media["imagePrefix"]);
        if v.is_empty() { "images".into() } else { v }
    };
    let original_prefix = {
        let v = text(&media["originalPrefix"]);
        if v.is_empty() { "originals".into() } else { v }
    };
    let raw = get_json(&format!("{release_base}codexes.json")).await?;
    let items = list(&raw)
        .iter()
        .map(|item| normalize_collection(item, &media_base, &image_prefix))
        .filter(|c| !c.id.is_empty())
        .collect::<Vec<_>>();
    let catalog = Catalog {
        release,
        release_base,
        media_base,
        image_prefix,
        original_prefix,
        collections: items,
    };
    if let Ok(mut guard) = cache().lock() {
        guard.catalog = Some(Timed {
            at: Instant::now(),
            value: catalog.clone(),
        });
    }
    Ok(catalog)
}

fn disk_book(release: &str, id: &str) -> Result<std::path::PathBuf, String> {
    let dir = data_dir()?.join("quicktag").join(release);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{id}.json")))
}

async fn load_book(catalog: &Catalog, id: &str) -> Result<Value, String> {
    let key = format!("{}:{id}", catalog.release);
    if let Ok(guard) = cache().lock() {
        if let Some(hit) = guard.books.get(&key) {
            if hit.at.elapsed() < CACHE_TTL {
                return Ok(hit.value.clone());
            }
        }
    }
    if let Ok(path) = disk_book(&catalog.release, id) {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(parsed) = serde_json::from_slice::<Value>(&bytes) {
                if let Ok(mut guard) = cache().lock() {
                    guard.books.insert(
                        key.clone(),
                        Timed {
                            at: Instant::now(),
                            value: parsed.clone(),
                        },
                    );
                }
                return Ok(parsed);
            }
        }
    }
    let url = format!("{}{id}.json", catalog.release_base);
    let client = http_client()?;
    let res = client
        .get(&url)
        .header("Referer", SITE)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("下载法典失败：{e}"))?;
    if !res.status().is_success() {
        return Err(format!("下载法典失败：{}", res.status()));
    }
    let bytes = res.bytes().await.map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_slice(&bytes).map_err(|e| format!("解析法典失败：{e}"))?;
    if let Ok(path) = disk_book(&catalog.release, id) {
        let _ = fs::write(path, &bytes);
    }
    if let Ok(mut guard) = cache().lock() {
        guard.books.insert(
            key,
            Timed {
                at: Instant::now(),
                value: parsed.clone(),
            },
        );
    }
    Ok(parsed)
}

fn match_query(entry: &Value, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    let chars = list(&entry["characterPrompts"])
        .iter()
        .map(|c| format!("{} {}", text(&c["label"]), text(&c["prompt"])))
        .collect::<Vec<_>>()
        .join(" ");
    let hay = format!(
        "{} {} {} {} {} {}",
        text(&entry["title"]),
        text(&entry["tags"]),
        text(&entry["prompt"]),
        text(&entry["note"]),
        text(&entry["negative"]),
        chars
    )
    .to_lowercase();
    q.split_whitespace().all(|token| {
        let exclude = token.starts_with('-');
        let term = token.trim_start_matches('-').trim_matches('"').to_lowercase();
        if term.is_empty() {
            true
        } else if exclude {
            !hay.contains(&term)
        } else {
            hay.contains(&term)
        }
    })
}

fn entry_nsfw(entry: &Value, collection_nsfw: bool) -> bool {
    if collection_nsfw || boolish(&entry["nsfw"]) {
        return true;
    }
    matches!(
        text(&entry["rating"]).to_ascii_lowercase().as_str(),
        "r18" | "r18g" | "nsfw" | "explicit" | "questionable" | "sensitive" | "adult"
    )
}

fn map_entry(catalog: &Catalog, collection_id: &str, collection_nsfw: bool, entry: &Value, index: usize) -> QuickEntry {
    let id = {
        let v = text(&entry["id"]);
        if v.is_empty() {
            format!("{collection_id}_{}", index + 1)
        } else {
            v
        }
    };
    let path = list(&entry["path"])
        .iter()
        .map(text)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let mut images = Vec::new();
    let listed = list(&entry["images"]);
    if listed.is_empty() {
        let image = text(&entry["image"]);
        let original = text(&entry["original"]);
        if !image.is_empty() || !original.is_empty() {
            images.push((image, original));
        }
    } else {
        for item in listed {
            images.push((
                text(&item["path"]).or_empty(&text(&item["image"])),
                text(&item["original"]),
            ));
        }
    }
    let mapped_images = images
        .into_iter()
        .filter_map(|(preview, original)| {
            let preview_url = asset_url(&catalog.media_base, &catalog.image_prefix, collection_id, &preview);
            let original_url = asset_url(
                &catalog.media_base,
                &catalog.original_prefix,
                collection_id,
                if original.is_empty() { &preview } else { &original },
            );
            if preview_url.is_empty() && original_url.is_empty() {
                None
            } else {
                Some(QuickImage {
                    preview_url: if preview_url.is_empty() { original_url.clone() } else { preview_url },
                    original_url: if original_url.is_empty() {
                        asset_url(&catalog.media_base, &catalog.image_prefix, collection_id, &preview)
                    } else {
                        original_url
                    },
                    width: num_u32(&entry["imageWidth"]),
                    height: num_u32(&entry["imageHeight"]),
                })
            }
        })
        .collect::<Vec<_>>();
    let prompt = {
        let tags = text(&entry["tags"]);
        if tags.is_empty() { text(&entry["prompt"]) } else { tags }
    };
    let negative = {
        let n = text(&entry["negative"]);
        if n.is_empty() { text(&entry["negativePrompt"]) } else { n }
    };
    QuickEntry {
        id: id.clone(),
        collection_id: collection_id.to_string(),
        title: text(&entry["title"]),
        path,
        prompt,
        negative,
        note: text(&entry["note"]),
        nsfw: entry_nsfw(entry, collection_nsfw),
        character_prompts: list(&entry["characterPrompts"])
            .iter()
            .filter_map(|c| {
                let prompt = text(&c["prompt"]);
                if prompt.is_empty() {
                    None
                } else {
                    Some(QuickChar {
                        label: text(&c["label"]),
                        prompt,
                        negative: text(&c["negative"]).or_empty(&text(&c["uc"])),
                    })
                }
            })
            .collect(),
        cover_url: mapped_images.first().map(|i| i.preview_url.clone()).unwrap_or_default(),
        images: mapped_images,
        source_url: source_url(collection_id, &id),
        model: text(&entry["model"]),
        sampler: text(&entry["sampler"]),
        steps: entry["steps"].as_u64().map(|n| n as u32),
        seed: entry["seed"].as_i64(),
        width: entry["width"].as_u64().map(|n| n as u32),
        height: entry["height"].as_u64().map(|n| n as u32),
        cfg_scale: entry["cfgScale"].as_f64().or_else(|| entry["scale"].as_f64()),
    }
}

trait OrEmpty {
    fn or_empty(self, other: &str) -> String;
}
impl OrEmpty for String {
    fn or_empty(self, other: &str) -> String {
        if self.is_empty() { other.to_string() } else { self }
    }
}

fn categories_from(entries: &[Value]) -> Vec<QuickCategory> {
    let mut map: HashMap<String, QuickCategory> = HashMap::new();
    for entry in entries {
        let path = list(&entry["path"])
            .iter()
            .map(text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        for i in 1..=path.len() {
            let prefix = path[..i].to_vec();
            let key = prefix.join("\u{1f}");
            map.entry(key)
                .and_modify(|c| c.count += 1)
                .or_insert(QuickCategory { path: prefix, count: 1 });
        }
    }
    let mut out = map.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[tauri::command]
pub async fn quicktag_catalog(safe_only: Option<bool>) -> Result<Value, String> {
    let catalog = load_catalog().await?;
    let safe = safe_only.unwrap_or(true);
    let collections = catalog
        .collections
        .iter()
        .filter(|c| !safe || !c.nsfw)
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "release": catalog.release,
        "collections": collections,
    }))
}

#[tauri::command]
pub async fn quicktag_search(
    collection_id: String,
    query: Option<String>,
    path: Option<Vec<String>>,
    page: Option<u32>,
    page_size: Option<u32>,
    safe_only: Option<bool>,
) -> Result<QuickPage, String> {
    let catalog = load_catalog().await?;
    let id = collection_id.trim();
    if id.is_empty() {
        return Err("缺少法典 id".into());
    }
    let meta = catalog
        .collections
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| "找不到这部法典".to_string())?;
    let safe = safe_only.unwrap_or(true);
    if safe && meta.nsfw {
        return Err("这部法典被全年龄过滤隐藏了，可在页面打开成人内容".into());
    }
    let book = load_book(&catalog, id).await?;
    let q = query.unwrap_or_default();
    let crumbs = path.unwrap_or_default();
    let filtered = list(&book["entries"])
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            if safe && entry_nsfw(entry, meta.nsfw) {
                return false;
            }
            let entry_path = list(&entry["path"]).iter().map(text).collect::<Vec<_>>();
            crumbs.iter().enumerate().all(|(i, part)| entry_path.get(i).map(|s| s == part).unwrap_or(false))
                && match_query(entry, &q)
        })
        .collect::<Vec<_>>();
    let cats = categories_from(
        &filtered.iter().map(|(_, e)| (*e).clone()).collect::<Vec<_>>(),
    );
    let size = page_size.unwrap_or(48).clamp(12, 120);
    let total = filtered.len() as u32;
    let pages = (total.max(1) + size - 1) / size;
    let page = page.unwrap_or(1).clamp(1, pages.max(1));
    let start = ((page - 1) * size) as usize;
    let items = filtered
        .iter()
        .skip(start)
        .take(size as usize)
        .map(|(i, e)| map_entry(&catalog, id, meta.nsfw, e, *i))
        .collect();
    Ok(QuickPage {
        release: catalog.release,
        collection_id: id.to_string(),
        collection_title: {
            let t = text(&book["title"]);
            if t.is_empty() { meta.title } else { t }
        },
        page,
        page_size: size,
        total,
        categories: cats,
        items,
    })
}

#[tauri::command]
pub async fn quicktag_entry(collection_id: String, entry_id: String) -> Result<QuickEntry, String> {
    let catalog = load_catalog().await?;
    let id = collection_id.trim();
    let book = load_book(&catalog, id).await?;
    let meta_nsfw = catalog.collections.iter().any(|c| c.id == id && c.nsfw);
    for (i, entry) in list(&book["entries"]).iter().enumerate() {
        let eid = text(&entry["id"]);
        let fallback = format!("{id}_{}", i + 1);
        if eid == entry_id || fallback == entry_id {
            return Ok(map_entry(&catalog, id, meta_nsfw, entry, i));
        }
    }
    Err("找不到这条法典词条".into())
}
