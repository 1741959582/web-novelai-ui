use crate::store::{
    add_history, get_token, save_image_bytes, set_account, set_token, AccountSummary, HistoryItem,
};
use crate::store::{load_settings, AppSettings};
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};
use url::Url;
use uuid::Uuid;

const OFFICIAL_IMAGE: &str = "https://image.novelai.net";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub model: String,
    pub style_prompt: String,
    pub positive_prompt: String,
    pub negative_prompt: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg_scale: f64,
    pub cfg_rescale: f64,
    pub sampler: String,
    pub noise_schedule: String,
    pub seed: u32,
    pub seed_mode: String,
    pub uc_preset: u8,
    pub quality_preset: String,
    pub transparent_background: bool,
    pub smea: bool,
    pub smea_dyn: bool,
    pub variety: bool,
    pub file_name_prefix: String,
    pub model_mode: Option<String>,
    pub image_base64: Option<String>,
    pub strength: Option<f64>,
    pub char_captions: Option<Vec<CharCaption>>,
    pub vibe_images: Option<Vec<VibeImageIn>>,
    pub precise_references: Option<Vec<PreciseRefIn>>,
    pub normalize_vibe: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VibeImageIn {
    pub base64: String,
    pub info_extracted: f64,
    pub strength: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreciseRefIn {
    pub base64: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub strength: f64,
    pub fidelity: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharCaption {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub use_coords: bool,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    pub ok: bool,
    pub message: String,
    pub items: Vec<HistoryItem>,
    pub actual_seed: u32,
    pub account: AccountSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStatus {
    pub valid: bool,
    pub message: String,
    pub account: AccountSummary,
}

fn is_v5(model: &str) -> bool {
    normalize_model(model).starts_with("nai-diffusion-5-")
}

fn is_v4_plus(model: &str) -> bool {
    let m = normalize_model(model);
    m.starts_with("nai-diffusion-4-") || m.starts_with("nai-diffusion-5-")
}

fn normalize_model(model: &str) -> String {
    model
        .strip_suffix("-inpainting")
        .unwrap_or(model)
        .to_string()
}

fn snap64(v: u32, fallback: u32) -> u32 {
    let n = if v == 0 { fallback } else { v };
    ((n.max(64) / 64) * 64).min(49152)
}

fn merge_prompt(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn nai_source_name(model: &str) -> String {
    let m = normalize_model(model);
    match m.as_str() {
        "nai-diffusion-5-curated" => "NovelAI Diffusion V5 Curated".into(),
        "nai-diffusion-5-full" => "NovelAI Diffusion V5 Full".into(),
        "nai-diffusion-4-5-curated" => "NovelAI Diffusion V4.5 Curated".into(),
        "nai-diffusion-4-5-full" => "NovelAI Diffusion V4.5 Full".into(),
        "nai-diffusion-4-curated" => "NovelAI Diffusion V4 Curated".into(),
        "nai-diffusion-4-full" => "NovelAI Diffusion V4 Full".into(),
        "nai-diffusion-furry-3" => "NovelAI Diffusion Furry V3".into(),
        "nai-diffusion-3" => "NovelAI Diffusion V3".into(),
        _ => "NovelAI".into(),
    }
}

fn embed_png(png: Vec<u8>, req: &GenerateRequest, seed: u32, payload: &Value) -> Vec<u8> {
    if !png.starts_with(b"\x89PNG") {
        return png;
    }
    let prompt = merge_prompt(&[&req.style_prompt, &req.positive_prompt]);
    let parameters = payload.get("parameters").cloned().unwrap_or(json!({}));
    let mut comment = json!({
        "prompt": prompt,
        "uc": req.negative_prompt,
        "steps": req.steps,
        "sampler": req.sampler,
        "seed": seed,
        "width": snap64(req.width, 832),
        "height": snap64(req.height, 1216),
        "scale": req.cfg_scale,
        "cfg_rescale": req.cfg_rescale,
        "model": req.model,
        "noise_schedule": req.noise_schedule,
    });
    if let Some(v) = parameters.get("v4_prompt") {
        comment["v4_prompt"] = v.clone();
    }
    if let Some(v) = parameters.get("v4_negative_prompt") {
        comment["v4_negative_prompt"] = v.clone();
    }
    crate::png_meta::inject_text_chunks(
        &png,
        &[
            ("Software", "NovelAI".into()),
            ("Source", nai_source_name(&req.model)),
            ("Description", prompt),
            ("Comment", comment.to_string()),
        ],
    )
}

fn quality_tags(model: &str, preset: &str, positive: &str) -> String {
    if preset == "none" {
        return String::new();
    }
    let mut tags = if preset == "light" && is_v5(model) {
        "very aesthetic, amazing quality, no text".to_string()
    } else {
        match normalize_model(model).as_str() {
            "nai-diffusion-5-full" | "nai-diffusion-5-curated" | "nai-diffusion-4-5-full" => {
                "very aesthetic, masterpiece, no text".into()
            }
            "nai-diffusion-4-5-curated" => {
                "very aesthetic, masterpiece, no text, -0.8::feet::, rating:general".into()
            }
            "nai-diffusion-4-full" => "no text, best quality, very aesthetic, absurdres".into(),
            "nai-diffusion-4-curated" => {
                "rating:general, best quality, very aesthetic, absurdres".into()
            }
            "nai-diffusion-3" => "best quality, amazing quality, very aesthetic, absurdres".into(),
            _ => String::new(),
        }
    };
    if regex_has_text_directive(positive) {
        tags = tags
            .split(',')
            .map(str::trim)
            .filter(|t| !t.eq_ignore_ascii_case("no text"))
            .collect::<Vec<_>>()
            .join(", ");
    }
    tags
}

fn regex_has_text_directive(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("text:") && lower.split("text:").nth(1).is_some_and(|rest| {
        rest.trim_start().chars().next().is_some_and(|c| !c.is_whitespace())
    })
}

fn uc_preset_text(model: &str, preset: u8) -> String {
    if preset == 3 {
        return String::new();
    }
    let normalized = normalize_model(model);
    let key = match normalized.as_str() {
        "nai-diffusion-5-full" => "nai-diffusion-4-5-full",
        "nai-diffusion-5-curated" => "nai-diffusion-4-5-curated",
        other => other,
    };
    if preset == 2 {
        return match key {
            "nai-diffusion-4-5-full" => "lowres, artistic error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, dithering, halftone, screentone, multiple views, logo, too many watermarks, negative space, blank page, @_@, mismatched pupils, glowing eyes, bad anatomy".into(),
            "nai-diffusion-4-5-curated" => "blurry, lowres, upscaled, artistic error, film grain, scan artifacts, bad anatomy, bad hands, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, halftone, multiple views, logo, too many watermarks, @_@, mismatched pupils, glowing eyes, negative space, blank page".into(),
            "nai-diffusion-3" => "lowres, {bad}, error, fewer, extra, missing, worst quality, jpeg artifacts, bad quality, watermark, unfinished, displeasing, chromatic aberration, signature, extra digits, artistic error, username, scan, [abstract], bad anatomy, bad hands, @_@, mismatched pupils, heart-shaped pupils, glowing eyes".into(),
            _ => String::new(),
        };
    }
    match (key, preset) {
        ("nai-diffusion-4-5-full", 0) => "lowres, artistic error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, dithering, halftone, screentone, multiple views, logo, too many watermarks, negative space, blank page".into(),
        ("nai-diffusion-4-5-full", _) => "lowres, artistic error, scan artifacts, worst quality, bad quality, jpeg artifacts, multiple views, very displeasing, too many watermarks, negative space, blank page".into(),
        ("nai-diffusion-4-5-curated", 0) => "blurry, lowres, upscaled, artistic error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, halftone, multiple views, logo, too many watermarks, negative space, blank page".into(),
        ("nai-diffusion-4-5-curated", _) => "blurry, lowres, upscaled, artistic error, scan artifacts, jpeg artifacts, logo, too many watermarks, negative space, blank page".into(),
        ("nai-diffusion-4-full", 0) => "blurry, lowres, error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, multiple views, logo, too many watermarks".into(),
        ("nai-diffusion-4-full", _) => "blurry, lowres, error, worst quality, bad quality, jpeg artifacts, very displeasing".into(),
        ("nai-diffusion-4-curated", 0) => "blurry, lowres, error, film grain, scan artifacts, worst quality, bad quality, jpeg artifacts, very displeasing, chromatic aberration, logo, dated, signature, multiple views, gigantic breasts".into(),
        ("nai-diffusion-4-curated", _) => "blurry, lowres, error, worst quality, bad quality, jpeg artifacts, very displeasing, logo, dated, signature".into(),
        ("nai-diffusion-3", 0) => "lowres, {bad}, error, fewer, extra, missing, worst quality, jpeg artifacts, bad quality, watermark, unfinished, displeasing, chromatic aberration, signature, extra digits, artistic error, username, scan, [abstract]".into(),
        ("nai-diffusion-3", _) => "lowres, jpeg artifacts, worst quality, watermark, blurry, very displeasing".into(),
        _ => String::new(),
    }
}

fn is_official_host(base: &str) -> bool {
    Url::parse(base)
        .ok()
        .map(|u| {
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            let https = u.scheme() == "https";
            let nai = host == "novelai.net" || host.ends_with(".novelai.net");
            if nai {
                return https;
            }
            host == "localhost" || host == "127.0.0.1" || host == "::1"
        })
        .unwrap_or(false)
}

fn token_safe_base(raw: &str, fallback: &str, settings: &AppSettings) -> String {
    let resolved = raw.trim().trim_end_matches('/').to_string();
    let resolved = if resolved.is_empty() {
        fallback.to_string()
    } else {
        resolved
    };
    if is_official_host(&resolved) {
        if let (Ok(u), Ok(f)) = (Url::parse(&resolved), Url::parse(fallback)) {
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            let fb = f.host_str().unwrap_or("").to_ascii_lowercase();
            let loopback = host == "localhost" || host == "127.0.0.1" || host == "::1";
            if loopback || host == fb {
                return resolved;
            }
            if host == "novelai.net" || host.ends_with(".novelai.net") {
                return fallback.to_string();
            }
        }
    }
    if settings.allow_custom_endpoint {
        resolved
    } else {
        fallback.to_string()
    }
}

fn build_client(settings: &AppSettings) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .user_agent("Langbai-NovelAI-Studio/1");
    if let Some(proxy) = detect_proxy(&settings.proxy_url) {
        let p = reqwest::Proxy::all(&proxy).map_err(|e| e.to_string())?;
        builder = builder.proxy(p);
    }
    builder.build().map_err(|e| e.to_string())
}

fn port_open(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok()
}

fn detect_proxy(explicit: &str) -> Option<String> {
    let explicit = explicit.trim();
    if !explicit.is_empty() {
        return Some(if explicit.contains("://") {
            explicit.to_string()
        } else {
            format!("http://{explicit}")
        });
    }
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    if port_open(7890) {
        return Some("http://127.0.0.1:7890".into());
    }
    if port_open(7897) {
        return Some("http://127.0.0.1:7897".into());
    }
    if port_open(10809) {
        return Some("http://127.0.0.1:10809".into());
    }
    if port_open(10808) {
        return Some("socks5://127.0.0.1:10808".into());
    }
    None
}

fn strip_b64(value: &str) -> &str {
    value.split(',').next_back().unwrap_or(value).trim()
}

fn decode_b64(value: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(strip_b64(value).as_bytes())
        .map_err(|e| e.to_string())
}

fn encode_b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn vibe_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn auth_headers(token: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| e.to_string())?,
    );
    Ok(headers)
}

fn parse_account(data: &Value) -> AccountSummary {
    let sub = data
        .get("subscription")
        .or_else(|| data.pointer("/information/subscription"))
        .or_else(|| data.pointer("/data/subscription"))
        .cloned()
        .unwrap_or(Value::Null);
    let tier = sub.get("tier").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let active = sub.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
    let anlas = sub
        .get("trainingStepsLeft")
        .and_then(|v| {
            if v.is_object() {
                let a = v
                    .get("fixedTrainingStepsLeft")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0);
                let b = v
                    .get("purchasedTrainingSteps")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0);
                Some(a + b)
            } else {
                v.as_i64()
            }
        });
    let expires = sub.get("expiresAt").and_then(|v| v.as_i64()).map(|raw| {
        let seconds = if raw > 10_000_000_000 {
            raw / 1000
        } else {
            raw
        };
        chrono::DateTime::from_timestamp(seconds, 0)
            .map(|d| d.date_naive().to_string())
            .unwrap_or_default()
    });
    AccountSummary {
        has_token: true,
        tier_name: match tier {
            3 => "Opus",
            2 => "Scroll",
            1 => "Tablet",
            0 => "Paper",
            _ => "未知",
        }
        .into(),
        tier_level: Some(tier),
        anlas_balance: anlas,
        expires_at: expires.filter(|s| !s.is_empty()),
        has_active_subscription: active && tier > 0,
    }
}

