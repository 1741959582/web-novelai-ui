//! Still-image auto censor, following the desktop tool's YOLO ONNX path:
//! letterbox, multi-engine union, precise flip + tiles, face guard, fit/ellipse/rect
//! masks, dilate, then mosaic or blur only inside the mask.

use crate::apng::{decode_data_url, save_png_image};
use futures_util::StreamExt;
use image::{imageops, Rgb, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const RAW_FLOOR: f32 = 0.05;
const FACE_CONF: f32 = 0.35;
const FACE_ENGINE: &str = "anime_face";

const FACE_GUARD_LABELS: &[&str] = &[
    "female_genital",
    "anus",
    "nipple",
    "xray",
    "female_genital_covered",
    "anus_covered",
    "sex",
];

struct BuiltinModel {
    id: &'static str,
    title: &'static str,
    file: &'static str,
    urls: &'static [&'static str],
    conf: f32,
    kind: &'static str,
}

const BUILTIN: &[BuiltinModel] = &[
    BuiltinModel {
        id: "ntd11",
        title: "二次元 · ntd11 分割（推荐）",
        file: "ntd11_anime_nsfw_segm_v5-variant1.onnx",
        urls: &["https://github.com/caiweida/web-novelai-ui/releases/download/v0.1.9/ntd11_anime_nsfw_segm_v5-variant1.onnx"],
        conf: 0.40,
        kind: "anime",
    },
    BuiltinModel {
        id: "deepghs_s",
        title: "二次元 · deepghs censor s（框）",
        file: "deepghs_censor_v1.0_s.onnx",
        urls: &[
            "https://github.com/caiweida/web-novelai-ui/releases/download/v0.1.9/deepghs_censor_v1.0_s.onnx",
            "https://huggingface.co/deepghs/anime_censor_detection/resolve/main/censor_detect_v1.0_s/model.onnx",
            "https://hf-mirror.com/deepghs/anime_censor_detection/resolve/main/censor_detect_v1.0_s/model.onnx",
        ],
        conf: 0.40,
        kind: "anime",
    },
    BuiltinModel {
        id: "nudenet_640m",
        title: "真人 · NudeNet 640m（框）",
        file: "nudenet_640m.onnx",
        urls: &["https://github.com/notAI-tech/NudeNet/releases/download/v3.4-weights/640m.onnx"],
        conf: 0.30,
        kind: "real",
    },
    BuiltinModel {
        id: "nudenet_320n",
        title: "真人 · NudeNet 320n（快，准度低）",
        file: "nudenet_320n.onnx",
        urls: &["https://github.com/notAI-tech/NudeNet/releases/download/v3.4-weights/320n.onnx"],
        conf: 0.25,
        kind: "real",
    },
    BuiltinModel {
        id: "anime_face",
        title: "面部检测 · deepghs anime face s",
        file: "deepghs_face_v1.4_s.onnx",
        urls: &[
            "https://github.com/caiweida/web-novelai-ui/releases/download/v0.1.9/deepghs_face_v1.4_s.onnx",
            "https://huggingface.co/deepghs/anime_face_detection/resolve/main/face_detect_v1.4_s/model.onnx",
            "https://hf-mirror.com/deepghs/anime_face_detection/resolve/main/face_detect_v1.4_s/model.onnx",
        ],
        conf: FACE_CONF,
        kind: "face",
    },
];

const NUDENET_LABELS: &[&str] = &[
    "FEMALE_GENITALIA_COVERED",
    "FACE_FEMALE",
    "BUTTOCKS_EXPOSED",
    "FEMALE_BREAST_EXPOSED",
    "FEMALE_GENITALIA_EXPOSED",
    "MALE_BREAST_EXPOSED",
    "ANUS_EXPOSED",
    "FEET_EXPOSED",
    "BELLY_COVERED",
    "FEET_COVERED",
    "ARMPITS_COVERED",
    "ARMPITS_EXPOSED",
    "FACE_MALE",
    "BELLY_EXPOSED",
    "MALE_GENITALIA_EXPOSED",
    "ANUS_COVERED",
    "FEMALE_BREAST_COVERED",
    "BUTTOCKS_COVERED",
];

#[derive(Clone)]
struct Det {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    label: String,
    score: f32,
    /// Mask cropped to the box, row-major, 0 or 255. Length is (x1-x0)*(y1-y0).
    mask: Option<Vec<u8>>,
}

struct YoloEngine {
    session: ort::session::Session,
    input_name: String,
    size: i32,
    labels: Vec<String>,
    seg: bool,
}

struct Loaded {
    path: PathBuf,
    modified: Option<SystemTime>,
    engine: YoloEngine,
}

fn hub() -> &'static Mutex<HashMap<String, Loaded>> {
    static HUB: std::sync::OnceLock<Mutex<HashMap<String, Loaded>>> = std::sync::OnceLock::new();
    HUB.get_or_init(|| Mutex::new(HashMap::new()))
}

fn models_dir() -> Result<PathBuf, String> {
    let dir = crate::store::data_dir()?.join("censor-models");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    seed_local_models(&dir);
    Ok(dir)
}

fn bundled_models_dir() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    for start in starts {
        let mut current = Some(start.as_path());
        for _ in 0..6 {
            let Some(dir) = current else { break };
            let candidate = dir.join("models").join("models");
            if candidate.is_dir() {
                return Some(candidate);
            }
            current = dir.parent();
        }
    }
    None
}

fn seed_local_models(dest: &Path) {
    let Some(src) = bundled_models_dir() else {
        return;
    };
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|item| item.to_str()) else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".onnx") {
            continue;
        }
        let target = dest.join(name);
        if target.is_file() {
            continue;
        }
        let _ = fs::copy(&path, &target);
    }
}

fn builtin(id: &str) -> Option<&'static BuiltinModel> {
    BUILTIN.iter().find(|item| item.id == id)
}

fn model_path(id: &str) -> Result<PathBuf, String> {
    if let Some(spec) = builtin(id) {
        return Ok(models_dir()?.join(spec.file));
    }
    let Some(name) = id.strip_prefix("file:") else {
        return Err(format!("找不到模型 {id}"));
    };
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return Err("模型文件名不合法".into());
    }
    Ok(models_dir()?.join(name))
}

