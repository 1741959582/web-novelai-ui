use crate::store::default_output_dir;
use base64::Engine;
use flate2::{write::ZlibEncoder, Compression};
use image::{imageops, ImageEncoder, Rgba, RgbaImage};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
const MAX_SIDE: u32 = 1600;
const IDAT_SPLIT: usize = 65536;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedImage {
    pub path: String,
    pub data_url: String,
}

fn decode_data_url(input: &str) -> Result<RgbaImage, String> {
    let raw = if let Some((_, b64)) = input.split_once("base64,") {
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| format!("图片解码失败：{e}"))?
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(input.trim())
            .or_else(|_| fs::read(input.trim()))
            .map_err(|e| format!("读取图片失败：{e}"))?
    };
    let img = image::load_from_memory(&raw).map_err(|e| format!("解析图片失败：{e}"))?;
    Ok(strip_metadata(img.to_rgba8()))
}

fn strip_metadata(mut img: RgbaImage) -> RgbaImage {
    for px in img.pixels_mut() {
        px.0[0] &= 0xFE;
        px.0[1] &= 0xFE;
        px.0[2] &= 0xFE;
        px.0[3] = if px.0[3] < 16 { 0 } else { 255 };
        if px.0[3] == 0 {
            px.0[0] = 255;
            px.0[1] = 255;
            px.0[2] = 255;
        }
    }
    img
}

fn is_knockout_bg(px: [u8; 4], white_thr: u8, allow_black: bool) -> bool {
    let [r, g, b, a] = px;
    if a < 16 {
        return true;
    }
    if r >= white_thr && g >= white_thr && b >= white_thr {
        return true;
    }
    allow_black && r <= 8 && g <= 8 && b <= 8
}

fn knockout_edge(mut img: RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w < 2 || h < 2 {
        return img;
    }
    let corners = [
        img.get_pixel(0, 0).0,
        img.get_pixel(w - 1, 0).0,
        img.get_pixel(0, h - 1).0,
        img.get_pixel(w - 1, h - 1).0,
    ];
    if !corners.iter().any(|c| is_knockout_bg(*c, 248, true)) {
        return img;
    }
    let allow_black = corners.iter().any(|c| c[0] <= 8 && (c[3] < 16 || (c[1] <= 8 && c[2] <= 8)));
    let mut visited = vec![0u8; (w as usize) * (h as usize)];
    let mut q = VecDeque::new();
    let push = |img: &RgbaImage, visited: &mut [u8], q: &mut VecDeque<(u32, u32)>, x: u32, y: u32| {
        let i = y as usize * w as usize + x as usize;
        if visited[i] != 0 {
            return;
        }
        if !is_knockout_bg(img.get_pixel(x, y).0, 248, allow_black) {
            return;
        }
        visited[i] = 1;
        q.push_back((x, y));
    };
    for x in 0..w {
        push(&img, &mut visited, &mut q, x, 0);
        push(&img, &mut visited, &mut q, x, h - 1);
    }
    for y in 0..h {
        push(&img, &mut visited, &mut q, 0, y);
        push(&img, &mut visited, &mut q, w - 1, y);
    }
    while let Some((x, y)) = q.pop_front() {
        img.put_pixel(x, y, Rgba([255, 255, 255, 0]));
        if x > 0 {
            push(&img, &mut visited, &mut q, x - 1, y);
        }
        if x + 1 < w {
            push(&img, &mut visited, &mut q, x + 1, y);
        }
        if y > 0 {
            push(&img, &mut visited, &mut q, x, y - 1);
        }
        if y + 1 < h {
            push(&img, &mut visited, &mut q, x, y + 1);
        }
    }
    img
}

fn data_url_png(img: &RgbaImage) -> Result<String, String> {
    let bytes = encode_png(img)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn sanitize_filename(raw: &str, ext: &str) -> String {
    let base = raw.trim().rsplit(['/', '\\']).next().unwrap_or(raw.trim());
    let stem = base
        .trim_end_matches(".png")
        .trim_end_matches(".PNG")
        .trim_end_matches(".gif")
        .trim_end_matches(".GIF");
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == ' ' || c == '.' || c == '_');
    let stem = if cleaned.is_empty() { "image" } else { cleaned };
    format!("{stem}{ext}")
}