async fn fetch_account(token: &str) -> Result<AccountSummary, String> {
    let settings = load_settings();
    let base = token_safe_base(&settings.image_base_url, OFFICIAL_IMAGE, &settings);
    let client = build_client(&settings)?;
    let res = client
        .get(format!("{base}/user/data"))
        .headers(auth_headers(token)?)
        .send()
        .await
        .map_err(format_reqwest)?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Token 无效或已过期。请到设置中重新填写 Persistent API Token。".into());
        }
        return Err(format!("验证失败 HTTP {status}: {body}"));
    }
    let data: Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(parse_account(&data))
}

fn build_payload(req: &GenerateRequest, seed: u32, action: &str) -> Value {
    let width = snap64(req.width, 832);
    let height = snap64(req.height, 1216);
    let model = req.model.clone();
    let mut base = merge_prompt(&[&req.style_prompt, &req.positive_prompt]);
    if req.model_mode.as_deref() == Some("furry")
        && is_v4_plus(&model)
        && !base.to_ascii_lowercase().contains("fur dataset")
    {
        base = merge_prompt(&["fur dataset", &base]);
    }
    let quality = quality_tags(&model, &req.quality_preset, &base);
    let mut prompt = merge_prompt(&[&base, &quality]);
    if is_v5(&model) && req.transparent_background {
        prompt = merge_prompt(&[&prompt, "transparent background"]);
    }
    let negative = merge_prompt(&[&req.negative_prompt, &uc_preset_text(&model, req.uc_preset)]);
    let v5 = is_v5(&model);
    let noise = if v5 {
        "karras"
    } else if req.noise_schedule.is_empty() {
        "native"
    } else {
        req.noise_schedule.as_str()
    };
    let mut parameters = json!({
        "params_version": 4,
        "width": width,
        "height": height,
        "scale": req.cfg_scale.clamp(0.0, 10.0),
        "sampler": req.sampler,
        "steps": req.steps.clamp(1, 50),
        "n_samples": 1,
        "seed": seed,
        "noise_schedule": noise,
        "uc": negative,
        "negative_prompt": negative,
        "ucPreset": req.uc_preset,
        "uc_preset": req.uc_preset,
        "cfg_rescale": req.cfg_rescale.clamp(0.0, 1.0),
        "legacy": false,
        "legacy_v3_extend": false,
        "dynamic_thresholding": if v5 { false } else { req.cfg_rescale > 0.0 },
        "skip_cfg_above_sigma": null,
        "qualityPresetId": req.quality_preset,
        "qualityToggle": req.quality_preset != "none",
        "quality_toggle": req.quality_preset != "none",
        "tag_hint_qt": if req.quality_preset == "standard" { 1 } else if req.quality_preset == "light" { 3 } else { 0 },
    });
    if v5 {
        parameters["tag_hint_transparent_background"] = json!(req.transparent_background);
        parameters["straight_alpha"] = json!(req.transparent_background);
    }
    if req.variety && !v5 {
        parameters["skip_cfg_above_sigma"] = json!(58);
    }
    if req.sampler == "k_euler_ancestral" && noise != "native" {
        parameters["deliberate_euler_ancestral_bug"] = json!(false);
        parameters["prefer_brownian"] = json!(true);
    }
    if is_v4_plus(&model) {
        let captions = req.char_captions.clone().unwrap_or_default();
        let char_payload: Vec<Value> = captions
            .iter()
            .filter(|c| c.enabled.unwrap_or(true) && !c.prompt.trim().is_empty())
            .map(|c| {
                json!({
                    "char_caption": c.prompt,
                    "centers": [{ "x": if c.use_coords { c.x } else { 0.5 }, "y": if c.use_coords { c.y } else { 0.5 } }]
                })
            })
            .collect();
        let negative_char_payload: Vec<Value> = captions
            .iter()
            .filter(|c| c.enabled.unwrap_or(true) && !c.prompt.trim().is_empty())
            .map(|c| {
                json!({
                    "char_caption": c.negative_prompt.clone().unwrap_or_default(),
                    "centers": [{ "x": if c.use_coords { c.x } else { 0.5 }, "y": if c.use_coords { c.y } else { 0.5 } }]
                })
            })
            .collect();
        let use_coords = captions.iter().any(|c| c.use_coords);
        parameters["use_coords"] = json!(use_coords);
        parameters["v4_prompt"] = json!({
            "caption": { "base_caption": prompt, "char_captions": char_payload },
            "use_coords": use_coords,
            "use_order": true
        });
        parameters["v4_negative_prompt"] = json!({
            "caption": { "base_caption": negative, "char_captions": negative_char_payload },
            "use_coords": use_coords && negative_char_payload.iter().any(|v| v["char_caption"].as_str().is_some_and(|s| !s.is_empty())),
            "use_order": false,
            "legacy_uc": !is_v5(&model) && !normalize_model(&model).starts_with("nai-diffusion-4-5")
        });
    } else {
        parameters["sm"] = json!(req.smea);
        parameters["sm_dyn"] = json!(req.smea && req.smea_dyn);
    }
    if action == "img2img" {
        if let Some(img) = &req.image_base64 {
            let b64 = strip_b64(img);
            parameters["image"] = json!(b64);
            parameters["strength"] = json!(req.strength.unwrap_or(0.7).clamp(0.0, 1.0));
            parameters["noise"] = json!(0);
        }
    }
    let mut vibes = req.vibe_images.clone().unwrap_or_default();
    if is_v5(&model) {
        vibes.clear();
    }
    if !vibes.is_empty() {
        parameters["reference_image_multiple"] = json!(vibes
            .iter()
            .map(|v| strip_b64(&v.base64))
            .collect::<Vec<_>>());
        parameters["reference_information_extracted_multiple"] = json!(vibes
            .iter()
            .map(|v| v.info_extracted.clamp(0.0, 1.0))
            .collect::<Vec<_>>());
        parameters["reference_strength_multiple"] = json!(vibes
            .iter()
            .map(|v| v.strength.clamp(0.0, 1.0))
            .collect::<Vec<_>>());
        parameters["normalize_reference_strength_multiple"] = json!(req.normalize_vibe.unwrap_or(true));
    }
    let mut precise = req.precise_references.clone().unwrap_or_default();
    if !normalize_model(&model).starts_with("nai-diffusion-4-5-") {
        precise.clear();
    }
    if !precise.is_empty() {
        let bins: Vec<Vec<u8>> = precise
            .iter()
            .filter_map(|r| decode_b64(&r.base64).ok())
            .collect();
        parameters["director_reference_images"] = json!(bins.iter().map(|b| encode_b64(b)).collect::<Vec<_>>());
        parameters["director_reference_images_cached"] = json!(bins
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                json!({
                    "cache_secret_key": sha256_hex(bytes),
                    "data": format!("director_ref_{index}")
                })
            })
            .collect::<Vec<_>>());
        parameters["normalize_reference_strength_multiple"] = json!(true);
        parameters["director_reference_descriptions"] = json!(precise
            .iter()
            .map(|r| {
                let kind = if r.kind.trim().is_empty() {
                    "character&style"
                } else {
                    r.kind.as_str()
                };
                json!({
                    "caption": { "base_caption": kind, "char_captions": [] },
                    "legacy_uc": false
                })
            })
            .collect::<Vec<_>>());
        parameters["director_reference_strength_values"] = json!(precise
            .iter()
            .map(|r| (r.strength.clamp(0.0, 1.0) * 100.0).round() / 100.0)
            .collect::<Vec<_>>());
        parameters["director_reference_secondary_strength_values"] = json!(precise
            .iter()
            .map(|r| (((1.0 - r.fidelity.clamp(0.0, 1.0)) * 100.0).round()) / 100.0)
            .collect::<Vec<_>>());
        parameters["director_reference_information_extracted"] =
            json!(precise.iter().map(|_| 1.0).collect::<Vec<_>>());
    }
    json!({
        "input": prompt,
        "model": model,
        "action": action,
        "parameters": parameters
    })
}

