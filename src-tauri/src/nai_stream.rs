use base64::Engine;
use rmpv::Value;
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::{AppHandle, Emitter};

const MAX_FRAME: u32 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateProgress {
    pub progress: f64,
    pub current_step: u32,
    pub total_steps: u32,
    pub preview_data_url: String,
    pub phase: String,
}

#[derive(Debug)]
pub enum StreamResult {
    Zip(Vec<u8>),
    Images(Vec<Vec<u8>>),
}

#[derive(Debug, Clone)]
pub struct StreamFrame {
    pub event_type: String,
    pub sample_index: u32,
    pub step_index: Option<u32>,
    pub image: Option<Vec<u8>>,
    pub error: Option<String>,
}

fn as_map(value: &Value) -> Option<&Vec<(Value, Value)>> {
    match value {
        Value::Map(items) => Some(items),
        _ => None,
    }
}

fn map_get<'a>(map: &'a [(Value, Value)], key: &str) -> Option<&'a Value> {
    map.iter().find(|(k, _)| k.as_str() == Some(key)).map(|(_, v)| v)
}

fn as_int(value: &Value) -> Option<u32> {
    value.as_i64().or_else(|| value.as_u64().map(|n| n as i64)).and_then(|n| u32::try_from(n.max(0)).ok())
}

fn decode_image(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Binary(bytes) => Some(bytes.clone()),
        Value::String(s) => {
            let raw = s.as_str()?.trim();
            let compact = raw
                .split(',')
                .next_back()
                .unwrap_or(raw)
                .replace(['\n', '\r', ' '], "");
            base64::engine::general_purpose::STANDARD.decode(compact.as_bytes()).ok()
        }
        Value::Array(items) if items.iter().all(|v| v.as_u64().is_some()) => {
            Some(items.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect())
        }
        _ => None,
    }
}

fn frame_from_map(map: &[(Value, Value)], fallback: Option<&str>) -> StreamFrame {
    let nested = map_get(map, "payload")
        .or_else(|| map_get(map, "data"))
        .and_then(as_map);
    let source = if nested.is_some() && map_get(map, "image").is_none() && map_get(map, "event_type").is_none() {
        nested.unwrap()
    } else {
        map
    };
    let event_type = map_get(source, "event_type")
        .or_else(|| map_get(source, "eventType"))
        .or_else(|| map_get(source, "type"))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .or_else(|| fallback.map(|s| s.to_string()))
        .unwrap_or_else(|| "intermediate".into());
    let error_val = map_get(source, "message").or_else(|| map_get(source, "error"));
    let image = map_get(source, "image")
        .or_else(|| map_get(source, "data"))
        .or_else(|| map_get(source, "image_data"))
        .or_else(|| map_get(source, "imageData"))
        .and_then(decode_image);
    StreamFrame {
        sample_index: map_get(source, "samp_ix")
            .or_else(|| map_get(source, "sampleIndex"))
            .or_else(|| map_get(source, "sample_index"))
            .and_then(as_int)
            .unwrap_or(0),
        step_index: map_get(source, "step_ix")
            .or_else(|| map_get(source, "stepIndex"))
            .or_else(|| map_get(source, "step_index"))
            .and_then(as_int),
        error: if event_type == "error" || map_get(source, "error").is_some() {
            Some(
                error_val
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "Stream generation failed".into()),
            )
        } else {
            None
        },
        event_type,
        image,
    }
}

fn decode_msgpack_frame(bytes: &[u8]) -> Option<StreamFrame> {
    let value = rmpv::decode::read_value(&mut &bytes[..]).ok()?;
    as_map(&value).map(|map| frame_from_map(map, None))
}

struct MsgpackDecoder {
    pending: Vec<u8>,
}

impl MsgpackDecoder {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<StreamFrame>, String> {
        self.pending.extend_from_slice(chunk);
        let mut frames = Vec::new();
        while self.pending.len() >= 4 {
            let length = u32::from_be_bytes(self.pending[..4].try_into().unwrap());
            if length == 0 || length > MAX_FRAME {
                return Err(format!("Invalid NovelAI stream frame length: {length}"));
            }
            let total = 4 + length as usize;
            if self.pending.len() < total {
                break;
            }
            if let Some(frame) = decode_msgpack_frame(&self.pending[4..total]) {
                frames.push(frame);
            }
            self.pending.drain(..total);
        }
        Ok(frames)
    }
}

