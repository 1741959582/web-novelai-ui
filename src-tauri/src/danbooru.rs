use crate::nai::{format_reqwest, http_client};
use crate::store::{data_dir, get_token, load_settings};
use encoding_rs::GBK;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const PINNED_COMMIT: &str = "2975a0aae0a375abf9d3f7abadc19276633a8e42";
const CN_CSV_URL: &str = "https://raw.githubusercontent.com/SuzumiyaAkizuki/DanbooruSearchOnline/2975a0aae0a375abf9d3f7abadc19276633a8e42/origin_database/tags_enhanced.csv";
const MAX_DOWNLOAD_BYTES: usize = 20 * 1024 * 1024;
const MIN_RECORDS: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DanbooruTag {
    name: String,
    cn: Vec<String>,
    #[serde(default)]
    post: u32,
    #[serde(default)]
    category: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSuggestion {
    pub tag: String,
    pub count: u32,
    pub category: u32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DanbooruStatus {
    pub downloaded: bool,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagLookup {
    pub tag: String,
    pub found: bool,
    pub count: u32,
    pub category: u32,
    pub description: String,
}

static INDEX: OnceLock<Mutex<Option<Vec<DanbooruTag>>>> = OnceLock::new();

fn csv_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("danbooru-cn.csv"))
}

fn custom_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("custom-tags.json"))
}

fn norm_tag(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', " ")
}

fn load_custom_tags() -> Vec<DanbooruTag> {
    let Ok(path) = custom_path() else {
        return Vec::new();
    };
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    serde_json::from_slice::<Vec<DanbooruTag>>(&bytes).unwrap_or_default()
}

fn save_custom_tags(tags: &[DanbooruTag]) -> Result<(), String> {
    let path = custom_path()?;
    let bytes = serde_json::to_vec_pretty(tags).map_err(|e| format!("写入自定义词条失败：{e}"))?;
    fs::write(path, bytes).map_err(|e| format!("写入自定义词条失败：{e}"))
}

fn merge_custom_into(idx: &mut Vec<DanbooruTag>) {
    for tag in load_custom_tags() {
        if let Some(existing) = idx.iter_mut().find(|item| norm_tag(&item.name) == norm_tag(&tag.name)) {
            for cn in tag.cn {
                if !cn.is_empty() && !existing.cn.iter().any(|item| item == &cn) {
                    existing.cn.push(cn);
                }
            }
            if tag.category != 0 {
                existing.category = tag.category;
            }
        } else {
            idx.push(tag);
        }
    }
}

fn to_lookup(requested: &str, tag: Option<&DanbooruTag>) -> TagLookup {
    if let Some(tag) = tag {
        TagLookup {
            tag: requested.to_string(),
            found: true,
            count: tag.post,
            category: tag.category,
            description: tag.cn.join(" "),
        }
    } else {
        TagLookup {
            tag: requested.to_string(),
            found: false,
            count: 0,
            category: 0,
            description: String::new(),
        }
    }
}

fn index_lock() -> &'static Mutex<Option<Vec<DanbooruTag>>> {
    INDEX.get_or_init(|| Mutex::new(None))
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    cur.push('"');
                    i += 1;
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    out.push(cur);
    out
}