fn unzip_images(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        return Ok(vec![bytes.to_vec()]);
    }
    let reader = Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let mut images = Vec::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut buf).map_err(|e| e.to_string())?;
        if !buf.is_empty() {
            images.push(buf);
        }
    }
    Ok(images)
}

pub fn format_reqwest(err: reqwest::Error) -> String {
    let mut msg = err.to_string();
    let mut src = std::error::Error::source(&err);
    while let Some(s) = src {
        msg.push_str(" → ");
        msg.push_str(&s.to_string());
        src = s.source();
    }
    if err.is_timeout() {
        format!("请求超时。请检查网络或代理。{msg}")
    } else if err.is_connect() {
        format!("无法连接 NovelAI。请检查网络，或在设置中填写代理（如 http://127.0.0.1:7890）。{msg}")
    } else {
        msg
    }
}

pub fn http_status_error(status: reqwest::StatusCode, body: &str) -> String {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        "Token 无效或已过期。请到设置中重新填写 Persistent API Token。".into()
    } else {
        format!("生成失败 HTTP {status}: {body}")
    }
}

async fn encode_vibe_image(
    client: &reqwest::Client,
    token: &str,
    base: &str,
    model: &str,
    image_b64: &str,
    info_extracted: f64,
) -> Result<String, String> {
    let raw = strip_b64(image_b64);
    let key = format!("{model}|{info_extracted:.2}|{}", sha256_hex(raw.as_bytes()));
    if let Ok(cache) = vibe_cache().lock() {
        if let Some(hit) = cache.get(&key) {
            return Ok(hit.clone());
        }
    }
    let res = client
        .post(format!("{base}/ai/encode-vibe"))
        .headers(auth_headers(token)?)
        .json(&json!({
            "image": raw,
            "information_extracted": info_extracted.clamp(0.0, 1.0),
            "model": model
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Vibe 编码失败 HTTP {status}: {body}"));
    }
    let bytes = res.bytes().await.map_err(|e| e.to_string())?;
    let encoded = encode_b64(&bytes);
    if let Ok(mut cache) = vibe_cache().lock() {
        cache.insert(key, encoded.clone());
    }
    Ok(encoded)
}

async fn prepare_request(req: &GenerateRequest, base: &str, token: &str) -> Result<GenerateRequest, String> {
    if is_v5(&req.model) {
        return Ok(req.clone());
    }
    let settings = load_settings();
    let client = build_client(&settings)?;
    let Some(vibes) = req.vibe_images.clone() else {
        return Ok(req.clone());
    };
    if vibes.is_empty() {
        return Ok(req.clone());
    }
    let mut next = req.clone();
    let mut encoded = Vec::with_capacity(vibes.len());
    for vibe in vibes {
        match encode_vibe_image(
            &client,
            token,
            base,
            &req.model,
            &vibe.base64,
            vibe.info_extracted,
        )
        .await
        {
            Ok(b64) => encoded.push(VibeImageIn {
                base64: b64,
                info_extracted: vibe.info_extracted,
                strength: vibe.strength,
            }),
            Err(_) => encoded.push(vibe),
        }
    }
    next.vibe_images = Some(encoded);
    Ok(next)
}

fn build_generate_form(payload: Value) -> Result<Form, String> {
    let mut request = payload;
    let params = request
        .get_mut("parameters")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "payload missing parameters".to_string())?;
    let images: Vec<String> = params
        .get("director_reference_images")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    params.remove("director_reference_images");

    let mut i2i: Option<Vec<u8>> = None;
    if let Some(Value::String(img)) = params.get("image").cloned() {
        if let Ok(bytes) = decode_b64(&img) {
            i2i = Some(bytes);
            params.insert("image".into(), json!("image"));
        }
    }

    let request_part = Part::text(request.to_string())
        .mime_str("application/json")
        .map_err(|e| e.to_string())?;
    let mut form = Form::new().part("request", request_part);
    if let Some(bytes) = i2i {
        let part = Part::bytes(bytes)
            .file_name("image")
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;
        form = form.part("image", part);
    }
    for (index, b64) in images.into_iter().enumerate() {
        let bytes = decode_b64(&b64)?;
        let part = Part::bytes(bytes)
            .file_name("blob")
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;
        form = form.part(format!("director_ref_{index}"), part);
    }
    Ok(form)
}

fn build_stream_form(payload: &Value) -> Result<Form, String> {
    let request_part = Part::text(payload.to_string())
        .file_name("blob")
        .mime_str("application/json")
        .map_err(|e| e.to_string())?;
    Ok(Form::new().part("request", request_part))
}

fn extra_stream_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&chrono::Utc::now().to_rfc3339()) {
        headers.insert("x-initiated-at", value);
    }
    let id = Uuid::new_v4().simple().to_string();
    if let Ok(value) = HeaderValue::from_str(&id[..id.len().min(8)]) {
        headers.insert("x-correlation-id", value);
    }
    headers
}