struct SseDecoder {
    pending: String,
    event_type: String,
    data_lines: Vec<String>,
}

impl SseDecoder {
    fn new() -> Self {
        Self {
            pending: String::new(),
            event_type: String::new(),
            data_lines: Vec::new(),
        }
    }

    fn dispatch(&mut self) -> Vec<StreamFrame> {
        if self.data_lines.is_empty() {
            self.event_type.clear();
            return Vec::new();
        }
        let data = self.data_lines.join("\n");
        self.data_lines.clear();
        let event = std::mem::take(&mut self.event_type);
        decode_sse_data(&event, &data).into_iter().collect()
    }

    fn consume_lines(&mut self, final_flush: bool) -> Vec<StreamFrame> {
        let mut frames = Vec::new();
        while let Some(idx) = self.pending.find('\n') {
            let mut line = self.pending[..idx].to_string();
            self.pending = self.pending[idx + 1..].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            if line.is_empty() {
                frames.extend(self.dispatch());
            } else if !line.starts_with(':') {
                let (field, value) = split_sse_field(&line);
                if field == "event" {
                    self.event_type = value.trim().to_string();
                } else if field == "data" {
                    self.data_lines.push(value);
                }
            }
        }
        if final_flush {
            if !self.pending.is_empty() {
                let line = self.pending.trim_end_matches('\r').to_string();
                let (field, value) = split_sse_field(&line);
                if field == "event" {
                    self.event_type = value.trim().to_string();
                } else if field == "data" {
                    self.data_lines.push(value);
                }
            }
            self.pending.clear();
            frames.extend(self.dispatch());
        }
        frames
    }

    fn push(&mut self, chunk: &[u8]) -> Vec<StreamFrame> {
        self.pending.push_str(&String::from_utf8_lossy(chunk));
        self.consume_lines(false)
    }

    fn finish(&mut self) -> Vec<StreamFrame> {
        self.consume_lines(true)
    }
}

fn split_sse_field(line: &str) -> (String, String) {
    if let Some(colon) = line.find(':') {
        let value = line[colon + 1..].strip_prefix(' ').unwrap_or(&line[colon + 1..]);
        (line[..colon].to_string(), value.to_string())
    } else {
        (line.to_string(), String::new())
    }
}

fn decode_sse_data(event_type: &str, value: &str) -> Option<StreamFrame> {
    let data = value.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(obj) = parsed.as_object() {
            let map: Vec<(Value, Value)> = obj
                .iter()
                .map(|(k, v)| (Value::from(k.as_str()), json_to_rmpv(v)))
                .collect();
            return Some(frame_from_map(&map, if event_type.is_empty() { None } else { Some(event_type) }));
        }
        if let Some(inner) = parsed.as_str() {
            return decode_sse_data(event_type, inner);
        }
    }
    if event_type == "error" {
        return Some(StreamFrame {
            event_type: event_type.into(),
            sample_index: 0,
            step_index: None,
            image: None,
            error: Some(data.to_string()),
        });
    }
    None
}

fn json_to_rmpv(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                Value::from(u)
            } else {
                Value::from(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Value::from(s.as_str()),
        serde_json::Value::Array(items) => Value::Array(items.iter().map(json_to_rmpv).collect()),
        serde_json::Value::Object(obj) => Value::Map(
            obj.iter()
                .map(|(k, v)| (Value::from(k.as_str()), json_to_rmpv(v)))
                .collect(),
        ),
    }
}

fn looks_like_sse(prefix: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&prefix[..prefix.len().min(32)]);
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with("event:")
        || trimmed.starts_with("data:")
        || trimmed.starts_with("id:")
        || trimmed.starts_with("retry:")
        || trimmed.starts_with(':')
}