fn parse_index(bytes: &[u8]) -> Result<Vec<DanbooruTag>, String> {
    let (text, _, _) = GBK.decode(bytes);
    let mut lines = text.lines();
    let header = lines.next().unwrap_or("").to_ascii_lowercase();
    if !header.contains("name") || !header.contains("cn_name") {
        return Err("表头不符（缺少 name/cn_name 列）".into());
    }
    let mut out = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let cols = parse_csv_line(line);
        let name = cols.first().map(|s| s.trim().to_string()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let cn: Vec<String> = cols
            .get(1)
            .map(|s| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
            .unwrap_or_default();
        if cn.is_empty() {
            continue;
        }
        let post = cols.get(3).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let category = cols.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        out.push(DanbooruTag { name, cn, post, category });
    }
    out.sort_by(|a, b| b.post.cmp(&a.post));
    Ok(out)
}

fn ensure_index() -> Result<(), String> {
    {
        let guard = index_lock().lock().map_err(|e| e.to_string())?;
        if guard.as_ref().is_some() {
            return Ok(());
        }
    }
    let mut parsed = fs::read(csv_path()?)
        .ok()
        .and_then(|bytes| parse_index(&bytes).ok())
        .unwrap_or_default();
    merge_custom_into(&mut parsed);
    let mut guard = index_lock().lock().map_err(|e| e.to_string())?;
    *guard = Some(parsed);
    Ok(())
}

fn search_index(idx: &[DanbooruTag], query: &str, limit: usize) -> Vec<TagSuggestion> {
    let raw = query.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let is_cjk = raw.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let q = raw.to_ascii_lowercase();
    let normalized = q.replace('_', " ");
    let mut scored: Vec<(&DanbooruTag, i32)> = Vec::new();
    for tag in idx {
        let mut score = 0;
        if is_cjk {
            for cn in &tag.cn {
                if cn == raw {
                    score = 3;
                    break;
                }
                if cn.starts_with(raw) {
                    score = score.max(2);
                } else if cn.contains(raw) {
                    score = score.max(1);
                }
            }
        } else {
            let name = tag.name.to_ascii_lowercase().replace('_', " ");
            if name == normalized {
                score = 3;
            } else if name.starts_with(&normalized) {
                score = 2;
            } else if name.contains(&normalized) {
                score = 1;
            }
        }
        if score > 0 {
            scored.push((tag, score));
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.post.cmp(&a.0.post)));
    scored
        .into_iter()
        .take(limit)
        .map(|(t, _)| TagSuggestion {
            tag: t.name.replace('_', " "),
            count: t.post,
            category: t.category,
            description: t.cn.join(" "),
        })
        .collect()
}

#[tauri::command]
pub fn danbooru_status() -> DanbooruStatus {
    if ensure_index().is_ok() {
        if let Ok(guard) = index_lock().lock() {
            if let Some(idx) = guard.as_ref() {
                if idx.len() >= MIN_RECORDS {
                    return DanbooruStatus {
                        downloaded: true,
                        count: idx.len() as u32,
                    };
                }
            }
        }
    }
    DanbooruStatus {
        downloaded: false,
        count: 0,
    }
}

#[tauri::command]
pub async fn download_danbooru() -> Result<DanbooruStatus, String> {
    let _ = PINNED_COMMIT;
    let client = http_client()?;
    let res = client
        .get(CN_CSV_URL)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(format_reqwest)?;
    if !res.status().is_success() {
        return Err(format!("下载失败 HTTP {}", res.status()));
    }
    let bytes = res.bytes().await.map_err(format_reqwest)?;
    if bytes.len() > MAX_DOWNLOAD_BYTES {
        return Err("下载内容超过 20MB 上限，已放弃。".into());
    }
    let mut parsed = parse_index(&bytes)?;
    if parsed.len() < MIN_RECORDS {
        return Err(format!(
            "数据校验失败：仅解析出 {} 条（< {}），未覆盖现有文件。",
            parsed.len(),
            MIN_RECORDS
        ));
    }
    let path = csv_path()?;
    let tmp = path.with_extension("csv.tmp");
    fs::write(&tmp, &bytes).map_err(|e| format!("写入失败：{e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("写入失败：{e}"))?;
    merge_custom_into(&mut parsed);
    let count = parsed.len() as u32;
    let mut guard = index_lock().lock().map_err(|e| e.to_string())?;
    *guard = Some(parsed);
    Ok(DanbooruStatus {
        downloaded: true,
        count,
    })
}

#[derive(Debug, Deserialize)]
struct OfficialSuggest {
    tags: Option<Vec<OfficialTag>>,
}

#[derive(Debug, Deserialize)]
struct OfficialTag {
    tag: Option<String>,
    count: Option<u32>,
    category: Option<u32>,
}

async fn official_suggest(query: &str, limit: usize) -> Vec<TagSuggestion> {
    let token = get_token();
    if token.is_empty() {
        return Vec::new();
    }
    let settings = load_settings();
    let base = if settings.allow_custom_endpoint && !settings.api_base_url.trim().is_empty() {
        settings.api_base_url.trim().trim_end_matches('/').to_string()
    } else {
        "https://api.novelai.net".into()
    };
    let client = match http_client() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let res = client
        .get(format!("{base}/ai/generate-image/suggest-tags"))
        .query(&[("model", "nai-diffusion-4-5-full"), ("prompt", query)])
        .header("Authorization", format!("Bearer {token}"))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;
    let Ok(res) = res else {
        return Vec::new();
    };
    let Ok(body) = res.json::<OfficialSuggest>().await else {
        return Vec::new();
    };
    body.tags
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| {
            let tag = t.tag?.replace('_', " ");
            if tag.is_empty() {
                return None;
            }
            Some(TagSuggestion {
                tag,
                count: t.count.unwrap_or(0),
                category: t.category.unwrap_or(0),
                description: String::new(),
            })
        })
        .take(limit)
        .collect()
}

#[tauri::command]
pub async fn suggest_tags(query: String, limit: Option<u32>) -> Result<Vec<TagSuggestion>, String> {
    let limit = limit.unwrap_or(8).clamp(1, 20) as usize;
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    if ensure_index().is_ok() {
        if let Ok(guard) = index_lock().lock() {
            if let Some(idx) = guard.as_ref() {
                let hits = search_index(idx, &q, limit);
                if !hits.is_empty() {
                    return Ok(hits);
                }
            }
        }
    }
    Ok(official_suggest(&q, limit).await)
}

#[tauri::command]
pub fn lookup_tags(names: Vec<String>) -> Result<Vec<TagLookup>, String> {
    let _ = ensure_index();
    let guard = index_lock().lock().map_err(|e| e.to_string())?;
    let map: HashMap<String, &DanbooruTag> = guard
        .as_ref()
        .map(|items| items.iter().map(|item| (norm_tag(&item.name), item)).collect())
        .unwrap_or_default();
    Ok(names
        .into_iter()
        .map(|name| {
            let key = norm_tag(&name);
            if key.is_empty() {
                return to_lookup(&name, None);
            }
            to_lookup(&name, map.get(&key).copied())
        })
        .collect())
}

#[tauri::command]
pub fn add_custom_tag(name: String, cn: String, category: Option<u32>) -> Result<TagLookup, String> {
    let name = name.trim().replace(' ', "_");
    if name.is_empty() {
        return Err("标签名为空".into());
    }
    let cn: Vec<String> = cn
        .split([',', '，', '/', ';', '；'])
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    if cn.is_empty() {
        return Err("请填写中文翻译".into());
    }
    let tag = DanbooruTag {
        name: name.clone(),
        cn,
        post: 1,
        category: category.unwrap_or(0),
    };
    let mut custom = load_custom_tags();
    if let Some(existing) = custom.iter_mut().find(|item| norm_tag(&item.name) == norm_tag(&tag.name)) {
        *existing = tag.clone();
    } else {
        custom.push(tag.clone());
    }
    save_custom_tags(&custom)?;
    let _ = ensure_index();
    let mut guard = index_lock().lock().map_err(|e| e.to_string())?;
    if let Some(idx) = guard.as_mut() {
        if let Some(existing) = idx.iter_mut().find(|item| norm_tag(&item.name) == norm_tag(&tag.name)) {
            existing.cn = tag.cn.clone();
            existing.category = tag.category;
            if existing.post == 0 {
                existing.post = tag.post;
            }
        } else {
            idx.insert(0, tag.clone());
        }
    } else {
        *guard = Some(vec![tag.clone()]);
    }
    Ok(to_lookup(&name.replace('_', " "), Some(&tag)))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticQuery {
    pub query: String,
    pub search_mode: Option<String>,
    pub category: Option<String>,
    pub show_nsfw: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTag {
    pub tag: String,
    pub cn_name: String,
    pub category: String,
    pub count: u64,
    pub wiki: String,
}

fn semantic_categories(category: &str) -> Vec<&'static str> {
    match category {
        "general" => vec!["General"],
        "character" => vec!["Character"],
        "copyright" => vec!["Copyright"],
        _ => vec!["General", "Character", "Copyright"],
    }
}

#[tauri::command]
pub async fn danbooru_semantic_search(query: SemanticQuery) -> Result<Vec<SemanticTag>, String> {
    let text = query.query.trim();
    if text.is_empty() {
        return Err("先写要找的描述".into());
    }
    let mode = query.search_mode.unwrap_or_else(|| "full_scene".into());
    let category = query.category.unwrap_or_else(|| "all".into());
    let client = http_client()?;
    let res = client
        .post("https://sakizuki-danboorusearch.hf.space/api/search")
        .header("User-Agent", "Langbai-NovelAI-Studio/1")
        .json(&serde_json::json!({
            "query": text,
            "top_k": 5,
            "limit": 24,
            "popularity_weight": 0.15,
            "show_nsfw": query.show_nsfw.unwrap_or(false),
            "use_segmentation": true,
            "search_mode": mode,
            "target_categories": semantic_categories(&category),
        }))
        .send()
        .await
        .map_err(format_reqwest)?;
    let status = res.status();
    if !status.is_success() {
        if status.as_u16() == 502 || status.as_u16() == 503 {
            return Err("标签搜索正在启动，等十几秒再试".into());
        }
        return Err(format!("标签搜索失败 HTTP {status}"));
    }
    let body: serde_json::Value = res.json().await.map_err(format_reqwest)?;
    let Some(rows) = body.get("results").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for row in rows {
        let tag = row.get("tag").and_then(|value| value.as_str()).unwrap_or("").trim();
        if tag.is_empty() {
            continue;
        }
        out.push(SemanticTag {
            tag: tag.to_string(),
            cn_name: row.get("cn_name").and_then(|value| value.as_str()).unwrap_or("").to_string(),
            category: row.get("category").and_then(|value| value.as_str()).unwrap_or("").to_string(),
            count: row.get("count").and_then(|value| value.as_u64()).unwrap_or(0),
            wiki: row.get("wiki").and_then(|value| value.as_str()).unwrap_or("").to_string(),
        });
    }
    Ok(out)
}