async fn try_stream_generate(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    token: &str,
    base: &str,
    payload: &Value,
    total_steps: u32,
) -> Result<Option<Vec<Vec<u8>>>, String> {
    let res = client
        .post(format!("{base}/ai/generate-image-stream"))
        .headers(auth_headers(token)?)
        .headers(extra_stream_headers())
        .header(
            "Accept",
            "application/x-msgpack, text/event-stream, application/zip",
        )
        .multipart(build_stream_form(payload)?)
        .send()
        .await
        .map_err(format_reqwest)?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if matches!(status.as_u16(), 404 | 405 | 415 | 501) || crate::nai_stream::is_streaming_not_allowed(&body) {
            return Ok(None);
        }
        return Err(http_status_error(status, &body));
    }
    match crate::nai_stream::consume_generate_stream(app, res, total_steps).await {
        Ok(crate::nai_stream::StreamResult::Images(images)) => Ok(Some(images)),
        Ok(crate::nai_stream::StreamResult::Zip(bytes)) => Ok(Some(unzip_images(&bytes)?)),
        Err(err) if crate::nai_stream::is_streaming_not_allowed(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

async fn post_generate(
    app: &tauri::AppHandle,
    req: &GenerateRequest,
    action: &str,
) -> Result<GenerateResult, String> {
    let token = get_token();
    if token.is_empty() {
        return Err("请先在设置中配置 API Token。".into());
    }
    let settings = load_settings();
    let seed = if req.seed_mode == "fixed" && req.seed > 0 {
        req.seed
    } else {
        (uuid::Uuid::new_v4().as_u128() % 2_147_483_647) as u32
    };
    let base = token_safe_base(&settings.image_base_url, OFFICIAL_IMAGE, &settings);
    let prepared = prepare_request(req, &base, &token).await?;
    let payload = build_payload(&prepared, seed.max(1), action);
    let client = build_client(&settings)?;
    let use_multipart = prepared
        .precise_references
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let total_steps = req.steps.clamp(1, 50);
    crate::nai_stream::emit_waiting(app, total_steps);

    let send_zip = |url: String, payload: Value, multipart: bool| {
        let client = client.clone();
        let token = token.clone();
        async move {
            let builder = client
                .post(url)
                .headers(auth_headers(&token)?)
                .header("Accept", "application/zip, application/octet-stream");
            if multipart {
                builder
                    .multipart(build_generate_form(payload)?)
                    .send()
                    .await
                    .map_err(format_reqwest)
            } else {
                builder.json(&payload).send().await.map_err(format_reqwest)
            }
        }
    };

    let mut images: Option<Vec<Vec<u8>>> = None;
    if settings.stream_preview_enabled && crate::nai_stream::supports_safe_stream(&payload, action) {
        match try_stream_generate(app, &client, &token, &base, &payload, total_steps).await {
            Ok(Some(got)) => images = Some(got),
            Ok(None) => {}
            Err(err) if err.contains("Token 无效") => return Err(err),
            Err(err)
                if base != OFFICIAL_IMAGE
                    && settings.allow_custom_endpoint_fallback
                    && (err.contains("401") || err.contains("403") || err.contains("Token")) =>
            {
                images = try_stream_generate(app, &client, &token, OFFICIAL_IMAGE, &payload, total_steps)
                    .await
                    .ok()
                    .flatten();
            }
            Err(_) => {}
        }
    }

    if images.is_none() {
        crate::nai_stream::emit_waiting(app, total_steps);
        let mut res = send_zip(
            format!("{base}/ai/generate-image"),
            payload.clone(),
            use_multipart,
        )
        .await?;
        if (res.status() == reqwest::StatusCode::UNAUTHORIZED
            || res.status() == reqwest::StatusCode::FORBIDDEN)
            && base != OFFICIAL_IMAGE
            && settings.allow_custom_endpoint_fallback
        {
            res = send_zip(
                format!("{OFFICIAL_IMAGE}/ai/generate-image"),
                payload.clone(),
                use_multipart,
            )
            .await?;
        }
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(http_status_error(status, &body));
        }
        let bytes = res.bytes().await.map_err(format_reqwest)?;
        images = Some(unzip_images(&bytes)?);
    }

    let images = images.unwrap_or_default();
    if images.is_empty() {
        return Err("接口成功但没有返回图片。".into());
    }
    if let Some(first) = images.first() {
        let preview = format!(
            "data:image/png;base64,{}",
            encode_b64(first)
        );
        crate::nai_stream::emit_saving(app, &preview, total_steps);
    }
    let mut items = Vec::new();
    for img in images {
        let saved = if img.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
            embed_png(img, req, seed, &payload)
        } else {
            img
        };
        let ext = if saved.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
            "png"
        } else {
            "bin"
        };
        let path = save_image_bytes(&saved, &req.file_name_prefix, seed, ext)?;
        let item = HistoryItem {
            id: Uuid::new_v4().to_string(),
            path: path.to_string_lossy().into(),
            created_at: chrono::Local::now().to_rfc3339(),
            model: req.model.clone(),
            prompt: merge_prompt(&[&req.style_prompt, &req.positive_prompt]),
            negative_prompt: req.negative_prompt.clone(),
            seed,
            width: snap64(req.width, 832),
            height: snap64(req.height, 1216),
            steps: req.steps,
            sampler: req.sampler.clone(),
            kind: if action == "img2img" { "i2i" } else { "txt2img" }.into(),
            session_id: String::new(),
        };
        add_history(item.clone())?;
        items.push(item);
    }
    let account = fetch_account(&token).await.unwrap_or_else(|_| {
        let mut acc = crate::store::get_account();
        acc.has_token = true;
        acc
    });
    let _ = set_account(account.clone());
    Ok(GenerateResult {
        ok: true,
        message: format!("已保存 {} 张图片", items.len()),
        items,
        actual_seed: seed,
        account,
    })
}