fn user_kind(stem: &str) -> (&'static str, f32, &'static str) {
    let stem = stem.to_ascii_lowercase();
    if stem.contains("ntd11") || stem.contains("anime_nsfw") {
        ("二次元", 0.40, "anime")
    } else if stem.contains("erax") {
        ("真人", 0.30, "real")
    } else {
        ("自定义", 0.35, "custom")
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CensorEngineInfo {
    pub id: String,
    pub title: String,
    pub present: bool,
    pub kind: String,
    pub default_conf: f32,
    pub builtin: bool,
    pub file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CensorStatus {
    pub dir: String,
    pub face_ready: bool,
    pub engines: Vec<CensorEngineInfo>,
}

pub fn status() -> Result<CensorStatus, String> {
    let dir = models_dir()?;
    let builtin_files: Vec<String> = BUILTIN.iter().map(|item| item.file.to_ascii_lowercase()).collect();
    let mut user = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".onnx") {
            continue;
        }
        if builtin_files.iter().any(|file| file == &name.to_ascii_lowercase()) {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
        let (prefix, conf, kind) = user_kind(stem);
        let task = if stem.to_ascii_lowercase().contains("seg") || stem.to_ascii_lowercase().contains("ntd11") {
            "分割"
        } else {
            "检测"
        };
        user.push(CensorEngineInfo {
            id: format!("file:{name}"),
            title: format!("{prefix} · {stem}（{task}）"),
            present: true,
            kind: kind.into(),
            default_conf: conf,
            builtin: false,
            file: name.into(),
        });
    }
    user.sort_by(|a, b| {
        let rank = |kind: &str| match kind {
            "anime" => 0,
            "real" => 1,
            _ => 2,
        };
        rank(&a.kind).cmp(&rank(&b.kind)).then(a.id.cmp(&b.id))
    });
    let mut engines = user;
    for spec in BUILTIN {
        let path = dir.join(spec.file);
        engines.push(CensorEngineInfo {
            id: spec.id.into(),
            title: spec.title.into(),
            present: path.is_file(),
            kind: spec.kind.into(),
            default_conf: spec.conf,
            builtin: true,
            file: spec.file.into(),
        });
    }
    Ok(CensorStatus {
        dir: dir.to_string_lossy().into_owned(),
        face_ready: builtin(FACE_ENGINE).is_some_and(|item| dir.join(item.file).is_file()),
        engines,
    })
}

#[tauri::command]
pub fn censor_status() -> Result<CensorStatus, String> {
    status()
}

#[tauri::command]
pub fn censor_open_dir() -> Result<(), String> {
    let dir = models_dir()?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }
    Ok(())
}

#[tauri::command]
pub async fn censor_download(id: String) -> Result<CensorStatus, String> {
    let spec = builtin(&id).ok_or_else(|| format!("只能下载内置模型，{id} 请手动放进模型目录"))?;
    let dest = models_dir()?.join(spec.file);
    if dest.is_file() && fs::metadata(&dest).map(|meta| meta.len()).unwrap_or(0) > 1024 * 1024 {
        return status();
    }
    let urls: Vec<String> = spec.urls.iter().map(|url| (*url).to_string()).collect();
    download_to(&urls, &dest).await?;
    let _ = hub().lock().unwrap_or_else(|err| err.into_inner()).remove(&id);
    status()
}

async fn download_to(urls: &[String], dest: &Path) -> Result<(), String> {
    let client = crate::nai::http_client_no_redirect()?;
    let tmp = dest.with_extension("part");
    let mut errors = Vec::new();
    for url in urls {
        if let Err(err) = fetch_one(&client, url, &tmp).await {
            let _ = fs::remove_file(&tmp);
            errors.push(format!("{url}\n{err}"));
            continue;
        }
        let len = fs::metadata(&tmp).map(|meta| meta.len()).unwrap_or(0);
        if len < 1024 * 1024 {
            let _ = fs::remove_file(&tmp);
            errors.push(format!("{url}\n下载内容过小"));
            continue;
        }
        let head = fs::read(&tmp).unwrap_or_default();
        let sniff = &head[..head.len().min(64)];
        if sniff.starts_with(b"<") || sniff.starts_with(b"version https://git-lfs") {
            let _ = fs::remove_file(&tmp);
            errors.push(format!("{url}\n下到的不是模型文件"));
            continue;
        }
        fs::rename(&tmp, dest).map_err(|e| format!("保存模型失败：{e}"))?;
        return Ok(());
    }
    Err(format!("模型下载失败。\n{}", errors.join("\n")))
}

async fn fetch_one(client: &reqwest::Client, start: &str, dest: &Path) -> Result<(), String> {
    let mut url = start.to_string();
    for _ in 0..8 {
        let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .trim()
                .to_string();
            if location.is_empty() {
                return Err("下载跳转没有地址".into());
            }
            let base = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
            url = base.join(&location).map_err(|e| e.to_string())?.to_string();
            continue;
        }
        if !status.is_success() {
            return Err(format!("HTTP {}", status.as_u16()));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut file, &chunk).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    Err("下载跳转次数过多".into())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CensorEngineChoice {
    pub id: String,
    pub conf: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CensorRequest {
    pub image: String,
    pub engines: Vec<CensorEngineChoice>,
    pub parts: Vec<String>,
    pub precise: bool,
    pub face_guard: bool,
    pub face_guard_male: bool,
    pub shape: String,
    pub mode: String,
    pub dilate: u32,
    pub strength: u32,
    pub min_block: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CensorResult {
    pub path: String,
    pub data_url: String,
    pub hits: u32,
    pub block: u32,
    pub note: String,
}

#[tauri::command]
pub fn censor_save_png(image: String) -> Result<crate::apng::SavedImage, String> {
    let img = decode_data_url(&image)?;
    save_png_image(&img, "mosaic")
}

#[tauri::command]
pub async fn censor_apply(request: CensorRequest) -> Result<CensorResult, String> {
    tokio::task::spawn_blocking(move || apply_request(request))
        .await
        .map_err(|e| e.to_string())?
}

fn apply_request(request: CensorRequest) -> Result<CensorResult, String> {
    if request.engines.is_empty() {
        return Err("先勾选至少一个检测模型".into());
    }
    if request.parts.is_empty() {
        return Err("先勾选要打码的部位".into());
    }
    for engine in &request.engines {
        let path = model_path(&engine.id)?;
        if !path.is_file() {
            return Err(format!("模型还没准备好：{}。内置模型点下载，自定义 onnx 放进模型目录。", engine.id));
        }
    }
    if request.face_guard && !model_path(FACE_ENGINE)?.is_file() {
        return Err("面部保护需要先下载面部检测模型".into());
    }
    #[cfg(windows)]
    prefer_app_dlls();
    let img = decode_data_url(&request.image)?;
    let (w, h) = img.dimensions();
    let block = block_size(w, h, request.strength, request.min_block);
    let (dets, note) = detect_parts(&img, &request)?;
    if dets.is_empty() {
        return Ok(CensorResult {
            path: String::new(),
            data_url: String::new(),
            hits: 0,
            block,
            note,
        });
    }
    let built = if request.shape == "rect" || request.shape == "ellipse" {
        build_mask(&img, &dets, &request.shape)
    } else {
        build_fit_mask(&img, &dets)
    };
    let mask = dilate_mask(&built, w, h, request.dilate.min(80));
    if !mask.iter().any(|px| *px > 0) {
        return Ok(CensorResult {
            path: String::new(),
            data_url: String::new(),
            hits: 0,
            block,
            note: "检出了部位，但遮罩是空的".into(),
        });
    }
    let mut out = img;
    paint(&mut out, &mask, block, &request.mode);
    let saved = save_png_image(&out, "mosaic")?;
    Ok(CensorResult {
        path: saved.path,
        data_url: saved.data_url,
        hits: dets.len() as u32,
        block,
        note: String::new(),
    })
}

fn detect_parts(img: &RgbaImage, request: &CensorRequest) -> Result<(Vec<Det>, String), String> {
    let parts: Vec<String> = request.parts.iter().map(|part| part.to_ascii_lowercase()).collect();
    let mut selected = Vec::new();
    let mut raws = Vec::new();
    for choice in &request.engines {
        let raw = infer_id(&choice.id, img, request.precise)?;
        let conf = choice.conf.clamp(0.01, 0.99);
        for det in &raw {
            if parts.iter().any(|part| part == &det.label) && det.score >= conf {
                selected.push(det.clone());
            }
        }
        raws.push(raw);
    }
    let before_guard = selected.len();
    let mut dropped = 0usize;
    if request.face_guard && !selected.is_empty() {
        let faces_raw = infer_id(FACE_ENGINE, img, false)?;
        raws.push(faces_raw);
        let (w, h) = img.dimensions();
        let faces = collect_faces(&raws, w, h);
        let (kept, gone) = face_guard(selected, &faces, request.face_guard_male);
        dropped = gone;
        selected = kept;
    }
    let note = if selected.is_empty() && dropped > 0 {
        "检出的部位落在脸上，面部保护已跳过".into()
    } else if selected.is_empty() && before_guard == 0 {
        "没有检出勾选的部位".into()
    } else {
        String::new()
    };
    Ok((selected, note))
}

fn infer_id(id: &str, img: &RgbaImage, precise: bool) -> Result<Vec<Det>, String> {
    let mut guard = hub().lock().unwrap_or_else(|err| err.into_inner());
    let loaded = ensure_locked(&mut guard, id)?;
    loaded.engine.detect(img, precise)
}

fn ensure_locked<'a>(guard: &'a mut HashMap<String, Loaded>, id: &str) -> Result<&'a mut Loaded, String> {
    let path = model_path(id)?;
    let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
    let fresh = guard.get(id).is_some_and(|item| item.path == path && item.modified == modified);
    if !fresh {
        let engine = YoloEngine::load(&path, id)?;
        guard.insert(
            id.to_string(),
            Loaded {
                path,
                modified,
                engine,
            },
        );
    }
    guard.get_mut(id).ok_or_else(|| format!("模型加载失败：{id}"))
}

#[cfg(windows)]
fn prefer_app_dlls() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let resources = dir.join("resources");
    let dll_dir = if resources.join("DirectML.dll").is_file() {
        resources
    } else {
        dir.to_path_buf()
    };
    let mut wide: Vec<u16> = std::os::windows::ffi::OsStrExt::encode_wide(dll_dir.as_os_str()).collect();
    wide.push(0);
    unsafe extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }
    unsafe {
        SetDllDirectoryW(wide.as_ptr());
    }
}

