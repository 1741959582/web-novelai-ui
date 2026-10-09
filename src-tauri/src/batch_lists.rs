use crate::store::data_dir;
use serde_json::Value;
use std::fs;

fn library_path() -> Result<std::path::PathBuf, String> {
    Ok(data_dir()?.join("batch-lists.json"))
}

#[tauri::command]
pub fn batch_library_load() -> Result<Value, String> {
    let path = library_path()?;
    if !path.exists() {
        return Ok(Value::Null);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("读取批量任务失败：{e}"))
}

#[tauri::command]
pub fn batch_library_save(library: Value) -> Result<(), String> {
    let path = library_path()?;
    let bytes = serde_json::to_vec(&library).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}