fn parse_color(value: &str) -> [u8; 4] {
    let raw = value.trim().trim_start_matches('#');
    let hex = if raw.len() == 3 {
        raw.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        raw.to_string()
    };
    if hex.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return [r, g, b, 255];
        }
    }
    [255, 255, 255, 255]
}

fn limit_side(img: RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let m = w.max(h);
    if m <= MAX_SIDE {
        return img;
    }
    let scale = MAX_SIDE as f32 / m as f32;
    imageops::resize(
        &img,
        (w as f32 * scale).round().max(1.0) as u32,
        (h as f32 * scale).round().max(1.0) as u32,
        imageops::FilterType::Triangle,
    )
}

fn fit_contain(img: &RgbaImage, tw: u32, th: u32, bg: [u8; 4]) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    let mut canvas = RgbaImage::from_pixel(tw, th, Rgba(bg));
    if iw == 0 || ih == 0 {
        return canvas;
    }
    let scale = (tw as f32 / iw as f32).min(th as f32 / ih as f32);
    let nw = (iw as f32 * scale).round().max(1.0) as u32;
    let nh = (ih as f32 * scale).round().max(1.0) as u32;
    let resized = imageops::resize(img, nw, nh, imageops::FilterType::Triangle);
    imageops::overlay(&mut canvas, &resized, ((tw - nw) / 2) as i64, ((th - nh) / 2) as i64);
    canvas
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12 + data.len());
    buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
    buf.extend_from_slice(kind);
    buf.extend_from_slice(data);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(kind);
    hasher.update(data);
    buf.extend_from_slice(&hasher.finalize().to_be_bytes());
    buf
}

fn filter_rows(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let row = w as usize * 4;
    let mut out = vec![0u8; (row + 1) * h as usize];
    let raw = img.as_raw();
    for y in 0..h as usize {
        let o = y * (row + 1);
        out[o] = 0;
        out[o + 1..o + 1 + row].copy_from_slice(&raw[y * row..(y + 1) * row]);
    }
    out
}

fn zlib_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(6));
    enc.write_all(data).map_err(|e| e.to_string())?;
    enc.finish().map_err(|e| e.to_string())
}

fn split_chunks(kind: &[u8; 4], data: &[u8], with_seq: bool, mut seq: u32) -> (Vec<Vec<u8>>, u32) {
    let mut parts = Vec::new();
    let src = if data.is_empty() { &[0u8; 0][..] } else { data };
    let mut i = 0;
    while i < src.len() || parts.is_empty() {
        let piece = &src[i..(i + IDAT_SPLIT).min(src.len())];
        let payload = if with_seq {
            let mut v = seq.to_be_bytes().to_vec();
            v.extend_from_slice(piece);
            seq += 1;
            v
        } else {
            piece.to_vec()
        };
        parts.push(png_chunk(kind, &payload));
        if src.is_empty() {
            break;
        }
        i += IDAT_SPLIT;
        if i >= src.len() {
            break;
        }
    }
    (parts, seq)
}