impl YoloEngine {
    fn load(path: &Path, id: &str) -> Result<Self, String> {
        #[cfg(windows)]
        prefer_app_dlls();
        let mut builder = ort::session::Session::builder().map_err(|e| format!("ONNX 初始化失败：{e}"))?;
        builder = builder
            .with_memory_pattern(false)
            .map_err(|e| format!("ONNX 初始化失败：{e}"))?;
        #[cfg(windows)]
        {
            use ort::execution_providers::{CPUExecutionProvider, DirectMLExecutionProvider};
            builder = match builder.with_execution_providers([
                DirectMLExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ]) {
                Ok(next) => next,
                Err(_) => ort::session::Session::builder()
                    .map_err(|e| format!("ONNX 初始化失败：{e}"))?
                    .with_memory_pattern(false)
                    .map_err(|e| format!("ONNX 初始化失败：{e}"))?,
            };
        }
        let session = builder
            .commit_from_file(path)
            .map_err(|e| format!("加载模型失败：{e}"))?;
        let input = session.inputs.first().ok_or("模型没有输入")?;
        let input_name = input.name.clone();
        let mut size = match &input.input_type {
            ort::value::ValueType::Tensor { ty, shape, .. } => {
                if *ty == ort::tensor::TensorElementType::Float16 {
                    return Err("这个模型是 float16，当前只支持 float32 的 onnx".into());
                }
                let side = shape.get(2).copied().unwrap_or(640);
                if side > 0 { side as i32 } else { 640 }
            }
            _ => return Err("模型输入不是张量".into()),
        };
        if let Ok(meta) = session.metadata() {
            if let Ok(Some(raw)) = meta.custom("imgsz") {
                if let Ok(values) = serde_json::from_str::<Vec<i32>>(&raw) {
                    if let Some(side) = values.first().copied() {
                        if side > 0 {
                            size = side;
                        }
                    }
                }
            }
        }
        let mut labels = session
            .metadata()
            .ok()
            .and_then(|meta| meta.custom("names").ok().flatten())
            .map(|raw| parse_class_names(&raw))
            .unwrap_or_default();
        if labels.is_empty() && id.contains("nudenet") {
            labels = NUDENET_LABELS.iter().map(|label| (*label).to_string()).collect();
        }
        let seg = session.outputs.len() >= 2
            && session.outputs.iter().any(|output| match &output.output_type {
                ort::value::ValueType::Tensor { shape, .. } => shape.len() == 4,
                _ => false,
            });
        Ok(Self {
            session,
            input_name,
            size,
            labels,
            seg,
        })
    }

    fn detect(&mut self, img: &RgbaImage, precise: bool) -> Result<Vec<Det>, String> {
        let rgb = to_rgb(img);
        let mut dets = self.infer(&rgb)?;
        if !precise {
            return Ok(dets);
        }
        let flipped = imageops::flip_horizontal(&rgb);
        for det in self.infer(&flipped)? {
            let width = rgb.width() as i32;
            let box_w = (det.x1 - det.x0).max(1) as usize;
            let mask = det.mask.map(|item| flip_mask_h(item, box_w));
            dets.push(Det {
                x0: width - det.x1,
                y0: det.y0,
                x1: width - det.x0,
                y1: det.y1,
                label: det.label,
                score: det.score,
                mask,
            });
        }
        for (tx0, ty0, tx1, ty1) in tile_boxes(rgb.width(), rgb.height(), self.size as u32) {
            let crop = imageops::crop_imm(&rgb, tx0, ty0, tx1 - tx0, ty1 - ty0).to_image();
            for det in self.infer(&crop)? {
                dets.push(Det {
                    x0: det.x0 + tx0 as i32,
                    y0: det.y0 + ty0 as i32,
                    x1: det.x1 + tx0 as i32,
                    y1: det.y1 + ty0 as i32,
                    ..det
                });
            }
        }
        Ok(merge_dets(dets))
    }

    fn infer(&mut self, img: &RgbImage) -> Result<Vec<Det>, String> {
        let (width, height) = img.dimensions();
        let side = self.size.max(32) as usize;
        let (scale, _nw, _nh, left, top, blob) = letterbox(img, side);
        let tensor = ort::value::Tensor::from_array(([1usize, 3, side, side], blob))
            .map_err(|e| format!("构造输入失败：{e}"))?;
        let outputs = self
            .session
            .run(ort::inputs![self.input_name.as_str() => tensor])
            .map_err(|e| format!("检测失败：{e}"))?;
        let mut pred: Option<(Vec<i64>, Vec<f32>)> = None;
        let mut proto: Option<(Vec<i64>, Vec<f32>)> = None;
        for (_name, value) in outputs.iter() {
            let Ok((shape, data)) = value.try_extract_tensor::<f32>() else {
                continue;
            };
            let dims = shape.to_vec();
            let rank = dims.len();
            if rank == 4 && proto.is_none() {
                proto = Some((dims, data.to_vec()));
            } else if (rank == 2 || rank == 3) && pred.is_none() {
                pred = Some((dims, data.to_vec()));
            } else if rank == 3 && pred.is_some() && proto.is_none() {
                proto = Some((dims, data.to_vec()));
            }
        }
        let (pred_shape, pred_data) = pred.ok_or("模型没有检测输出")?;
        let (rows, cols, matrix) = matrix_from(&pred_shape, pred_data)?;
        let (nm, mh, mw, proto_data) = if self.seg {
            if let Some((shape, data)) = proto {
                let (nm, mh, mw) = proto_dims(&shape).unwrap_or((0, 0, 0));
                (nm, mh, mw, data)
            } else {
                (0, 0, 0, Vec::new())
            }
        } else {
            (0, 0, 0, Vec::new())
        };
        if rows == 0 || cols < 5 {
            return Ok(Vec::new());
        }
        let standard = rows < cols;
        let (count, width_c, oriented) = if standard {
            transpose(rows, cols, &matrix)
        } else {
            (rows, cols, matrix)
        };
        decode_dets(
            &oriented,
            count,
            width_c,
            standard,
            nm,
            &proto_data,
            mh,
            mw,
            side,
            scale,
            left,
            top,
            width,
            height,
            &self.labels,
        )
    }
}