#[tauri::command]
pub async fn token_verify(token: String) -> Result<TokenStatus, String> {
    let normalized = token.trim().to_string();
    if normalized.is_empty() {
        return Ok(TokenStatus {
            valid: false,
            message: "请输入 NovelAI Persistent API Token。".into(),
            account: AccountSummary::default(),
        });
    }
    match fetch_account(&normalized).await {
        Ok(mut account) => {
            account.has_token = true;
            set_token(normalized)?;
            set_account(account.clone())?;
            Ok(TokenStatus {
                valid: true,
                message: format!(
                    "已连接 {}，Anlas {}",
                    account.tier_name,
                    account.anlas_balance.unwrap_or(0)
                ),
                account,
            })
        }
        Err(e) => Ok(TokenStatus {
            valid: false,
            message: e,
            account: AccountSummary::default(),
        }),
    }
}

#[tauri::command]
pub async fn account_refresh() -> Result<AccountSummary, String> {
    let token = get_token();
    if token.is_empty() {
        return Err("请先配置 API Token。".into());
    }
    let account = fetch_account(&token).await?;
    set_account(account.clone())?;
    Ok(account)
}

#[tauri::command]
pub async fn generate_txt2img(app: tauri::AppHandle, request: GenerateRequest) -> Result<GenerateResult, String> {
    post_generate(&app, &request, "generate").await
}

#[tauri::command]
pub async fn generate_img2img(app: tauri::AppHandle, request: GenerateRequest) -> Result<GenerateResult, String> {
    if request
        .image_base64
        .as_ref()
        .map(|s| s.is_empty())
        .unwrap_or(true)
    {
        return Err("请先加载参考图片。".into());
    }
    post_generate(&app, &request, "img2img").await
}