fn write_apng(cover: &RgbaImage, frames: &[RgbaImage], delay_num: u16, delay_den: u16) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("至少需要 1 张真图".into());
    }
    let (w, h) = frames[0].dimensions();
    let cover = if cover.dimensions() == (w, h) {
        cover.clone()
    } else {
        imageops::resize(cover, w, h, imageops::FilterType::Nearest)
    };
    let cover_z = zlib_bytes(&filter_rows(&cover))?;
    let frame_z = frames
        .iter()
        .map(|f| {
            if f.dimensions() != (w, h) {
                return Err("动画帧尺寸必须一致".into());
            }
            zlib_bytes(&filter_rows(f))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let use_keepalive = frames.len() == 1;
    let num_anim = frames.len() as u32 + u32::from(use_keepalive);
    let kind: &[u8] = if frames.len() > 1 { b"ANIMATED" } else { b"STATIC" };
    let marker = [
        b"ChatBarApngDisguise\x001;".as_slice(),
        kind,
        b";",
        frames.len().to_string().as_bytes(),
    ]
    .concat();
    let mut chunks = vec![
        PNG_SIG.to_vec(),
        png_chunk(b"IHDR", &{
            let mut ihdr = Vec::with_capacity(13);
            ihdr.extend_from_slice(&w.to_be_bytes());
            ihdr.extend_from_slice(&h.to_be_bytes());
            ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
            ihdr
        }),
        png_chunk(b"acTL", &[num_anim.to_be_bytes(), 0u32.to_be_bytes()].concat()),
        png_chunk(b"tEXt", &marker),
    ];
    chunks.extend(split_chunks(b"IDAT", &cover_z, false, 0).0);
    let mut seq = 0u32;
    let delay_num = delay_num.max(1);
    let delay_den = delay_den.max(1);
    for zdata in &frame_z {
        let mut fctl = Vec::with_capacity(26);
        fctl.extend_from_slice(&seq.to_be_bytes());
        fctl.extend_from_slice(&w.to_be_bytes());
        fctl.extend_from_slice(&h.to_be_bytes());
        fctl.extend_from_slice(&0u32.to_be_bytes());
        fctl.extend_from_slice(&0u32.to_be_bytes());
        fctl.extend_from_slice(&delay_num.to_be_bytes());
        fctl.extend_from_slice(&delay_den.to_be_bytes());
        fctl.extend_from_slice(&[0, 0]);
        seq += 1;
        chunks.push(png_chunk(b"fcTL", &fctl));
        let (fdats, next) = split_chunks(b"fdAT", zdata, true, seq);
        seq = next;
        chunks.extend(fdats);
    }
    if use_keepalive {
        let hb = zlib_bytes(&filter_rows(&RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]))))?;
        let mut fctl = Vec::with_capacity(26);
        fctl.extend_from_slice(&seq.to_be_bytes());
        fctl.extend_from_slice(&1u32.to_be_bytes());
        fctl.extend_from_slice(&1u32.to_be_bytes());
        fctl.extend_from_slice(&0u32.to_be_bytes());
        fctl.extend_from_slice(&0u32.to_be_bytes());
        fctl.extend_from_slice(&10u16.to_be_bytes());
        fctl.extend_from_slice(&100u16.to_be_bytes());
        fctl.extend_from_slice(&[0, 1]);
        seq += 1;
        chunks.push(png_chunk(b"fcTL", &fctl));
        chunks.extend(split_chunks(b"fdAT", &hb, true, seq).0);
    }
    chunks.push(png_chunk(b"IEND", &[]));
    Ok(chunks.concat())
}

fn unique_filename(dir: &Path, name: Option<&str>, ext: &str) -> String {
    let base = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| sanitize_filename(s, ext))
        .unwrap_or_else(|| format!("{}{ext}", Uuid::new_v4()));
    if !dir.join(&base).exists() {
        return base;
    }
    let stem = base.trim_end_matches(ext);
    for n in 2..1000 {
        let cand = format!("{stem}_{n}{ext}");
        if !dir.join(&cand).exists() {
            return cand;
        }
    }
    format!("{}{ext}", Uuid::new_v4())
}