fn decode_dets(
    pred: &[f32],
    count: usize,
    cols: usize,
    standard: bool,
    nm: usize,
    proto: &[f32],
    mh: usize,
    mw: usize,
    side: usize,
    scale: f32,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    labels: &[String],
) -> Result<Vec<Det>, String> {
    if count == 0 || cols == 0 || pred.len() < count * cols {
        return Ok(Vec::new());
    }
    let mut boxes = Vec::new();
    let mut scores = Vec::new();
    let mut class_ids = Vec::new();
    let mut coefs = Vec::new();
    let mut max_coord = 0f32;
    for i in 0..count {
        let row = &pred[i * cols..(i + 1) * cols];
        let (xyxy, score, class_id, coef) = if standard {
            let nc = cols.saturating_sub(4 + nm);
            if nc == 0 || row.len() < 4 + nc {
                continue;
            }
            let mut best = 0usize;
            let mut best_score = row[4];
            for c in 1..nc {
                if row[4 + c] > best_score {
                    best_score = row[4 + c];
                    best = c;
                }
            }
            if best_score < RAW_FLOOR {
                continue;
            }
            let (cx, cy, bw, bh) = (row[0], row[1], row[2], row[3]);
            let xyxy = [cx - bw / 2.0, cy - bh / 2.0, cx + bw / 2.0, cy + bh / 2.0];
            let coef = if nm > 0 && row.len() >= 4 + nc + nm {
                row[4 + nc..4 + nc + nm].to_vec()
            } else {
                Vec::new()
            };
            (xyxy, best_score, best, coef)
        } else {
            if row.len() < 6 || row[4] < RAW_FLOOR {
                continue;
            }
            let xyxy = [row[0], row[1], row[2], row[3]];
            let class_id = row[5].max(0.0) as usize;
            let coef = if nm > 0 && row.len() >= 6 + nm {
                row[6..6 + nm].to_vec()
            } else {
                Vec::new()
            };
            (xyxy, row[4], class_id, coef)
        };
        max_coord = max_coord.max(xyxy.into_iter().fold(0f32, f32::max));
        boxes.push(xyxy);
        scores.push(score);
        class_ids.push(class_id);
        coefs.push(coef);
    }
    if boxes.is_empty() {
        return Ok(Vec::new());
    }
    if max_coord <= 2.0 {
        let side_f = side as f32;
        for box_ in &mut boxes {
            for value in box_.iter_mut() {
                *value *= side_f;
            }
        }
    }
    let mut nms_boxes = boxes.clone();
    let offset = (side * 4) as f32;
    for (index, box_) in nms_boxes.iter_mut().enumerate() {
        let shift = class_ids[index] as f32 * offset;
        for value in box_.iter_mut() {
            *value += shift;
        }
    }
    let keep = if standard {
        nms(&nms_boxes, &scores, 0.5)
    } else {
        (0..boxes.len()).collect()
    };
    let mut dets = Vec::new();
    for index in keep {
        let xyxy = boxes[index];
        let x0 = ((xyxy[0] - left as f32) / scale).floor().max(0.0) as i32;
        let y0 = ((xyxy[1] - top as f32) / scale).floor().max(0.0) as i32;
        let x1 = ((xyxy[2] - left as f32) / scale).ceil().min(width as f32) as i32;
        let y1 = ((xyxy[3] - top as f32) / scale).ceil().min(height as f32) as i32;
        if x1 - x0 < 2 || y1 - y0 < 2 {
            continue;
        }
        let class_id = class_ids[index];
        let raw = labels
            .get(class_id)
            .cloned()
            .unwrap_or_else(|| format!("class{class_id}"));
        let mask = if nm > 0 && !proto.is_empty() && coefs[index].len() == nm {
            mask_crop(
                &coefs[index],
                proto,
                nm,
                mh,
                mw,
                side,
                scale,
                left,
                top,
                x0,
                y0,
                x1,
                y1,
            )
        } else {
            None
        };
        dets.push(Det {
            x0,
            y0,
            x1,
            y1,
            label: norm_label(&raw),
            score: scores[index],
            mask,
        });
    }
    Ok(dets)
}

fn mask_crop(
    coef: &[f32],
    proto: &[f32],
    nm: usize,
    mh: usize,
    mw: usize,
    side: usize,
    scale: f32,
    left: i32,
    top: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Option<Vec<u8>> {
    if mh == 0 || mw == 0 || proto.len() < nm * mh * mw {
        return None;
    }
    let hw = mh * mw;
    let mut low = vec![0f32; hw];
    for c in 0..nm {
        let plane = &proto[c * hw..(c + 1) * hw];
        let weight = coef[c];
        for (dst, src) in low.iter_mut().zip(plane) {
            *dst += weight * *src;
        }
    }
    for value in &mut low {
        *value = 1.0 / (1.0 + (-*value).exp());
    }
    let full = resize_bilinear(&low, mw, mh, side, side);
    let ix0 = ((x0 as f32 * scale) + left as f32).floor().max(0.0) as usize;
    let iy0 = ((y0 as f32 * scale) + top as f32).floor().max(0.0) as usize;
    let ix1 = ((x1 as f32 * scale) + left as f32).ceil().min(side as f32) as usize;
    let iy1 = ((y1 as f32 * scale) + top as f32).ceil().min(side as f32) as usize;
    if ix1 <= ix0 || iy1 <= iy0 {
        return None;
    }
    let mut crop = vec![0f32; (iy1 - iy0) * (ix1 - ix0)];
    for y in 0..(iy1 - iy0) {
        for x in 0..(ix1 - ix0) {
            crop[y * (ix1 - ix0) + x] = full[(iy0 + y) * side + (ix0 + x)];
        }
    }
    let bw = (x1 - x0) as usize;
    let bh = (y1 - y0) as usize;
    let resized = resize_bilinear(&crop, ix1 - ix0, iy1 - iy0, bw, bh);
    let mut mask = vec![0u8; bw * bh];
    let mut any = false;
    for (dst, src) in mask.iter_mut().zip(resized) {
        if src > 0.4 {
            *dst = 255;
            any = true;
        }
    }
    if any { Some(mask) } else { None }
}

fn matrix_from(shape: &[i64], data: Vec<f32>) -> Result<(usize, usize, Vec<f32>), String> {
    let mut dims: Vec<usize> = shape.iter().map(|dim| (*dim).max(0) as usize).collect();
    if dims.len() >= 3 && dims[0] == 1 {
        dims.remove(0);
    }
    if dims.len() != 2 {
        return Err(format!("无法解析检测输出形状 {shape:?}"));
    }
    let (rows, cols) = (dims[0], dims[1]);
    if data.len() < rows.saturating_mul(cols) {
        return Err("检测输出长度不够".into());
    }
    Ok((rows, cols, data))
}

fn transpose(rows: usize, cols: usize, data: &[f32]) -> (usize, usize, Vec<f32>) {
    let mut out = vec![0f32; rows * cols];
    for r in 0..rows {
        for c in 0..cols {
            out[c * rows + r] = data[r * cols + c];
        }
    }
    (cols, rows, out)
}

fn proto_dims(shape: &[i64]) -> Option<(usize, usize, usize)> {
    let mut dims: Vec<usize> = shape.iter().map(|dim| (*dim).max(0) as usize).collect();
    if dims.len() == 4 && dims[0] == 1 {
        dims.remove(0);
    }
    if dims.len() == 3 {
        Some((dims[0], dims[1], dims[2]))
    } else {
        None
    }
}

fn letterbox(img: &RgbImage, side: usize) -> (f32, u32, u32, i32, i32, Vec<f32>) {
    let (width, height) = img.dimensions();
    let scale = (side as f32 / height as f32).min(side as f32 / width.max(1) as f32);
    let nw = py_round(width as f32 * scale).max(1) as u32;
    let nh = py_round(height as f32 * scale).max(1) as u32;
    let left = py_round((side as f32 - nw as f32) / 2.0 - 0.1);
    let top = py_round((side as f32 - nh as f32) / 2.0 - 0.1);
        let resized = imageops::resize(img, nw, nh, imageops::FilterType::Triangle);
    let mut canvas = vec![114u8; side * side * 3];
    for y in 0..nh {
        for x in 0..nw {
            let dx = left + x as i32;
            let dy = top + y as i32;
            if dx < 0 || dy < 0 || dx as usize >= side || dy as usize >= side {
                continue;
            }
            let px = resized.get_pixel(x, y);
            let offset = (dy as usize * side + dx as usize) * 3;
            canvas[offset] = px[0];
            canvas[offset + 1] = px[1];
            canvas[offset + 2] = px[2];
        }
    }
    let plane = side * side;
    let mut blob = vec![0f32; plane * 3];
    for i in 0..plane {
        blob[i] = canvas[i * 3] as f32 / 255.0;
        blob[plane + i] = canvas[i * 3 + 1] as f32 / 255.0;
        blob[plane * 2 + i] = canvas[i * 3 + 2] as f32 / 255.0;
    }
    (scale, nw, nh, left, top, blob)
}

fn tile_boxes(width: u32, height: u32, side: u32) -> Vec<(u32, u32, u32, u32)> {
    if width.max(height) < ((side as f32) * 1.4) as u32 {
        return Vec::new();
    }
    let axis = |n_len: u32| -> Vec<(u32, u32)> {
        let n = if (n_len as f32) < 1.2 * side as f32 {
            1
        } else if (n_len as f32) < 2.6 * side as f32 {
            2
        } else {
            3
        };
        if n == 1 {
            return vec![(0, n_len)];
        }
        let size = n_len.min(((n_len as f32 / n as f32) * 1.3).ceil() as u32).max(1);
        let step = (n_len.saturating_sub(size)) as f32 / (n - 1) as f32;
        (0..n)
            .map(|i| {
                let start = (i as f32 * step) as u32;
                let end = (start + size).min(n_len).max(start + 1);
                (start, end.min(n_len))
            })
            .collect()
    };
    let mut tiles = Vec::new();
    for (y0, y1) in axis(height) {
        for (x0, x1) in axis(width) {
            if x1 > x0 && y1 > y0 {
                tiles.push((x0, y0, x1, y1));
            }
        }
    }
    tiles
}

fn flip_mask_h(mask: Vec<u8>, width: usize) -> Vec<u8> {
    if mask.is_empty() || width == 0 || mask.len() % width != 0 {
        return mask;
    }
    let height = mask.len() / width;
    let mut out = vec![0u8; mask.len()];
    for y in 0..height {
        for x in 0..width {
            out[y * width + x] = mask[y * width + (width - 1 - x)];
        }
    }
    out
}

fn nms(boxes: &[[f32; 4]], scores: &[f32], thr: f32) -> Vec<usize> {
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap_or(std::cmp::Ordering::Equal));
    let mut keep = Vec::new();
    let mut dead = vec![false; boxes.len()];
    for i in 0..order.len() {
        let a = order[i];
        if dead[a] {
            continue;
        }
        keep.push(a);
        for b in order.iter().skip(i + 1).copied() {
            if !dead[b] && iou(boxes[a], boxes[b]) > thr {
                dead[b] = true;
            }
        }
    }
    keep
}

