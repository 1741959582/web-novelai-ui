use serde::Serialize;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const MIN_VRAM_MB: u32 = 3072;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u32,
    pub driver: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuDetectResult {
    pub usable: bool,
    pub reason: String,
    pub gpus: Vec<GpuInfo>,
}

pub fn detect() -> GpuDetectResult {
    match run_nvidia_smi() {
        Ok(stdout) => from_smi_csv(&stdout),
        Err(reason) => GpuDetectResult {
            usable: false,
            reason,
            gpus: Vec::new(),
        },
    }
}

fn run_nvidia_smi() -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,driver_version",
                "--format=csv,noheader,nounits",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(output);
    });
    match rx.recv_timeout(Duration::from_secs(8)) {
        Ok(Ok(out)) => {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                let err = err.trim();
                if err.is_empty() {
                    return Err("nvidia-smi 执行失败，本机可能没有 NVIDIA 驱动。".into());
                }
                return Err(format!("nvidia-smi 失败：{err}"));
            }
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        Ok(Err(e)) => Err(format!(
            "未检测到 nvidia-smi（{e}）。本机可能没有 NVIDIA 显卡或驱动未安装。"
        )),
        Err(_) => Err("nvidia-smi 超时，已跳过显卡检测。".into()),
    }
}

pub fn from_smi_csv(stdout: &str) -> GpuDetectResult {
    let mut gpus = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.to_ascii_lowercase().contains("failed") {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() < 2 {
            continue;
        }
        let vram_mb = parts[1]
            .split('.')
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        gpus.push(GpuInfo {
            name: parts[0].to_string(),
            vram_mb,
            driver: parts.get(2).copied().unwrap_or("").to_string(),
        });
    }
    if gpus.is_empty() {
        return GpuDetectResult {
            usable: false,
            reason: "nvidia-smi 没有返回可用 GPU。".into(),
            gpus,
        };
    }
    let best = gpus.iter().max_by_key(|g| g.vram_mb).cloned();
    let Some(best) = best else {
        return GpuDetectResult {
            usable: false,
            reason: "没有可用 GPU。".into(),
            gpus,
        };
    };
    if best.vram_mb < MIN_VRAM_MB {
        return GpuDetectResult {
            usable: false,
            reason: format!(
                "检测到 {}（{} MB 显存），低于本地 CL Tagger 建议的 {} MB。",
                best.name, best.vram_mb, MIN_VRAM_MB
            ),
            gpus,
        };
    }
    GpuDetectResult {
        usable: true,
        reason: format!(
            "检测到 {}（{} MB 显存），可以启用本地 CL Tagger v2。",
            best.name, best.vram_mb
        ),
        gpus,
    }
}

#[tauri::command]
pub fn gpu_detect() -> GpuDetectResult {
    detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usable_gpu() {
        let out = from_smi_csv("NVIDIA GeForce RTX 4070, 12282, 551.23\n");
        assert!(out.usable);
        assert_eq!(out.gpus[0].name, "NVIDIA GeForce RTX 4070");
        assert_eq!(out.gpus[0].vram_mb, 12282);
    }

    #[test]
    fn rejects_low_vram() {
        let out = from_smi_csv("NVIDIA GeForce GTX 1650, 4096, 531.41\n");
        assert!(out.usable);
        let low = from_smi_csv("NVIDIA GeForce MX150, 2048, 31.0\n");
        assert!(!low.usable);
    }

    #[test]
    fn empty_output_not_usable() {
        let out = from_smi_csv("");
        assert!(!out.usable);
        assert!(out.gpus.is_empty());
    }
}
