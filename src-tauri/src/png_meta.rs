use serde::Serialize;
use serde_json::Value;
use std::fs;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharCaptionMeta {
    pub prompt: String,
    pub negative_prompt: String,
    pub use_coords: bool,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataReport {
    pub kind: String,
    pub software: String,
    pub prompt: String,
    pub negative: String,
    pub model: String,
    pub seed: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub steps: Option<u32>,
    pub sampler: String,
    pub cfg_scale: Option<f64>,
    pub cfg_rescale: Option<f64>,
    pub character_captions: Vec<CharCaptionMeta>,
    pub has_metadata: bool,
    pub raw_text: String,
    pub entries: Vec<(String, String)>,
}

fn empty_report() -> MetadataReport {
    MetadataReport {
        kind: "unknown".into(),
        software: String::new(),
        prompt: String::new(),
        negative: String::new(),
        model: String::new(),
        seed: None,
        width: None,
        height: None,
        steps: None,
        sampler: String::new(),
        cfg_scale: None,
        cfg_rescale: None,
        character_captions: Vec::new(),
        has_metadata: false,
        raw_text: String::new(),
        entries: Vec::new(),
    }
}

fn read_png_texts(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("不是 PNG 文件".into());
    }
    let mut i = 8usize;
    let mut out = Vec::new();
    while i + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let typ = &bytes[i + 4..i + 8];
        let start = i + 8;
        let end = start.saturating_add(len);
        if end + 4 > bytes.len() {
            break;
        }
        let data = &bytes[start..end];
        if typ == b"tEXt" {
            if let Some(z) = data.iter().position(|&b| b == 0) {
                let key = String::from_utf8_lossy(&data[..z]).to_string();
                let val = String::from_utf8_lossy(&data[z + 1..]).to_string();
                out.push((key, val));
            }
        } else if typ == b"iTXt" {
            // keyword\0 compression\0 lang\0 translated\0 text
            if let Some(z) = data.iter().position(|&b| b == 0) {
                let key = String::from_utf8_lossy(&data[..z]).to_string();
                let rest = &data[z + 1..];
                if rest.len() >= 2 {
                    let compressed = rest[0] != 0;
                    // skip compression method, lang, translated
                    let mut p = 2usize;
                    for _ in 0..2 {
                        if let Some(n) = rest[p..].iter().position(|&b| b == 0) {
                            p += n + 1;
                        }
                    }
                    if !compressed && p < rest.len() {
                        out.push((key, String::from_utf8_lossy(&rest[p..]).to_string()));
                    }
                }
            }
        } else if typ == b"IEND" {
            break;
        }
        i = end + 4;
    }
    Ok(out)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffffffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb88320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn text_chunk(key: &str, val: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(key.len() + val.len() + 1);
    payload.extend_from_slice(key.as_bytes());
    payload.push(0);
    payload.extend_from_slice(val.as_bytes());
    let mut crc_src = Vec::with_capacity(4 + payload.len());
    crc_src.extend_from_slice(b"tEXt");
    crc_src.extend_from_slice(&payload);
    let mut chunk = Vec::with_capacity(12 + payload.len());
    chunk.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&payload);
    chunk.extend_from_slice(&crc32(&crc_src).to_be_bytes());
    chunk
}

pub fn inject_text_chunks(png: &[u8], texts: &[(&str, String)]) -> Vec<u8> {
    if png.len() < 33 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return png.to_vec();
    }
    let ihdr_len = u32::from_be_bytes(png[8..12].try_into().unwrap_or([0; 4])) as usize;
    let insert_at = 8 + 12 + ihdr_len;
    if insert_at > png.len() {
        return png.to_vec();
    }
    let extra: Vec<u8> = texts.iter().flat_map(|(k, v)| text_chunk(k, v)).collect();
    let mut out = Vec::with_capacity(png.len() + extra.len());
    out.extend_from_slice(&png[..insert_at]);
    out.extend_from_slice(&extra);
    out.extend_from_slice(&png[insert_at..]);
    out
}

pub fn model_to_nai(value: &str) -> String {
    if value.starts_with("nai-diffusion-") {
        return value.to_string();
    }
    let name = value.to_ascii_lowercase();
    if name.contains("furry") && name.contains("v3") {
        return "nai-diffusion-furry-3".into();
    }
    if name.contains("v5") {
        return if name.contains("curated") {
            "nai-diffusion-5-curated".into()
        } else {
            "nai-diffusion-5-full".into()
        };
    }
    if name.contains("v4.5") || name.contains("v4 5") {
        return if name.contains("curated") {
            "nai-diffusion-4-5-curated".into()
        } else {
            "nai-diffusion-4-5-full".into()
        };
    }
    if name.contains("v4") {
        return if name.contains("curated") {
            "nai-diffusion-4-curated".into()
        } else {
            "nai-diffusion-4-full".into()
        };
    }
    if name.contains("v3") {
        return "nai-diffusion-3".into();
    }
    value.to_string()
}