fn iou(a: [f32; 4], b: [f32; 4]) -> f32 {
    let ix0 = a[0].max(b[0]);
    let iy0 = a[1].max(b[1]);
    let ix1 = a[2].min(b[2]);
    let iy1 = a[3].min(b[3]);
    let inter = (ix1 - ix0).max(0.0) * (iy1 - iy0).max(0.0);
    if inter <= 0.0 {
        return 0.0;
    }
    let area_a = (a[2] - a[0]).max(0.0) * (a[3] - a[1]).max(0.0);
    let area_b = (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
    inter / (area_a + area_b - inter).max(1.0)
}

fn merge_dets(mut dets: Vec<Det>) -> Vec<Det> {
    dets.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<Det> = Vec::new();
    for det in dets {
        if let Some(index) = out.iter().position(|item| item.label == det.label && iou(det_box(item), det_box(&det)) > 0.45) {
            let current = out[index].clone();
            out[index] = union_det(current, det);
        } else {
            out.push(det);
        }
    }
    out
}

fn det_box(det: &Det) -> [f32; 4] {
    [det.x0 as f32, det.y0 as f32, det.x1 as f32, det.y1 as f32]
}

fn union_det(a: Det, b: Det) -> Det {
    let x0 = a.x0.min(b.x0);
    let y0 = a.y0.min(b.y0);
    let x1 = a.x1.max(b.x1);
    let y1 = a.y1.max(b.y1);
    let bw = (x1 - x0).max(0) as usize;
    let bh = (y1 - y0).max(0) as usize;
    let mask = if bw > 0 && bh > 0 && (a.mask.is_some() || b.mask.is_some()) {
        let mut mask = vec![0u8; bw * bh];
        blit_mask(&mut mask, bw, x0, y0, &a);
        blit_mask(&mut mask, bw, x0, y0, &b);
        Some(mask)
    } else {
        None
    };
    let (label, score) = if a.score >= b.score {
        (a.label, a.score)
    } else {
        (b.label, b.score)
    };
    Det {
        x0,
        y0,
        x1,
        y1,
        label,
        score,
        mask,
    }
}

fn blit_mask(dest: &mut [u8], dest_w: usize, ox: i32, oy: i32, det: &Det) {
    let bw = (det.x1 - det.x0).max(0) as usize;
    let bh = (det.y1 - det.y0).max(0) as usize;
    for y in 0..bh {
        for x in 0..bw {
            let dx = det.x0 - ox + x as i32;
            let dy = det.y0 - oy + y as i32;
            if dx < 0 || dy < 0 {
                continue;
            }
            let di = dy as usize * dest_w + dx as usize;
            if di >= dest.len() {
                continue;
            }
            let on = match &det.mask {
                Some(mask) if mask.len() == bw * bh => mask[y * bw + x] > 0,
                _ => true,
            };
            if on {
                dest[di] = 255;
            }
        }
    }
}

fn collect_faces(raws: &[Vec<Det>], width: u32, height: u32) -> Vec<[i32; 4]> {
    let mut faces = Vec::new();
    for raw in raws {
        for det in raw {
            if det.label == "face" && det.score >= FACE_CONF {
                let ex = ((det.x1 - det.x0) as f32 * 0.1) as i32;
                let ey = ((det.y1 - det.y0) as f32 * 0.1) as i32;
                faces.push([
                    (det.x0 - ex).max(0),
                    (det.y0 - ey).max(0),
                    (det.x1 + ex).min(width as i32),
                    (det.y1 + ey).min(height as i32),
                ]);
            }
        }
    }
    faces
}

fn face_guard(dets: Vec<Det>, faces: &[[i32; 4]], guard_male: bool) -> (Vec<Det>, usize) {
    if faces.is_empty() {
        return (dets, 0);
    }
    let mut kept = Vec::new();
    let mut dropped = 0;
    for det in dets {
        let guarded = FACE_GUARD_LABELS.contains(&det.label.as_str()) || (guard_male && det.label == "male_genital");
        let covered = guarded
            && faces.iter().any(|face| ioa([det.x0, det.y0, det.x1, det.y1], *face) > 0.5);
        if covered {
            dropped += 1;
        } else {
            kept.push(det);
        }
    }
    (kept, dropped)
}

fn ioa(inner: [i32; 4], outer: [i32; 4]) -> f32 {
    let ix0 = inner[0].max(outer[0]);
    let iy0 = inner[1].max(outer[1]);
    let ix1 = inner[2].min(outer[2]);
    let iy1 = inner[3].min(outer[3]);
    let inter = (ix1 - ix0).max(0) * (iy1 - iy0).max(0);
    let area = (inner[2] - inner[0]).max(1) * (inner[3] - inner[1]).max(1);
    inter as f32 / area as f32
}

fn build_fit_mask(img: &RgbaImage, dets: &[Det]) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut total = vec![0u8; (w as usize) * (h as usize)];
    for det in dets {
        if let Some(mask) = &det.mask {
            paste_mask(&mut total, w, h, det, mask);
        } else {
            paint_color_fit(&mut total, img, det);
        }
    }
    total
}