fn save_bytes(bytes: &[u8], ext: &str, name: Option<&str>) -> Result<SavedImage, String> {
    let dir = default_output_dir().join("apng");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(unique_filename(&dir, name, ext));
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let mime = if ext == ".gif" { "image/gif" } else { "image/png" };
    Ok(SavedImage {
        path: path.to_string_lossy().into_owned(),
        data_url: format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
    })
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn encode_gif(frames: &[RgbaImage], delay_ms: u32) -> Result<Vec<u8>, String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame};
    if frames.len() < 2 {
        return Err("合成 GIF 至少需要 2 张图".into());
    }
    let mut out = Vec::new();
    {
        let mut enc = GifEncoder::new(&mut out);
        enc.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
        let delay = Delay::from_numer_denom_ms(delay_ms.max(20), 1);
        for frame in frames {
            enc.encode_frame(Frame::from_parts(frame.clone(), 0, 0, delay))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

fn align_frames(frames: &[RgbaImage], pad: [u8; 4], by_first: bool) -> Vec<RgbaImage> {
    let cleaned: Vec<_> = frames.iter().cloned().map(limit_side).collect();
    if cleaned.is_empty() {
        return cleaned;
    }
    let (tw, th) = if by_first {
        cleaned[0].dimensions()
    } else {
        (
            cleaned.iter().map(|f| f.width()).max().unwrap_or(1),
            cleaned.iter().map(|f| f.height()).max().unwrap_or(1),
        )
    };
    let m = tw.max(th);
    let (tw, th) = if m > MAX_SIDE {
        let scale = MAX_SIDE as f32 / m as f32;
        (
            (tw as f32 * scale).round().max(1.0) as u32,
            (th as f32 * scale).round().max(1.0) as u32,
        )
    } else {
        (tw, th)
    };
    cleaned.into_iter().map(|f| fit_contain(&f, tw, th, pad)).collect()
}

#[tauri::command]
pub fn apng_disguise(
    cover: String,
    reals: Vec<String>,
    delay_ms: Option<u32>,
    pad_color: Option<String>,
    name: Option<String>,
) -> Result<SavedImage, String> {
    if reals.is_empty() {
        return Err("请先加入至少一张真图".into());
    }
    let pad = parse_color(pad_color.as_deref().unwrap_or("#ffffff"));
    let frames = align_frames(
        &reals.iter().map(|s| decode_data_url(s)).collect::<Result<Vec<_>, _>>()?,
        pad,
        false,
    );
    let real0 = frames[0].clone();
    let cover_src = knockout_edge(limit_side(decode_data_url(&cover)?));
    let cover = fit_contain(&cover_src, real0.width(), real0.height(), pad);
    let delay = delay_ms.unwrap_or(400).max(20);
    let bytes = if frames.len() == 1 {
        write_apng(&cover, &frames, 10, 100)?
    } else {
        write_apng(&cover, &frames, delay as u16, 1000)?
    };
    save_bytes(&bytes, ".png", name.as_deref())
}

#[tauri::command]
pub fn apng_gif(frames: Vec<String>, delay_ms: Option<u32>, pad_color: Option<String>, fit_first: Option<bool>) -> Result<SavedImage, String> {
    let pad = parse_color(pad_color.as_deref().unwrap_or("#ffffff"));
    let aligned = align_frames(
        &frames.iter().map(|s| decode_data_url(s)).collect::<Result<Vec<_>, _>>()?,
        pad,
        fit_first.unwrap_or(false),
    );
    save_bytes(&encode_gif(&aligned, delay_ms.unwrap_or(400))?, ".gif", None)
}

#[tauri::command]
pub fn apng_clean(image: String) -> Result<String, String> {
    let img = decode_data_url(&image)?;
    data_url_png(&img)
}

#[tauri::command]
pub fn apng_strip(image: String) -> Result<SavedImage, String> {
    let img = limit_side(decode_data_url(&image)?);
    save_bytes(&encode_png(&img)?, ".png", Some("cleaned"))
}

#[tauri::command]
pub fn apng_mosaic(image: String, block: Option<u32>) -> Result<SavedImage, String> {
    let img = decode_data_url(&image)?;
    let (w, h) = img.dimensions();
    let block = block.unwrap_or(16).clamp(4, 64);
    let sw = (w / block).max(1);
    let sh = (h / block).max(1);
    let small = imageops::resize(&img, sw, sh, imageops::FilterType::Nearest);
    let out = imageops::resize(&small, w, h, imageops::FilterType::Nearest);
    save_bytes(&encode_png(&out)?, ".png", Some("mosaic"))
}

#[tauri::command]
pub fn apng_restore(image: String) -> Result<Vec<SavedImage>, String> {
    let raw = if let Some((_, b64)) = image.split_once("base64,") {
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| e.to_string())?
    } else {
        fs::read(image.trim()).or_else(|_| {
            base64::engine::general_purpose::STANDARD
                .decode(image.trim())
                .map_err(|e| e.to_string())
        })?
    };
    let frames = extract_hidden(&raw)?;
    if frames.is_empty() {
        return Err("没有从这张图里拆出隐藏帧".into());
    }
    if frames.len() == 1 {
        return Ok(vec![save_bytes(&encode_png(&frames[0])?, ".png", Some("restored"))?]);
    }
    Ok(vec![save_bytes(&encode_gif(&frames, 400)?, ".gif", Some("restored"))?])
}

fn clipboard_path(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let abs = fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    });
    let text = abs.to_string_lossy();
    let text = text
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .unwrap_or_else(|| text.strip_prefix(r"\\?\").unwrap_or(&text).to_string());
    let text = text.replace('/', "\\");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn hdrop_bytes(paths: &[String]) -> Vec<u8> {
    let mut wide: Vec<u16> = Vec::new();
    for path in paths {
        wide.extend(path.encode_utf16());
        wide.push(0);
    }
    wide.push(0);
    let mut buf = vec![0u8; 20 + wide.len() * 2];
    buf[0..4].copy_from_slice(&20u32.to_le_bytes());
    buf[16..20].copy_from_slice(&1i32.to_le_bytes());
    for (i, unit) in wide.into_iter().enumerate() {
        let at = 20 + i * 2;
        buf[at..at + 2].copy_from_slice(&unit.to_le_bytes());
    }
    buf
}

#[tauri::command]
pub fn copy_image_files(paths: Vec<String>) -> Result<(), String> {
    let existing: Vec<String> = paths
        .iter()
        .map(Path::new)
        .filter_map(clipboard_path)
        .collect();
    if existing.is_empty() {
        return Err("没有可复制的图片文件".into());
    }
    #[cfg(windows)]
    {
        return windows_copy_files(&existing);
    }
    #[cfg(not(windows))]
    {
        let _ = existing;
        Err("当前系统不支持复制图片文件".into())
    }
}

#[cfg(windows)]
fn windows_copy_files(paths: &[String]) -> Result<(), String> {
    let hdrop = hdrop_bytes(paths);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            let result = unsafe { windows_set_clipboard(&hdrop) };
            let _ = tx.send(result);
        })
        .map_err(|e| format!("复制文件失败：{e}"))?;
    rx.recv()
        .map_err(|_| "复制文件失败：剪贴板线程中断".to_string())?
}