fn image_data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        "image/png"
    } else if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 {
        "image/jpeg"
    } else {
        "image/png"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

pub fn emit_progress(app: &AppHandle, progress: GenerateProgress) {
    let _ = app.emit("generate-progress", progress);
}

pub fn emit_waiting(app: &AppHandle, total_steps: u32) {
    emit_progress(
        app,
        GenerateProgress {
            progress: 0.05,
            current_step: 0,
            total_steps,
            preview_data_url: String::new(),
            phase: "waiting".into(),
        },
    );
}

pub fn emit_saving(app: &AppHandle, preview: &str, total_steps: u32) {
    emit_progress(
        app,
        GenerateProgress {
            progress: 1.0,
            current_step: total_steps,
            total_steps,
            preview_data_url: preview.to_string(),
            phase: "saving".into(),
        },
    );
}

pub async fn consume_generate_stream(
    app: &AppHandle,
    response: reqwest::Response,
    total_steps: u32,
) -> Result<StreamResult, String> {
    use futures_util::StreamExt;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut mode = if content_type.contains("text/event-stream") {
        "sse"
    } else {
        "unknown"
    };
    let mut msgpack = MsgpackDecoder::new();
    let mut sse = SseDecoder::new();
    let mut finals: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut zip_chunks: Vec<u8> = Vec::new();
    let mut prefix: Vec<u8> = Vec::new();
    let mut last_preview = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut stream = response.bytes_stream();

    let mut consume = |frames: Vec<StreamFrame>| -> Result<(), String> {
        for frame in frames {
            if let Some(err) = frame.error {
                return Err(err);
            }
            let Some(image) = frame.image.filter(|b| !b.is_empty()) else {
                continue;
            };
            let current_step = frame.step_index.unwrap_or(0).saturating_add(1);
            if frame.event_type == "final" {
                emit_progress(
                    app,
                    GenerateProgress {
                        progress: 1.0,
                        current_step: total_steps,
                        total_steps,
                        preview_data_url: image_data_url(&image),
                        phase: "saving".into(),
                    },
                );
                finals.insert(frame.sample_index, image);
            } else if last_preview.elapsed().as_millis() >= 110 || current_step >= total_steps {
                last_preview = std::time::Instant::now();
                emit_progress(
                    app,
                    GenerateProgress {
                        progress: (current_step as f64 / total_steps.max(1) as f64).clamp(0.0, 0.99),
                        current_step,
                        total_steps,
                        preview_data_url: image_data_url(&image),
                        phase: "streaming".into(),
                    },
                );
            }
        }
        Ok(())
    };

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        if mode == "unknown" {
            prefix.extend_from_slice(&chunk);
            if prefix.len() < 4 {
                continue;
            }
            let framed = u32::from_be_bytes(prefix[..4].try_into().unwrap());
            mode = if prefix.starts_with(&[0x50, 0x4b]) {
                "zip"
            } else if looks_like_sse(&prefix) {
                "sse"
            } else if framed > 0 && framed <= MAX_FRAME {
                "msgpack"
            } else {
                "sse"
            };
            if mode == "zip" {
                zip_chunks.extend_from_slice(&prefix);
            } else if mode == "sse" {
                consume(sse.push(&prefix))?;
            } else {
                consume(msgpack.push(&prefix)?)?;
            }
            prefix.clear();
            continue;
        }
        if mode == "zip" {
            zip_chunks.extend_from_slice(&chunk);
        } else if mode == "sse" {
            consume(sse.push(&chunk))?;
        } else {
            consume(msgpack.push(&chunk)?)?;
        }
    }

    if mode == "zip" {
        return Ok(StreamResult::Zip(zip_chunks));
    }
    if mode == "sse" {
        consume(sse.finish())?;
    }
    if finals.is_empty() {
        return Err("流式生成结束，但没有收到最终图片。为避免重复扣费，未自动重发请求。".into());
    }
    Ok(StreamResult::Images(finals.into_values().collect()))
}

pub fn is_streaming_not_allowed(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("streaming is not allowed")
        || text.contains("streaming not allowed")
        || text.contains("stream is not allowed")
        || text.contains("stream not allowed")
}

pub fn supports_safe_stream(payload: &serde_json::Value, action: &str) -> bool {
    if action != "generate" {
        return false;
    }
    let Some(params) = payload.get("parameters") else {
        return false;
    };
    if params.get("image").is_some() || params.get("mask").is_some() || params.get("reference_image").is_some() {
        return false;
    }
    if params
        .get("reference_image_multiple")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        return false;
    }
    if params
        .get("director_reference_images")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        return false;
    }
    if params
        .get("director_reference_images_cached")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        return false;
    }
    true
}