fn paint_color_fit(total: &mut [u8], img: &RgbaImage, det: &Det) {
    let (w, h) = img.dimensions();
    let x0 = det.x0.clamp(0, w as i32);
    let y0 = det.y0.clamp(0, h as i32);
    let x1 = det.x1.clamp(0, w as i32);
    let y1 = det.y1.clamp(0, h as i32);
    let bw = (x1 - x0) as usize;
    let bh = (y1 - y0) as usize;
    if bw < 12 || bh < 12 {
        fill_ellipse(total, w, h, det.x0, det.y0, det.x1, det.y1);
        return;
    }
    let (fg, bg) = sample_fit_colors(img, x0, y0, x1, y1);
    let cx = (x0 + x1) as f32 / 2.0;
    let cy = (y0 + y1) as f32 / 2.0;
    let rx = ((x1 - x0) as f32 / 2.0).max(1.0);
    let ry = ((y1 - y0) as f32 / 2.0).max(1.0);
    let mut bin = vec![0u8; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let px = img.get_pixel(x0 as u32 + x as u32, y0 as u32 + y as u32).0;
            let df = color_dist(px, fg);
            let db = color_dist(px, bg);
            let nx = (x0 as f32 + x as f32 + 0.5 - cx) / rx;
            let ny = (y0 as f32 + y as f32 + 0.5 - cy) / ry;
            let spatial = (nx * nx + ny * ny).sqrt();
            if df + 12.0 < db && df < 120.0 && spatial < 1.02 {
                bin[y * bw + x] = 255;
            }
        }
    }
    keep_main_component(&mut bin, bw, bh);
    fill_holes(&mut bin, bw, bh);
    let count = bin.iter().filter(|px| **px > 0).count();
    if count * 8 < bw * bh {
        fill_ellipse(total, w, h, det.x0, det.y0, det.x1, det.y1);
        return;
    }
    for y in 0..bh {
        for x in 0..bw {
            if bin[y * bw + x] == 0 {
                continue;
            }
            total[(y0 as usize + y) * w as usize + (x0 as usize + x)] = 255;
        }
    }
}

fn sample_fit_colors(img: &RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32) -> ([f32; 3], [f32; 3]) {
    let (w, h) = img.dimensions();
    let cx = (x0 + x1) as f32 / 2.0;
    let cy = (y0 + y1) as f32 / 2.0;
    let rx = ((x1 - x0) as f32 / 2.0).max(1.0);
    let ry = ((y1 - y0) as f32 / 2.0).max(1.0);
    let mut fg = [0f32; 3];
    let mut bg = [0f32; 3];
    let mut fg_n = 0f32;
    let mut bg_n = 0f32;
    let xa = (x0 - 4).max(0);
    let ya = (y0 - 4).max(0);
    let xb = (x1 + 4).min(w as i32);
    let yb = (y1 + 4).min(h as i32);
    for y in ya..yb {
        for x in xa..xb {
            let px = img.get_pixel(x as u32, y as u32).0;
            let nx = (x as f32 + 0.5 - cx) / rx;
            let ny = (y as f32 + 0.5 - cy) / ry;
            let inside = x >= x0 && y >= y0 && x < x1 && y < y1;
            let radial = nx * nx + ny * ny;
            if inside && radial <= 0.22 {
                fg[0] += px[0] as f32;
                fg[1] += px[1] as f32;
                fg[2] += px[2] as f32;
                fg_n += 1.0;
            } else if !inside {
                bg[0] += px[0] as f32;
                bg[1] += px[1] as f32;
                bg[2] += px[2] as f32;
                bg_n += 1.0;
            }
        }
    }
    if fg_n < 1.0 {
        fg = [128.0, 128.0, 128.0];
        fg_n = 1.0;
    }
    if bg_n < 1.0 {
        bg = [0.0, 0.0, 0.0];
        bg_n = 1.0;
    }
    ([fg[0] / fg_n, fg[1] / fg_n, fg[2] / fg_n], [bg[0] / bg_n, bg[1] / bg_n, bg[2] / bg_n])
}

fn color_dist(px: [u8; 4], mean: [f32; 3]) -> f32 {
    let dr = px[0] as f32 - mean[0];
    let dg = px[1] as f32 - mean[1];
    let db = px[2] as f32 - mean[2];
    (dr * dr + dg * dg + db * db).sqrt()
}

fn keep_main_component(bin: &mut [u8], bw: usize, bh: usize) {
    if bw == 0 || bh == 0 {
        return;
    }
    let mut labels = vec![0u32; bw * bh];
    let mut sizes = vec![0u32];
    let mut center_hits = vec![0u32];
    let cx = bw / 2;
    let cy = bh / 2;
    let mut next = 1u32;
    let mut stack = Vec::new();
    for start in 0..bin.len() {
        if bin[start] == 0 || labels[start] != 0 {
            continue;
        }
        labels[start] = next;
        stack.push(start);
        let mut size = 0u32;
        let mut hits = 0u32;
        while let Some(cur) = stack.pop() {
            size += 1;
            let y = cur / bw;
            let x = cur % bw;
            if x.abs_diff(cx) * 2 < bw && y.abs_diff(cy) * 2 < bh {
                hits += 1;
            }
            for (dx, dy) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= bw as i32 || ny >= bh as i32 {
                    continue;
                }
                let ni = ny as usize * bw + nx as usize;
                if bin[ni] > 0 && labels[ni] == 0 {
                    labels[ni] = next;
                    stack.push(ni);
                }
            }
        }
        sizes.push(size);
        center_hits.push(hits);
        next += 1;
    }
    if next <= 2 {
        return;
    }
    let mut best = 1u32;
    for id in 2..next {
        let better = center_hits[id as usize] > center_hits[best as usize]
            || (center_hits[id as usize] == center_hits[best as usize] && sizes[id as usize] > sizes[best as usize]);
        if better {
            best = id;
        }
    }
    for (px, label) in bin.iter_mut().zip(labels) {
        if label != best {
            *px = 0;
        }
    }
}

fn fill_holes(bin: &mut [u8], bw: usize, bh: usize) {
    if bw == 0 || bh == 0 {
        return;
    }
    let mut outside = vec![false; bw * bh];
    let mut stack = Vec::new();
    let seed = |i: usize, bin: &[u8], outside: &mut [bool], stack: &mut Vec<usize>| {
        if bin[i] == 0 && !outside[i] {
            outside[i] = true;
            stack.push(i);
        }
    };
    for x in 0..bw {
        seed(x, bin, &mut outside, &mut stack);
        seed((bh - 1) * bw + x, bin, &mut outside, &mut stack);
    }
    for y in 0..bh {
        seed(y * bw, bin, &mut outside, &mut stack);
        seed(y * bw + bw - 1, bin, &mut outside, &mut stack);
    }
    while let Some(cur) = stack.pop() {
        let y = cur / bw;
        let x = cur % bw;
        for (dx, dy) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= bw as i32 || ny >= bh as i32 {
                continue;
            }
            let ni = ny as usize * bw + nx as usize;
            if bin[ni] == 0 && !outside[ni] {
                outside[ni] = true;
                stack.push(ni);
            }
        }
    }
    for (px, seen) in bin.iter_mut().zip(outside) {
        if *px == 0 && !seen {
            *px = 255;
        }
    }
}

fn build_mask(img: &RgbaImage, dets: &[Det], shape: &str) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut total = vec![0u8; (w as usize) * (h as usize)];
    for det in dets {
        if shape == "rect" {
            fill_rect(&mut total, w, h, det.x0, det.y0, det.x1, det.y1);
        } else if shape == "ellipse" {
            fill_ellipse(&mut total, w, h, det.x0, det.y0, det.x1, det.y1);
        } else if let Some(mask) = &det.mask {
            paste_mask(&mut total, w, h, det, mask);
        } else {
            fill_ellipse(&mut total, w, h, det.x0, det.y0, det.x1, det.y1);
        }
    }
    total
}