#[cfg(windows)]
unsafe fn windows_set_clipboard(hdrop: &[u8]) -> Result<(), String> {
    use std::ffi::c_void;
    type Handle = *mut c_void;
    const CF_HDROP: u32 = 15;
    const GMEM_MOVEABLE: u32 = 0x0002;
    const GMEM_ZEROINIT: u32 = 0x0040;
    const DROPEFFECT_COPY: u32 = 1;
    #[link(name = "ole32")]
    extern "system" {
        fn OleInitialize(reserved: *mut c_void) -> i32;
        fn OleUninitialize();
    }
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: Handle) -> Handle;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
        fn GlobalLock(mem: Handle) -> *mut c_void;
        fn GlobalUnlock(mem: Handle) -> i32;
        fn GlobalFree(mem: Handle) -> Handle;
    }
    fn alloc_moveable(bytes: &[u8]) -> Result<Handle, String> {
        unsafe {
            let mem = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len());
            if mem.is_null() {
                return Err("申请剪贴板内存失败".into());
            }
            let lock = GlobalLock(mem);
            if lock.is_null() {
                GlobalFree(mem);
                return Err("锁定剪贴板内存失败".into());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), lock as *mut u8, bytes.len());
            GlobalUnlock(mem);
            Ok(mem)
        }
    }

    let hr = OleInitialize(std::ptr::null_mut());
    if hr < 0 {
        return Err("初始化剪贴板失败".into());
    }
    let mut opened = false;
    for _ in 0..25 {
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !opened {
        OleUninitialize();
        return Err("无法打开剪贴板，请稍后再试".into());
    }
    EmptyClipboard();
    let drop_mem = match alloc_moveable(hdrop) {
        Ok(mem) => mem,
        Err(err) => {
            CloseClipboard();
            OleUninitialize();
            return Err(err);
        }
    };
    if SetClipboardData(CF_HDROP, drop_mem).is_null() {
        GlobalFree(drop_mem);
        CloseClipboard();
        OleUninitialize();
        return Err("写入文件到剪贴板失败".into());
    }
    let effect = DROPEFFECT_COPY.to_le_bytes();
    let effect_name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
    let effect_fmt = RegisterClipboardFormatW(effect_name.as_ptr());
    if effect_fmt != 0 {
        if let Ok(mem) = alloc_moveable(&effect) {
            if SetClipboardData(effect_fmt, mem).is_null() {
                GlobalFree(mem);
            }
        }
    }
    CloseClipboard();
    OleUninitialize();
    Ok(())
}