fn pick<'a>(map: &'a [(String, String)], keys: &[&str]) -> String {
    for k in keys {
        if let Some((_, v)) = map.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)) {
            return v.clone();
        }
    }
    String::new()
}

#[tauri::command]
pub async fn inspect_image(path: String) -> Result<MetadataReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        inspect_bytes(&bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn inspect_image_bytes(base64_data: String) -> Result<MetadataReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let raw = base64_data
            .split(',')
            .next_back()
            .unwrap_or(&base64_data)
            .to_string();
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw)
            .map_err(|e| e.to_string())?;
        inspect_bytes(&bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) fn inspect_bytes(bytes: &[u8]) -> Result<MetadataReport, String> {
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Ok(empty_report());
    }
    let texts = read_png_texts(bytes)?;
    let comment = pick(&texts, &["Comment", "comment"]);
    let description = pick(&texts, &["Description", "description"]);
    let software = pick(&texts, &["Software", "software"]);
    let mut prompt = description.clone();
    let mut negative = String::new();
    let mut seed = None;
    let mut width = None;
    let mut height = None;
    let mut steps = None;
    let mut sampler = String::new();
    let mut cfg_scale = None;
    let mut cfg_rescale = None;
    let mut comment_model = String::new();
    let mut character_captions = Vec::new();
    let mut kind = "unknown";
    if !comment.is_empty() {
        if let Ok(v) = serde_json::from_str::<Value>(&comment) {
            kind = "novelai";
            if prompt.is_empty() {
                prompt = v
                    .pointer("/v4_prompt/caption/base_caption")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("prompt").and_then(|x| x.as_str()))
                    .unwrap_or("")
                    .to_string();
            }
            negative = v
                .pointer("/v4_negative_prompt/caption/base_caption")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("uc").and_then(|x| x.as_str()))
                .or_else(|| v.get("negative_prompt").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            seed = v.get("seed").and_then(|x| x.as_u64()).map(|n| n as u32);
            steps = v.get("steps").and_then(|x| x.as_u64()).map(|n| n as u32);
            sampler = v
                .get("sampler")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            width = v.get("width").and_then(|x| x.as_u64()).map(|n| n as u32);
            height = v.get("height").and_then(|x| x.as_u64()).map(|n| n as u32);
            cfg_scale = v
                .get("scale")
                .or_else(|| v.get("cfg_scale"))
                .and_then(|x| x.as_f64());
            cfg_rescale = v.get("cfg_rescale").and_then(|x| x.as_f64());
            if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                if !m.is_empty() {
                    comment_model = m.to_string();
                }
            }
            let use_coords = v
                .pointer("/v4_prompt/use_coords")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let positives = v
                .pointer("/v4_prompt/caption/char_captions")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            let negatives = v
                .pointer("/v4_negative_prompt/caption/char_captions")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            for (i, raw) in positives.iter().enumerate() {
                let cap = raw
                    .get("char_caption")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if cap.trim().is_empty() {
                    continue;
                }
                let neg = negatives
                    .get(i)
                    .and_then(|x| x.get("char_caption"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let center = raw
                    .get("centers")
                    .and_then(|x| x.as_array())
                    .and_then(|a| a.first());
                character_captions.push(CharCaptionMeta {
                    prompt: cap,
                    negative_prompt: neg,
                    use_coords,
                    x: center
                        .and_then(|c| c.get("x"))
                        .and_then(|x| x.as_f64())
                        .unwrap_or(0.5),
                    y: center
                        .and_then(|c| c.get("y"))
                        .and_then(|x| x.as_f64())
                        .unwrap_or(0.5),
                });
            }
        }
    }
    let source = pick(&texts, &["Source", "source"]);
    let model = if !comment_model.is_empty() {
        model_to_nai(&comment_model)
    } else {
        model_to_nai(&source)
    };
    if software.to_ascii_lowercase().contains("novelai") {
        kind = "novelai";
    }
    if prompt.is_empty() {
        let params = pick(&texts, &["parameters"]);
        if !params.is_empty() {
            kind = "stable-diffusion";
            prompt = params.lines().next().unwrap_or("").to_string();
        }
    }
    let has_metadata = !prompt.is_empty()
        || !negative.is_empty()
        || seed.is_some()
        || !character_captions.is_empty()
        || steps.is_some()
        || software.to_ascii_lowercase().contains("novelai")
        || model.to_ascii_lowercase().contains("novelai")
        || !description.is_empty()
        || !comment.is_empty();
    Ok(MetadataReport {
        kind: kind.into(),
        software,
        prompt,
        negative,
        model,
        seed,
        width,
        height,
        steps,
        sampler,
        cfg_scale,
        cfg_rescale,
        character_captions,
        has_metadata,
        raw_text: texts
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("\n\n"),
        entries: texts,
    })
}