fn paste_mask(total: &mut [u8], w: u32, h: u32, det: &Det, mask: &[u8]) {
    let bw = (det.x1 - det.x0).max(0) as usize;
    let bh = (det.y1 - det.y0).max(0) as usize;
    if bw == 0 || bh == 0 || mask.len() != bw * bh {
        fill_ellipse(total, w, h, det.x0, det.y0, det.x1, det.y1);
        return;
    }
    for y in 0..bh {
        for x in 0..bw {
            if mask[y * bw + x] == 0 {
                continue;
            }
            let dx = det.x0 + x as i32;
            let dy = det.y0 + y as i32;
            if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                continue;
            }
            total[dy as usize * w as usize + dx as usize] = 255;
        }
    }
}

fn fill_rect(mask: &mut [u8], w: u32, h: u32, x0: i32, y0: i32, x1: i32, y1: i32) {
    let x0 = x0.clamp(0, w as i32) as usize;
    let y0 = y0.clamp(0, h as i32) as usize;
    let x1 = x1.clamp(0, w as i32) as usize;
    let y1 = y1.clamp(0, h as i32) as usize;
    for y in y0..y1 {
        for x in x0..x1 {
            mask[y * w as usize + x] = 255;
        }
    }
}

fn fill_ellipse(mask: &mut [u8], w: u32, h: u32, x0: i32, y0: i32, x1: i32, y1: i32) {
    let cx = (x0 + x1) as f32 / 2.0;
    let cy = (y0 + y1) as f32 / 2.0;
    let rx = ((x1 - x0) as f32 / 2.0).max(1.0);
    let ry = ((y1 - y0) as f32 / 2.0).max(1.0);
    let xa = x0.clamp(0, w as i32) as usize;
    let ya = y0.clamp(0, h as i32) as usize;
    let xb = x1.clamp(0, w as i32) as usize;
    let yb = y1.clamp(0, h as i32) as usize;
    for y in ya..yb {
        for x in xa..xb {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                mask[y * w as usize + x] = 255;
            }
        }
    }
}

fn dilate_mask(mask: &[u8], w: u32, h: u32, radius: u32) -> Vec<u8> {
    if radius == 0 {
        return mask.to_vec();
    }
    let r = radius as i32;
    let offsets = circle_offsets(r);
    let mut out = mask.to_vec();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if mask[y as usize * w as usize + x as usize] == 0 {
                continue;
            }
            for (dx, dy) in &offsets {
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                out[ny as usize * w as usize + nx as usize] = 255;
            }
        }
    }
    out
}

fn circle_offsets(radius: i32) -> Vec<(i32, i32)> {
    let mut offsets = Vec::new();
    let limit = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= limit {
                offsets.push((dx, dy));
            }
        }
    }
    offsets
}

fn block_size(w: u32, h: u32, strength: u32, min_block: u32) -> u32 {
    let strength = strength.max(1) as f32;
    let long = w.max(h) as f32;
    let grain = py_round(long / strength).max(1) as u32;
    grain.max(min_block.max(1))
}

fn paint(img: &mut RgbaImage, mask: &[u8], block: u32, mode: &str) {
    let (w, h) = img.dimensions();
    let Some((x, y, bw, bh)) = mask_bounds(mask, w, h) else {
        return;
    };
    let block = block.max(1);
    let x0 = (x / block) * block;
    let y0 = (y / block) * block;
    let x1 = ((x + bw + block - 1) / block * block).min(w);
    let y1 = ((y + bh + block - 1) / block * block).min(h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let rw = x1 - x0;
    let rh = y1 - y0;
    let mut region = vec![0u8; (rw * rh * 3) as usize];
    for row in 0..rh {
        for col in 0..rw {
            let px = img.get_pixel(x0 + col, y0 + row).0;
            let offset = ((row * rw + col) * 3) as usize;
            region[offset] = px[0];
            region[offset + 1] = px[1];
            region[offset + 2] = px[2];
        }
    }
    let censored = if mode == "blur" {
        box_blur(&region, rw, rh, block)
    } else {
        pixelate(&region, rw, rh, block)
    };
    for row in 0..rh {
        for col in 0..rw {
            let mi = ((y0 + row) * w + (x0 + col)) as usize;
            if mask.get(mi).copied().unwrap_or(0) == 0 {
                continue;
            }
            let offset = ((row * rw + col) * 3) as usize;
            let px = img.get_pixel_mut(x0 + col, y0 + row);
            px[0] = censored[offset];
            px[1] = censored[offset + 1];
            px[2] = censored[offset + 2];
        }
    }
}

fn mask_bounds(mask: &[u8], w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for y in 0..h {
        for x in 0..w {
            if mask.get((y * w + x) as usize).copied().unwrap_or(0) == 0 {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
        }
    }
    if max_x <= min_x || max_y <= min_y {
        None
    } else {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    }
}

fn pixelate(region: &[u8], w: u32, h: u32, block: u32) -> Vec<u8> {
    let block = block.max(1);
    let bw = (w + block - 1) / block;
    let bh = (h + block - 1) / block;
    let mut small = vec![0u32; (bw * bh * 3) as usize];
    let mut count = vec![0u32; (bw * bh) as usize];
    for y in 0..h {
        for x in 0..w {
            let sx = x / block;
            let sy = y / block;
            let si = (sy * bw + sx) as usize;
            let oi = ((y * w + x) * 3) as usize;
            small[si * 3] += region[oi] as u32;
            small[si * 3 + 1] += region[oi + 1] as u32;
            small[si * 3 + 2] += region[oi + 2] as u32;
            count[si] += 1;
        }
    }
    let mut out = vec![0u8; region.len()];
    for y in 0..h {
        for x in 0..w {
            let si = ((y / block) * bw + (x / block)) as usize;
            let n = count[si].max(1);
            let oi = ((y * w + x) * 3) as usize;
            out[oi] = (small[si * 3] / n) as u8;
            out[oi + 1] = (small[si * 3 + 1] / n) as u8;
            out[oi + 2] = (small[si * 3 + 2] / n) as u8;
        }
    }
    out
}

fn box_blur(region: &[u8], w: u32, h: u32, radius: u32) -> Vec<u8> {
    let radius = radius.max(1);
    let mut tmp = vec![0u8; region.len()];
    let mut out = vec![0u8; region.len()];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius).min(w - 1);
            for sx in x0..=x1 {
                let oi = ((y * w + sx) * 3) as usize;
                acc[0] += region[oi] as u32;
                acc[1] += region[oi + 1] as u32;
                acc[2] += region[oi + 2] as u32;
                n += 1;
            }
            let oi = ((y * w + x) * 3) as usize;
            tmp[oi] = (acc[0] / n) as u8;
            tmp[oi + 1] = (acc[1] / n) as u8;
            tmp[oi + 2] = (acc[2] / n) as u8;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius).min(h - 1);
            for sy in y0..=y1 {
                let oi = ((sy * w + x) * 3) as usize;
                acc[0] += tmp[oi] as u32;
                acc[1] += tmp[oi + 1] as u32;
                acc[2] += tmp[oi + 2] as u32;
                n += 1;
            }
            let oi = ((y * w + x) * 3) as usize;
            out[oi] = (acc[0] / n) as u8;
            out[oi + 1] = (acc[1] / n) as u8;
            out[oi + 2] = (acc[2] / n) as u8;
        }
    }
    out
}