fn extract_hidden(data: &[u8]) -> Result<Vec<RgbaImage>, String> {
    if data.len() < 16 || &data[..8] != PNG_SIG {
        let img = image::load_from_memory(data).map_err(|e| e.to_string())?.to_rgba8();
        return Ok(vec![img]);
    }
    let mut pos = 8usize;
    let mut frames = Vec::new();
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if pos + 12 + len > data.len() {
            break;
        }
        let kind = &data[pos + 4..pos + 8];
        let payload = &data[pos + 8..pos + 8 + len];
        if kind == b"fcTL" && payload.len() >= 12 {
            let w = u32::from_be_bytes(payload[4..8].try_into().unwrap());
            let h = u32::from_be_bytes(payload[8..12].try_into().unwrap());
            if w * h > 4 {
                if let Some(img) = take_next_fdat(data, pos + 12 + len, w, h) {
                    frames.push(img);
                }
            }
        }
        if kind == b"IEND" {
            break;
        }
        pos += 12 + len;
    }
    if frames.is_empty() {
        let img = image::load_from_memory(data).map_err(|e| e.to_string())?.to_rgba8();
        frames.push(img);
    }
    Ok(frames)
}

fn take_next_fdat(data: &[u8], mut pos: usize, w: u32, h: u32) -> Option<RgbaImage> {
    let mut z = Vec::new();
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if pos + 12 + len > data.len() {
            break;
        }
        let kind = &data[pos + 4..pos + 8];
        let payload = &data[pos + 8..pos + 8 + len];
        if kind == b"fdAT" && payload.len() > 4 {
            z.extend_from_slice(&payload[4..]);
            pos += 12 + len;
            continue;
        }
        if kind == b"IDAT" {
            z.extend_from_slice(payload);
            pos += 12 + len;
            continue;
        }
        if kind == b"fcTL" || kind == b"IEND" {
            break;
        }
        pos += 12 + len;
    }
    let raw = inflate(&z)?;
    decode_filtered(&raw, w, h)
}

fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;
    let mut dec = ZlibDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).ok()?;
    Some(out)
}

fn decode_filtered(raw: &[u8], w: u32, h: u32) -> Option<RgbaImage> {
    let row = w as usize * 4;
    if raw.len() < (row + 1) * h as usize {
        return None;
    }
    let mut img = RgbaImage::new(w, h);
    for y in 0..h as usize {
        let o = y * (row + 1);
        if raw[o] != 0 {
            return None;
        }
        for x in 0..w as usize {
            let i = o + 1 + x * 4;
            img.put_pixel(x as u32, y as u32, Rgba([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]));
        }
    }
    Some(img)
}

#[tauri::command]
pub async fn pick_images(app: tauri::AppHandle) -> Result<Vec<SavedImage>, String> {
    use tauri_plugin_dialog::DialogExt;
    let files = app
        .dialog()
        .file()
        .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
        .blocking_pick_files();
    let Some(files) = files else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for file in files {
        let Ok(path) = file.into_path() else { continue };
        if let Ok(bytes) = fs::read(&path) {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("png")
                .to_ascii_lowercase();
            let mime = match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "bmp" => "image/bmp",
                _ => "image/png",
            };
            out.push(SavedImage {
                path: path.to_string_lossy().into_owned(),
                data_url: format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::hdrop_bytes;

    #[test]
    fn hdrop_payload_is_wide_and_double_null() {
        let raw = hdrop_bytes(&["C:\\temp\\a.png".into()]);
        assert_eq!(&raw[0..4], &20u32.to_le_bytes());
        assert_eq!(&raw[16..20], &1i32.to_le_bytes());
        assert_eq!(raw[raw.len() - 2..], [0, 0]);
        let wide: Vec<u16> = raw[20..]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16_lossy(&wide);
        assert!(text.starts_with("C:\\temp\\a.png"));
    }
}