fn to_rgb(img: &RgbaImage) -> RgbImage {
    let mut rgb = RgbImage::new(img.width(), img.height());
    for (dst, src) in rgb.pixels_mut().zip(img.pixels()) {
        *dst = Rgb([src[0], src[1], src[2]]);
    }
    rgb
}

fn resize_bilinear(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return vec![0.0; dw * dh];
    }
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    let mut out = vec![0f32; dw * dh];
    for y in 0..dh {
        let fy = (y as f32 + 0.5) * sh as f32 / dh as f32 - 0.5;
        let y0 = fy.floor().max(0.0) as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let ty = (fy - y0 as f32).clamp(0.0, 1.0);
        for x in 0..dw {
            let fx = (x as f32 + 0.5) * sw as f32 / dw as f32 - 0.5;
            let x0 = fx.floor().max(0.0) as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let tx = (fx - x0 as f32).clamp(0.0, 1.0);
            let p00 = src[y0 * sw + x0];
            let p10 = src[y0 * sw + x1];
            let p01 = src[y1 * sw + x0];
            let p11 = src[y1 * sw + x1];
            let top = p00 + (p10 - p00) * tx;
            let bot = p01 + (p11 - p01) * tx;
            out[y * dw + x] = top + (bot - top) * ty;
        }
    }
    out
}

fn py_round(value: f32) -> i32 {
    let floor = value.floor();
    let diff = value - floor;
    if (diff - 0.5).abs() < 1e-4 {
        let even = floor as i32;
        if even % 2 == 0 { even } else { even + 1 }
    } else {
        value.round() as i32
    }
}

fn norm_label(label: &str) -> String {
    let key = label.trim().to_ascii_lowercase().replace(' ', "_");
    match key.as_str() {
        "pussy" | "vagina" | "female_genitalia_exposed" => "female_genital",
        "penis" | "testicles" | "male_genitalia_exposed" => "male_genital",
        "anus" | "anus_exposed" => "anus",
        "nipple_f" | "nipple" | "nipples" => "nipple",
        "x-ray" | "xray" | "cross-section" | "cross_section" => "xray",
        "female_genitalia_covered" => "female_genital_covered",
        "anus_covered" => "anus_covered",
        "female_breast_exposed" => "breast",
        "buttocks_exposed" => "buttocks",
        "make_love" => "sex",
        "face" | "face_female" | "face_male" => "face",
        _ => return key,
    }
    .to_string()
}

fn parse_class_names(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    if let Ok(list) = serde_json::from_str::<Vec<String>>(raw) {
        return list;
    }
    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(raw) {
        let mut pairs: Vec<(i64, String)> = map
            .into_iter()
            .filter_map(|(key, value)| {
                let name = value.as_str()?.to_string();
                Some((key.parse().unwrap_or(0), name))
            })
            .collect();
        pairs.sort_by_key(|(index, _)| *index);
        if !pairs.is_empty() {
            return pairs.into_iter().map(|(_, name)| name).collect();
        }
    }
    let mut names = Vec::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let quote = bytes[i];
        if quote == b'\'' || quote == b'"' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            if let Ok(text) = std::str::from_utf8(&bytes[start..i]) {
                names.push(text.to_string());
            }
        }
        i += 1;
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn maps_labels_like_the_desktop_tool() {
        assert_eq!(norm_label("Pussy"), "female_genital");
        assert_eq!(norm_label("FACE_FEMALE"), "face");
        assert_eq!(norm_label("X-Ray"), "xray");
        assert_eq!(norm_label("male_genitalia_exposed"), "male_genital");
    }

    #[test]
    fn parses_ultralytics_names() {
        assert_eq!(parse_class_names("{0: 'pussy', 1: 'penis'}"), vec!["pussy", "penis"]);
        assert_eq!(parse_class_names(r#"["anus","nipple"]"#), vec!["anus", "nipple"]);
    }

    #[test]
    fn letterbox_matches_python_rounding() {
        let img = RgbImage::from_pixel(100, 50, Rgb([1, 2, 3]));
        let (scale, nw, nh, left, top, blob) = letterbox(&img, 640);
        assert!((scale - 6.4).abs() < 1e-4);
        assert_eq!((nw, nh, left, top), (640, 320, 0, 160));
        assert_eq!(blob.len(), 3 * 640 * 640);
    }

    #[test]
    fn nms_keeps_distinct_boxes() {
        let boxes = [[0.0, 0.0, 10.0, 10.0], [0.0, 0.0, 10.0, 10.0], [20.0, 20.0, 30.0, 30.0]];
        let keep = nms(&boxes, &[0.9, 0.4, 0.8], 0.5);
        assert_eq!(keep, vec![0, 2]);
    }

    #[test]
    fn face_guard_keeps_male_by_default() {
        let face = [[0, 0, 100, 100]];
        let dets = vec![
            Det { x0: 10, y0: 10, x1: 40, y1: 40, label: "female_genital".into(), score: 0.9, mask: None },
            Det { x0: 10, y0: 10, x1: 40, y1: 40, label: "male_genital".into(), score: 0.9, mask: None },
        ];
        let (kept, dropped) = face_guard(dets, &face, false);
        assert_eq!(dropped, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].label, "male_genital");
    }

    #[test]
    fn mosaic_leaves_pixels_outside_the_mask() {
        let mut img = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
        let mut mask = vec![0u8; 64];
        mask[0] = 255;
        paint(&mut img, &mask, 4, "mosaic");
        assert_eq!(img.get_pixel(7, 7).0, [10, 20, 30, 255]);
        assert_ne!(img.get_pixel(0, 0).0, [200, 0, 0, 255]);
    }

    #[test]
    fn ellipse_does_not_fill_the_box_corner() {
        let mut mask = vec![0u8; 20 * 20];
        fill_ellipse(&mut mask, 20, 20, 0, 0, 20, 20);
        assert_eq!(mask[0], 0);
        assert_eq!(mask[10 * 20 + 10], 255);
    }

    #[test]
    fn dilate_uses_a_disk() {
        let mut mask = vec![0u8; 11 * 11];
        mask[5 * 11 + 5] = 255;
        let grown = dilate_mask(&mask, 11, 11, 2);
        assert_eq!(grown[5 * 11 + 7], 255);
        assert_eq!(grown[5 * 11 + 8], 0);
    }

    #[test]
    fn strength_200_on_a_tall_image_uses_the_min_block() {
        assert_eq!(block_size(832, 1216, 200, 6), 6);
        assert_eq!(block_size(1024, 1024, 100, 8), 10);
    }

    #[test]
    fn color_fit_follows_the_foreground_strip() {
        let mut img = RgbaImage::from_pixel(40, 60, Rgba([20, 40, 180, 255]));
        for y in 6..54 {
            for x in 16..24 {
                img.put_pixel(x, y, Rgba([220, 80, 60, 255]));
            }
        }
        let det = Det {
            x0: 4,
            y0: 4,
            x1: 36,
            y1: 56,
            label: "male_genital".into(),
            score: 0.9,
            mask: None,
        };
        let mut total = vec![0u8; 40 * 60];
        paint_color_fit(&mut total, &img, &det);
        assert_eq!(total[30 * 40 + 20], 255);
        assert_eq!(total[10 * 40 + 8], 0);
    }

    #[test]
    fn fit_mask_does_not_cover_a_box_already_inside_a_contour() {
        let img = RgbaImage::new(40, 40);
        let contour = vec![255u8; 10 * 10];
        let dets = vec![
            Det { x0: 15, y0: 15, x1: 25, y1: 25, label: "female_genital".into(), score: 0.9, mask: Some(contour) },
            Det { x0: 0, y0: 0, x1: 40, y1: 40, label: "female_genital".into(), score: 0.8, mask: None },
        ];
        let mask = build_fit_mask(&img, &dets);
        assert_eq!(mask[20 * 40 + 20], 255);
        assert_eq!(mask[2 * 40 + 2], 0);
    }
}
