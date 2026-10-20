use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const DEFAULT_PORT: u16 = 8765;
const READY_TIMEOUT: Duration = Duration::from_secs(90);

static START_LOCK: Mutex<()> = Mutex::new(());
static SPAWNED: Mutex<Option<Child>> = Mutex::new(None);
static LISTEN_PORT: AtomicU16 = AtomicU16::new(DEFAULT_PORT);

fn listen_port() -> u16 {
    LISTEN_PORT.load(Ordering::Relaxed)
}

fn port_is_free(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err()
}

fn pick_port() -> u16 {
    (DEFAULT_PORT..DEFAULT_PORT + 16)
        .find(|port| port_is_free(*port))
        .unwrap_or(DEFAULT_PORT)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuyiStatus {
    pub running: bool,
    pub port: u16,
    pub exe_path: String,
    pub firefox_ready: bool,
    pub firefox_cached: bool,
    pub message: String,
}

pub fn start_on_launch() {
    std::thread::spawn(|| {
        let _ = ensure_started();
    });
}

pub fn stop_if_spawned() {
    let child = SPAWNED.lock().ok().and_then(|mut slot| slot.take());
    if let Some(mut child) = child {
        let pid = child.id();
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    kill_bundled_firefox();
}

fn kill_bundled_firefox() {
    #[cfg(windows)]
    {
        let marker = firefox_exe(&electron_root())
            .to_string_lossy()
            .replace('/', "\\")
            .to_ascii_lowercase();
        if marker.trim().is_empty() {
            return;
        }
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-Command",
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-CimInstance Win32_Process -Filter \"Name='firefox.exe'\" | ForEach-Object { $_.ProcessId.ToString() + \"`t\" + $_.CommandLine }",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000);
        let Ok(output) = cmd.output() else {
            return;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((pid, command)) = line.trim().split_once('\t') else {
                continue;
            };
            let folded = command.replace('/', "\\").to_ascii_lowercase();
            if !folded.contains(&marker) {
                continue;
            }
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000)
                .output();
        }
    }
}

fn electron_root() -> PathBuf {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("electron");
    if dev.join("firefox").is_dir() {
        return dev;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join("electron");
            if beside.join("firefox").is_dir() || beside.is_dir() {
                return beside;
            }
        }
    }
    dev
}

fn server_candidates() -> Vec<PathBuf> {
    let mut paths = vec![electron_root().join("ruyipage-server.exe")];
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(projects) = manifest.parent().and_then(|p| p.parent()) {
        paths.push(
            projects
                .join("qzl-aicjxt")
                .join("electron")
                .join("ruyipage-server.exe"),
        );
    }
    paths.push(PathBuf::from(r"D:\project\qzl-aicjxt\electron\ruyipage-server.exe"));
    paths
}

fn server_exe() -> Option<PathBuf> {
    server_candidates().into_iter().find(|path| path.is_file())
}

fn firefox_exe(root: &Path) -> PathBuf {
    root.join("firefox").join("firefox.exe")
}

fn firefox_zip(root: &Path) -> PathBuf {
    root.join("firefox.zip")
}

fn accounts_path(root: &Path) -> PathBuf {
    root.join("data").join("browser_accounts.json")
}

fn http_ready(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let req = format!("GET /config/concurrency HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 80];
    let n = stream.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    head.starts_with("HTTP/") && !head.contains(" 500")
}

fn wait_until_ready(port: u16) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < READY_TIMEOUT {
        if http_ready(port) {
            return Ok(());
        }
        if let Ok(mut slot) = SPAWNED.lock() {
            if let Some(child) = slot.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!("脚本包启动后退出了：{status}"));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    Err(format!("脚本包已拉起，但 {} 秒内没有监听到 {port}", READY_TIMEOUT.as_secs()))
}

fn status_now() -> RuyiStatus {
    let root = electron_root();
    let exe = server_exe();
    let running = http_ready(listen_port());
    let firefox_ready = firefox_exe(&root).is_file();
    let firefox_cached = firefox_zip(&root).is_file();
    let message = if running {
        "脚本包已在运行".to_string()
    } else if exe.is_none() {
        "没有找到 ruyipage-server.exe".to_string()
    } else if !firefox_ready {
        "指纹浏览器还没准备好".to_string()
    } else {
        "脚本包未启动".to_string()
    };
    RuyiStatus {
        running,
        port: listen_port(),
        exe_path: exe.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        firefox_ready,
        firefox_cached,
        message,
    }
}

fn ensure_firefox_ready() {
    let root = electron_root();
    if firefox_exe(&root).is_file() {
        return;
    }
    let _ = cache_firefox_blocking();
}

fn ensure_started() -> Result<RuyiStatus, String> {
    let _guard = START_LOCK.lock().map_err(|_| "启动锁不可用".to_string())?;
    ensure_firefox_ready();
    let owned = SPAWNED
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|child| child.id()))
        .is_some();
    if owned && http_ready(listen_port()) {
        return Ok(status_now());
    }
    let port = pick_port();
    if !port_is_free(port) {
        return Err("脚本包端口 8765 到 8780 都已被占用".into());
    }
    LISTEN_PORT.store(port, Ordering::Relaxed);
    let exe = server_exe().ok_or_else(|| {
        "没有找到 ruyipage-server.exe。请放到 electron 目录，或保留参考项目里的脚本包。".to_string()
    })?;
    let root = electron_root();
    let firefox = firefox_exe(&root);
    fs::create_dir_all(root.join("data")).map_err(|e| e.to_string())?;
    let log_path = root.join("data").join("ruyipage-server.log");
    let log = File::create(&log_path).map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&exe);
    cmd.current_dir(&root)
        .env("RUYI_ELECTRON_ROOT", &root)
        .env("RUYIPAGE_ELECTRON_ROOT", &root)
        .env("RUYI_OPEN_PORT", port.to_string())
        .env("RUYIPAGE_SERVER_PORT", port.to_string())
        .env("RUYI_MAX_LIVE_BROWSERS", "8")
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    if firefox.is_file() {
        let firefox_text = firefox.to_string_lossy().to_string();
        cmd.env("RUYIPAGE_FIREFOX_PATH", &firefox_text);
        cmd.env("RUYI_FIREFOX_PATH", &firefox_text);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let child = cmd.spawn().map_err(|e| format!("启动脚本包失败：{e}"))?;
    if let Ok(mut slot) = SPAWNED.lock() {
        if let Some(mut old) = slot.replace(child) {
            let _ = old.kill();
        }
    }
    if let Err(message) = wait_until_ready(port) {
        stop_if_spawned();
        return Err(format!("{message}。日志：{}", log_path.display()));
    }
    Ok(status_now())
}

fn read_accounts() -> Result<Value, String> {
    let mut paths = Vec::new();
    if let Some(exe) = server_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("data").join("browser_accounts.json"));
        }
    }
    paths.push(accounts_path(&electron_root()));
    paths.push(PathBuf::from(r"D:\project\qzl-aicjxt\electron\data\browser_accounts.json"));
    for path in paths {
        if !path.is_file() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                return Ok(value);
            }
        }
    }
    Ok(json!({ "version": 1, "accounts": [] }))
}

async fn fetch_accounts() -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let url = format!("http://127.0.0.1:{}/run/browser/accounts", listen_port());
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("读取账号失败 HTTP {}", response.status()));
    }
    let value: Value = response.json().await.map_err(|error| error.to_string())?;
    if value.get("accounts").is_some() {
        return Ok(value);
    }
    Ok(json!({ "accounts": [] }))
}

fn find_account_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(id) = map.get("account_id").and_then(|v| v.as_str()) {
                if !id.is_empty() {
                    return Some(id.to_string());
                }
            }
            for child in map.values() {
                if let Some(id) = find_account_id(child) {
                    return Some(id);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_account_id),
        _ => None,
    }
}

fn profile_dir_in(value: &Value) -> Option<PathBuf> {
    match value {
        Value::Object(map) => {
            if let Some(dir) = map.get("profile_dir").and_then(|item| item.as_str()) {
                if !dir.is_empty() {
                    return Some(PathBuf::from(dir));
                }
            }
            map.values().find_map(profile_dir_in)
        }
        Value::Array(items) => items.iter().find_map(profile_dir_in),
        _ => None,
    }
}

fn newest_account_id() -> Option<String> {
    let store = read_accounts().ok()?;
    let accounts = store.get("accounts")?.as_array()?;
    accounts.iter().rev().find_map(|row| {
        row.get("account_id")
            .and_then(|v| v.as_str())
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string())
    })
}

async fn post_json(path: &str, body: Value) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{}{path}", listen_port());
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求脚本包失败：{e}"))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("脚本包返回 {status}：{text}"));
    }
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

fn copy_zip_from_reference(dest: &Path) -> bool {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    if let Some(projects) = manifest.parent().and_then(|p| p.parent()) {
        sources.push(projects.join("qzl-aicjxt").join("electron").join("firefox.zip"));
    }
    sources.push(PathBuf::from(r"D:\project\qzl-aicjxt\electron\firefox.zip"));
    for source in sources {
        if source.is_file() && fs::copy(&source, dest).is_ok() {
            return true;
        }
    }
    false
}

fn zip_dir(src: &Path, dest: &Path) -> Result<(), String> {
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let rel = path.strip_prefix(src).unwrap_or(&path);
            let name = rel.to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                stack.push(path);
                let folder = if name.ends_with('/') { name } else { format!("{name}/") };
                zip.add_directory(&folder, options).map_err(|e| e.to_string())?;
            } else {
                zip.start_file(&name, options).map_err(|e| e.to_string())?;
                let mut input = File::open(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut input, &mut zip).map_err(|e| e.to_string())?;
            }
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index).map_err(|e| e.to_string())?;
        let Some(rel) = item.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out = dest.join(rel);
        if item.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut item, &mut output).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn cache_firefox_blocking() -> Result<String, String> {
    let root = electron_root();
    let firefox = root.join("firefox");
    let zip_path = firefox_zip(&root);
    let ready = firefox_exe(&root).is_file();
    if ready && zip_path.is_file() {
        return Ok("指纹浏览器已经缓存".into());
    }
    if ready {
        let partial = root.join("firefox.zip.partial");
        zip_dir(&firefox, &partial)?;
        if zip_path.exists() {
            fs::remove_file(&zip_path).map_err(|e| e.to_string())?;
        }
        fs::rename(&partial, &zip_path).map_err(|e| e.to_string())?;
        return Ok("已把当前指纹浏览器缓存成 firefox.zip".into());
    }
    if !zip_path.is_file() && !copy_zip_from_reference(&zip_path) {
        return Err("没有可缓存的指纹浏览器，也没有找到参考项目里的 firefox.zip".into());
    }
    extract_zip(&zip_path, &firefox)?;
    if !firefox_exe(&root).is_file() {
        return Err("缓存已解压，但没有找到 firefox.exe".into());
    }
    Ok("已从缓存恢复指纹浏览器".into())
}

#[tauri::command]
pub fn ruyi_status() -> RuyiStatus {
    status_now()
}

#[tauri::command]
pub async fn ruyi_accounts() -> Result<Value, String> {
    if http_ready(listen_port()) {
        if let Ok(value) = fetch_accounts().await {
            return Ok(value);
        }
    }
    read_accounts()
}

#[tauri::command]
pub async fn ruyi_ensure() -> Result<RuyiStatus, String> {
    tokio::task::spawn_blocking(ensure_started)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ruyi_cache_firefox() -> Result<String, String> {
    tokio::task::spawn_blocking(cache_firefox_blocking)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ruyi_open_browser(label: String, account_id: String, kind: String) -> Result<Value, String> {
    tokio::task::spawn_blocking(ensure_started)
        .await
        .map_err(|e| e.to_string())??;
    let kind = if kind.trim().is_empty() { "x".to_string() } else { kind.trim().to_string() };
    let account_id = account_id.trim().to_string();
    let account_id = if account_id.is_empty() {
        let label = if label.trim().is_empty() {
            format!("指纹 {}", chrono::Local::now().format("%m%d-%H%M"))
        } else {
            label.trim().to_string()
        };
        let created = post_json(
            "/run/browser/accounts",
            json!({ "label": label, "type": kind, "open_login": false }),
        )
        .await?;
        find_account_id(&created).or_else(newest_account_id).ok_or_else(|| {
            format!("脚本包没有返回账号编号：{created}")
        })?
    } else {
        account_id
    };
    let opened = post_json(
        "/run/browser/open",
        json!({ "account_id": account_id, "type": kind }),
    )
    .await?;
    Ok(json!({ "accountId": account_id, "opened": opened }))
}

const TWITTER_TYPE: &str = "x";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyChoice {
    pub id: String,
    pub name: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClashNodes {
    pub ok: bool,
    pub message: String,
    pub group: String,
    pub current: String,
    pub nodes: Vec<String>,
    pub product: String,
    pub choices: Vec<ProxyChoice>,
}

struct ClashEndpoint {
    controller: String,
    secret: String,
    mixed_port: u16,
}

const FREECAT_CONTROLLER: &str = "127.0.0.1:9090";
const FREECAT_MIXED_PORT: u16 = 7892;

fn clash_config_path() -> PathBuf {
    let home = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(home)
        .join("io.github.clash-verge-rev.clash-verge-rev")
        .join("config.yaml")
}

fn yaml_value(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix(key)?.strip_prefix(':')?.trim();
            let rest = rest.trim_matches('"').trim_matches('\'');
            if rest.is_empty() { None } else { Some(rest.to_string()) }
        })
        .unwrap_or_default()
}

fn freecat_prefs() -> Value {
    let home = std::env::var("APPDATA").unwrap_or_default();
    let path = PathBuf::from(home)
        .join("ziyoumao")
        .join("ziyoumao")
        .join("shared_preferences.json");
    let text = fs::read_to_string(path).unwrap_or_default();
    let root: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let config_text = root
        .get("flutter.config")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    serde_json::from_str(config_text).unwrap_or(Value::Null)
}

fn freecat_endpoint() -> ClashEndpoint {
    let patch = freecat_prefs()
        .get("patchClashConfig")
        .cloned()
        .unwrap_or(Value::Null);
    let controller = patch
        .get("external-controller")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let secret = patch
        .get("secret")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mixed_port = patch
        .get("mixed-port")
        .and_then(|value| value.as_u64())
        .filter(|port| *port > 0 && *port <= u16::MAX as u64)
        .unwrap_or(FREECAT_MIXED_PORT as u64) as u16;
    ClashEndpoint {
        controller: if controller.is_empty() {
            FREECAT_CONTROLLER.to_string()
        } else {
            controller
        },
        secret,
        mixed_port,
    }
}

fn clash_verge_endpoint() -> ClashEndpoint {
    let text = fs::read_to_string(clash_config_path()).unwrap_or_default();
    let controller = yaml_value(&text, "external-controller");
    let controller = if controller.is_empty() {
        "127.0.0.1:9097".to_string()
    } else {
        controller
    };
    let secret = yaml_value(&text, "secret");
    let mixed_port = yaml_value(&text, "mixed-port").parse().unwrap_or(7897);
    ClashEndpoint {
        controller,
        secret,
        mixed_port,
    }
}

fn proxy_store_path() -> PathBuf {
    electron_root().join("data").join("twitter_proxy.json")
}

fn read_proxy_map() -> Value {
    let path = proxy_store_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

fn proxy_name_for(account_id: &str) -> String {
    read_proxy_map()
        .get(account_id)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn save_proxy_name(account_id: &str, proxy_name: &str) -> Result<(), String> {
    let path = proxy_store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut map = read_proxy_map();
    if !map.is_object() {
        map = json!({});
    }
    if proxy_name.trim().is_empty() {
        if let Some(object) = map.as_object_mut() {
            object.remove(account_id);
        }
    } else if let Some(object) = map.as_object_mut() {
        object.insert(account_id.to_string(), json!(proxy_name.trim()));
    }
    fs::write(path, serde_json::to_string_pretty(&map).unwrap_or_else(|_| "{}".into()))
        .map_err(|error| error.to_string())
}

fn profile_dir_of(account_id: &str) -> Option<PathBuf> {
    let store = read_accounts().ok()?;
    let accounts = store.get("accounts")?.as_array()?;
    accounts.iter().find_map(|row| {
        let id = row.get("account_id").and_then(|value| value.as_str())?;
        if id != account_id {
            return None;
        }
        let dir = row.get("profile_dir").and_then(|value| value.as_str())?;
        if dir.is_empty() { None } else { Some(PathBuf::from(dir)) }
    })
}

fn write_firefox_proxy(profile: &Path, mixed_port: Option<u16>) -> Result<(), String> {
    if !profile.is_dir() {
        return Ok(());
    }
    let body = if let Some(port) = mixed_port {
        format!(
            "user_pref(\"nai.proxy.managed\", true);\n\
             user_pref(\"network.proxy.type\", 1);\n\
             user_pref(\"network.proxy.http\", \"127.0.0.1\");\n\
             user_pref(\"network.proxy.http_port\", {port});\n\
             user_pref(\"network.proxy.ssl\", \"127.0.0.1\");\n\
             user_pref(\"network.proxy.ssl_port\", {port});\n\
             user_pref(\"network.proxy.socks\", \"127.0.0.1\");\n\
             user_pref(\"network.proxy.socks_port\", {port});\n\
             user_pref(\"network.proxy.socks_version\", 5);\n\
             user_pref(\"network.proxy.socks_remote_dns\", true);\n\
             user_pref(\"network.proxy.share_proxy_settings\", true);\n\
             user_pref(\"network.proxy.no_proxies_on\", \"localhost, 127.0.0.1\");\n"
        )
    } else {
        "user_pref(\"nai.proxy.managed\", true);\n\
         user_pref(\"network.proxy.type\", 0);\n\
         user_pref(\"network.proxy.socks_remote_dns\", false);\n"
            .to_string()
    };
    fs::write(profile.join("user.js"), body).map_err(|error| error.to_string())
}

fn encode_clash_name(name: &str) -> String {
    let mut out = String::new();
    for byte in name.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn clash_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())
}

fn clash_auth(request: reqwest::RequestBuilder, secret: &str) -> reqwest::RequestBuilder {
    if secret.is_empty() {
        request
    } else {
        request.header("Authorization", format!("Bearer {secret}"))
    }
}

async fn clash_get_proxies(endpoint: &ClashEndpoint) -> Result<Value, String> {
    let client = clash_client()?;
    let response = clash_auth(
        client.get(format!("http://{}/proxies", endpoint.controller)),
        &endpoint.secret,
    )
    .send()
    .await
    .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("读取节点失败 HTTP {}", response.status()));
    }
    response.json().await.map_err(|error| error.to_string())
}

fn is_freecat_group(name: &str) -> bool {
    name.contains("自由猫") || name.to_ascii_lowercase().contains("freecat")
}

fn is_exit_node(item: &Value) -> bool {
    let kind = item.get("type").and_then(|value| value.as_str()).unwrap_or("");
    !matches!(
        kind,
        "Selector" | "URLTest" | "Fallback" | "LoadBalance" | "Direct" | "Reject" | "Pass" | "Compatible" | "Relay"
    )
}

fn is_selector(item: &Value) -> bool {
    item.get("type").and_then(|value| value.as_str()) == Some("Selector")
}

fn pick_clash_group(proxies: &serde_json::Map<String, Value>) -> Option<(String, Value)> {
    if let Some((name, item)) = proxies
        .iter()
        .find(|(name, item)| is_freecat_group(name) && is_selector(item))
    {
        return Some((name.clone(), item.clone()));
    }
    let preferred = ["节点选择", "PROXY", "🚀 节点选择", "🔰 节点选择"];
    for name in preferred {
        if let Some(item) = proxies.get(name) {
            if is_selector(item) {
                return Some((name.to_string(), item.clone()));
            }
        }
    }
    proxies
        .iter()
        .filter(|(name, item)| *name != "GLOBAL" && is_selector(item))
        .max_by_key(|(_, item)| item.get("all").and_then(|value| value.as_array()).map(|rows| rows.len()).unwrap_or(0))
        .map(|(name, item)| (name.clone(), item.clone()))
}

fn is_info_node(name: &str) -> bool {
    ["剩余流量", "重置", "到期", "网址", "提示", "套餐"]
        .iter()
        .any(|mark| name.contains(mark))
}

fn clash_nodes_from(value: &Value, product: &str) -> ClashNodes {
    let proxies = value
        .get("proxies")
        .and_then(|item| item.as_object())
        .cloned()
        .unwrap_or_default();
    let Some((group, item)) = pick_clash_group(&proxies) else {
        return ClashNodes {
            ok: false,
            message: format!("{product} 里没有可选的节点组"),
            group: String::new(),
            current: String::new(),
            nodes: Vec::new(),
            product: product.to_string(),
            choices: Vec::new(),
        };
    };
    let current = item.get("now").and_then(|value| value.as_str()).unwrap_or("").to_string();
    let mut nodes = Vec::new();
    if let Some(all) = item.get("all").and_then(|value| value.as_array()) {
        for name in all.iter().filter_map(|value| value.as_str()) {
            if name == "自动选择" {
                nodes.push(name.to_string());
                continue;
            }
            if matches!(name, "DIRECT" | "REJECT" | "GLOBAL" | "PASS") || is_info_node(name) {
                continue;
            }
            if proxies.get(name).is_some_and(is_exit_node) {
                nodes.push(name.to_string());
            }
        }
    }
    ClashNodes {
        ok: true,
        message: format!("已连接{product}，当前节点 {current}"),
        group,
        current,
        nodes,
        product: product.to_string(),
        choices: Vec::new(),
    }
}

struct ProxySource {
    id: String,
    endpoint: ClashEndpoint,
    proxies: Value,
    product: String,
}

fn proxy_product_path() -> PathBuf {
    electron_root().join("data").join("proxy_product.json")
}

fn saved_proxy_product() -> String {
    let value: Value = fs::read_to_string(proxy_product_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    match value.get("product").and_then(|item| item.as_str()).unwrap_or("") {
        "freecat" | "clash" => value["product"].as_str().unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn save_proxy_product(product: &str) -> Result<(), String> {
    let path = proxy_product_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, serde_json::to_string_pretty(&json!({ "product": product })).unwrap_or_else(|_| "{}".into()))
        .map_err(|error| error.to_string())
}

fn controller_key(value: &str) -> String {
    let value = value.trim().trim_start_matches("http://").trim_end_matches('/');
    let (host, port) = value.rsplit_once(':').unwrap_or((value, ""));
    let host = if host.eq_ignore_ascii_case("localhost") { "127.0.0.1" } else { host };
    format!("{}:{}", host.to_ascii_lowercase(), port)
}

fn product_offline_message(product: &str) -> String {
    if product == "clash" {
        "Clash Verge 没有在运行。请先打开 Clash Verge。".into()
    } else {
        "自由猫没有在运行。请先打开桌面上的自由猫。".into()
    }
}

fn freecat_controller_message() -> String {
    "自由猫已经打开，但还没开启外部控制器。请在自由猫设置里打开「外部控制器」（端口 9090），然后点刷新。".into()
}

fn process_exists(image: &str) -> bool {
    let output = match std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    String::from_utf8_lossy(&output.stdout)
        .to_ascii_lowercase()
        .contains(&image.to_ascii_lowercase())
}

fn tcp_open(port: u16) -> bool {
    if port == 0 {
        return false;
    }
    let address = format!("127.0.0.1:{port}");
    let Ok(address) = address.parse() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok()
}

fn freecat_app_open(mixed_port: u16) -> bool {
    process_exists("ziyoumao.exe") || tcp_open(mixed_port)
}

async fn probe_product(id: &str, name: &str, endpoint: ClashEndpoint) -> Option<ProxySource> {
    let proxies = clash_get_proxies(&endpoint).await.ok()?;
    let listed = clash_nodes_from(&proxies, name);
    if !listed.ok {
        return None;
    }
    Some(ProxySource {
        id: id.to_string(),
        endpoint,
        proxies,
        product: name.to_string(),
    })
}

async fn available_sources() -> (Vec<ProxyChoice>, Vec<ProxySource>) {
    let freecat_endpoint = freecat_endpoint();
    let freecat_mixed_port = freecat_endpoint.mixed_port;
    let clash_endpoint = clash_verge_endpoint();
    let same = controller_key(&freecat_endpoint.controller) == controller_key(&clash_endpoint.controller);
    let (freecat, clash) = tokio::join!(
        probe_product("freecat", "自由猫", freecat_endpoint),
        async {
            if same {
                None
            } else {
                probe_product("clash", "Clash Verge", clash_endpoint).await
            }
        }
    );
    let freecat_running = freecat.is_some() || freecat_app_open(freecat_mixed_port);
    let choices = vec![
        ProxyChoice { id: "freecat".into(), name: "自由猫".into(), running: freecat_running },
        ProxyChoice { id: "clash".into(), name: "Clash Verge".into(), running: clash.is_some() },
    ];
    let mut sources = Vec::new();
    if let Some(source) = freecat {
        sources.push(source);
    }
    if let Some(source) = clash {
        sources.push(source);
    }
    (choices, sources)
}

fn nodes_for(saved: &str, choices: Vec<ProxyChoice>, sources: &[ProxySource]) -> ClashNodes {
    if let Some(source) = sources.iter().find(|item| item.id == saved) {
        let mut listed = clash_nodes_from(&source.proxies, &source.product);
        listed.product = saved.to_string();
        listed.choices = choices;
        return listed;
    }
    let app_open = choices.iter().any(|item| item.id == saved && item.running);
    let message = if saved.is_empty() {
        "请选择用自由猫还是 Clash Verge".to_string()
    } else if saved == "freecat" && app_open {
        freecat_controller_message()
    } else {
        product_offline_message(saved)
    };
    ClashNodes {
        ok: false,
        message,
        group: String::new(),
        current: String::new(),
        nodes: Vec::new(),
        product: saved.to_string(),
        choices,
    }
}

async fn load_proxy_source() -> Result<ProxySource, String> {
    let saved = saved_proxy_product();
    if saved.is_empty() {
        return Err("请先选择用自由猫还是 Clash Verge".into());
    }
    let (choices, sources) = available_sources().await;
    if !choices.iter().any(|item| item.id == saved && item.running) {
        return Err(product_offline_message(&saved));
    }
    sources.into_iter().find(|item| item.id == saved).ok_or_else(|| {
        if saved == "freecat" {
            freecat_controller_message()
        } else {
            product_offline_message(&saved)
        }
    })
}

async fn ensure_mixed_port(endpoint: &ClashEndpoint, freecat: bool) -> Result<u16, String> {
    let client = clash_client()?;
    let response = clash_auth(
        client.get(format!("http://{}/configs", endpoint.controller)),
        &endpoint.secret,
    )
    .send()
    .await
    .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        if let Ok(value) = response.json::<Value>().await {
            let port = value.get("mixed-port").and_then(|item| item.as_u64()).unwrap_or(0);
            if (1..=u16::MAX as u64).contains(&port) {
                return Ok(port as u16);
            }
        }
    }
    if !freecat {
        return Ok(endpoint.mixed_port);
    }
    let port = if endpoint.mixed_port > 0 {
        endpoint.mixed_port
    } else {
        FREECAT_MIXED_PORT
    };
    let client = clash_client()?;
    let response = clash_auth(
        client
            .patch(format!("http://{}/configs", endpoint.controller))
            .json(&json!({ "mixed-port": port })),
        &endpoint.secret,
    )
    .send()
    .await
    .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("打开自由猫本地端口失败 HTTP {}", response.status()));
    }
    Ok(port)
}

async fn select_clash_node(node: &str) -> Result<(String, u16), String> {
    let source = load_proxy_source().await?;
    let listed = clash_nodes_from(&source.proxies, &source.product);
    if !listed.ok {
        return Err(listed.message);
    }
    if !listed.nodes.iter().any(|name| name == node) {
        return Err(format!("{} 里没有节点 {node}", source.product));
    }
    let client = clash_client()?;
    let response = clash_auth(
        client
            .put(format!(
                "http://{}/proxies/{}",
                source.endpoint.controller,
                encode_clash_name(&listed.group)
            ))
            .json(&json!({ "name": node })),
        &source.endpoint.secret,
    )
    .send()
    .await
    .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("切换{}节点失败 HTTP {}", source.product, response.status()));
    }
    let port = ensure_mixed_port(&source.endpoint, source.product == "自由猫").await?;
    Ok((format!("已切换到 {node}，浏览器走 127.0.0.1:{port}"), port))
}

fn launch_proxy_port(account_id: &str) -> Option<u16> {
    let path = electron_root().join("data").join("firefox_proxy.json");
    let map: Value = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())?;
    let port = map.get(account_id)?.get("port")?.as_u64()?;
    if (1..=u16::MAX as u64).contains(&port) {
        Some(port as u16)
    } else {
        None
    }
}

fn write_launch_proxy(account_id: &str, port: u16, node: &str) -> Result<(), String> {
    let path = electron_root().join("data").join("firefox_proxy.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut map = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or_else(|| json!({}));
    if !map.is_object() {
        map = json!({});
    }
    if let Some(object) = map.as_object_mut() {
        object.insert(account_id.to_string(), json!({ "port": port, "node": node }));
    }
    fs::write(path, serde_json::to_string_pretty(&map).unwrap_or_else(|_| "{}".into()))
        .map_err(|error| error.to_string())
}

async fn apply_account_proxy(account_id: &str) -> Result<String, String> {
    let node = proxy_name_for(account_id);
    let profile = profile_dir_of(account_id);
    if node.is_empty() {
        write_launch_proxy(account_id, 0, "")?;
        if let Some(profile) = profile.as_ref() {
            write_firefox_proxy(profile, None)?;
        }
        return Ok("本机直连".into());
    }
    let (switched, port) = select_clash_node(&node).await?;
    write_launch_proxy(account_id, port, &node)?;
    if let Some(profile) = profile.as_ref() {
        write_firefox_proxy(profile, Some(port))?;
    }
    Ok(switched)
}

#[tauri::command]
pub async fn ruyi_clash_nodes() -> ClashNodes {
    let saved = saved_proxy_product();
    let (choices, sources) = available_sources().await;
    nodes_for(&saved, choices, &sources)
}

#[tauri::command]
pub async fn ruyi_proxy_product_set(product: String) -> Result<ClashNodes, String> {
    let product = match product.trim() {
        "freecat" | "clash" => product.trim().to_string(),
        _ => return Err("请选择自由猫或 Clash Verge".into()),
    };
    let (choices, sources) = available_sources().await;
    if !choices.iter().any(|item| item.id == product && item.running) {
        return Err(product_offline_message(&product));
    }
    save_proxy_product(&product)?;
    Ok(nodes_for(&product, choices, &sources))
}

#[tauri::command]
pub fn ruyi_proxy_map() -> Value {
    read_proxy_map()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitIp {
    pub ip: String,
    pub via: String,
    pub check_url: String,
}

async fn lookup_public_ip(proxy: Option<&str>) -> Result<String, String> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(15));
    builder = if let Some(proxy) = proxy {
        builder.proxy(reqwest::Proxy::all(proxy).map_err(|error| error.to_string())?)
    } else {
        builder.no_proxy()
    };
    let client = builder.build().map_err(|error| error.to_string())?;
    let mut last = String::from("没有响应");
    for url in ["https://api.ipify.org?format=json", "https://api.ip.sb/ip"] {
        match client.get(url).header("User-Agent", "curl/8.0").send().await {
            Ok(response) if response.status().is_success() => {
                let text = response.text().await.unwrap_or_default();
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if let Some(ip) = value.get("ip").and_then(|item| item.as_str()) {
                        let ip = ip.trim();
                        if !ip.is_empty() {
                            return Ok(ip.to_string());
                        }
                    }
                }
                let ip = text.trim();
                if !ip.is_empty() && ip.len() < 64 && !ip.contains('<') && !ip.contains(' ') {
                    return Ok(ip.to_string());
                }
                last = text.chars().take(80).collect();
            }
            Ok(response) => last = format!("HTTP {}", response.status()),
            Err(error) => last = error.to_string(),
        }
    }
    Err(format!("查不到出口 IP：{last}"))
}

#[tauri::command]
pub async fn ruyi_exit_ip(account_id: String) -> Result<ExitIp, String> {
    let account_id = account_id.trim();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    let node = proxy_name_for(account_id);
    let check_url = "https://ip.sb".to_string();
    if node.is_empty() {
        let ip = lookup_public_ip(None).await?;
        return Ok(ExitIp {
            ip,
            via: "本机直连".into(),
            check_url,
        });
    }
    let (_, port) = select_clash_node(&node).await?;
    let proxy = format!("http://127.0.0.1:{port}");
    let ip = lookup_public_ip(Some(&proxy)).await?;
    Ok(ExitIp {
        ip,
        via: node,
        check_url,
    })
}

#[tauri::command]
pub async fn ruyi_node_ip(proxy_name: String) -> Result<ExitIp, String> {
    let node = proxy_name.trim();
    let check_url = "https://ip.sb".to_string();
    if node.is_empty() {
        let ip = lookup_public_ip(None).await?;
        return Ok(ExitIp {
            ip,
            via: "本机直连".into(),
            check_url,
        });
    }
    let (_, port) = select_clash_node(node).await?;
    let proxy = format!("http://127.0.0.1:{port}");
    let ip = lookup_public_ip(Some(&proxy)).await?;
    Ok(ExitIp {
        ip,
        via: node.to_string(),
        check_url,
    })
}

#[tauri::command]
pub fn ruyi_proxy_set(account_id: String, proxy_name: String) -> Result<(), String> {
    let account_id = account_id.trim();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    save_proxy_name(account_id, proxy_name.trim())
}

async fn ensure_script() -> Result<(), String> {
    tokio::task::spawn_blocking(ensure_started)
        .await
        .map_err(|error| error.to_string())??;
    Ok(())
}

fn type_matches(row_type: &str, kind: &str) -> bool {
    if row_type.eq_ignore_ascii_case(kind) {
        return true;
    }
    kind.eq_ignore_ascii_case("x") && row_type.eq_ignore_ascii_case("twitter")
}

fn newest_account_id_of_type(kind: &str) -> Option<String> {
    let store = read_accounts().ok()?;
    let accounts = store.get("accounts")?.as_array()?;
    accounts.iter().rev().find_map(|row| {
        let row_type = row.get("type").and_then(|value| value.as_str()).unwrap_or("");
        if !type_matches(row_type, kind) {
            return None;
        }
        row.get("account_id")
            .and_then(|value| value.as_str())
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string())
    })
}

async fn open_typed_account(account_id: &str, kind: &str) -> Result<Value, String> {
    let proxy = apply_account_proxy(account_id).await?;
    let opened = post_json(
        "/run/browser/open",
        json!({ "account_id": account_id, "type": kind }),
    )
    .await?;
    Ok(json!({ "opened": opened, "proxy": proxy }))
}

async fn create_typed_account(
    kind: &str,
    label: String,
    cookie: String,
    proxy_name: String,
) -> Result<Value, String> {
    ensure_script().await?;
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err("请输入备注名".into());
    }
    let mut body = json!({ "label": label, "type": kind, "open_login": false });
    let cookie = cookie.trim().to_string();
    if !cookie.is_empty() {
        body["cookie_header"] = json!(cookie);
    }
    let created = post_json("/run/browser/accounts", body).await?;
    let account_id = find_account_id(&created).or_else(|| newest_account_id_of_type(kind));
    let Some(account_id) = account_id else {
        return Err("账号已提交，但没有拿到账号编号".into());
    };
    save_proxy_name(&account_id, proxy_name.trim())?;
    if proxy_name.trim().is_empty() {
        if let Some(profile) = profile_dir_in(&created).or_else(|| profile_dir_of(&account_id)) {
            let _ = write_firefox_proxy(&profile, None);
        }
    }
    let opened = open_typed_account(&account_id, kind).await?;
    let via = if proxy_name.trim().is_empty() {
        "本机直连".to_string()
    } else {
        proxy_name.trim().to_string()
    };
    let ip = if proxy_name.trim().is_empty() {
        lookup_public_ip(None).await.unwrap_or_default()
    } else if let Some(port) = launch_proxy_port(&account_id) {
        lookup_public_ip(Some(&format!("http://127.0.0.1:{port}"))).await.unwrap_or_default()
    } else {
        String::new()
    };
    Ok(json!({
        "created": created,
        "accountId": account_id,
        "ip": ip,
        "via": via,
        "proxy": opened.get("proxy").and_then(|value| value.as_str()).unwrap_or(""),
    }))
}

#[tauri::command]
pub async fn ruyi_twitter_create(label: String, cookie: String, proxy_name: String) -> Result<Value, String> {
    create_typed_account(TWITTER_TYPE, label, cookie, proxy_name).await
}

async fn open_twitter_account(account_id: &str, headless: bool) -> Result<Value, String> {
    ensure_script().await?;
    let proxy = apply_account_proxy(account_id).await?;
    let opened = post_json(
        "/run/browser/open",
        json!({ "account_id": account_id, "type": TWITTER_TYPE, "headless": headless }),
    )
    .await?;
    Ok(json!({ "opened": opened, "proxy": proxy }))
}

#[tauri::command]
pub async fn ruyi_twitter_update(account_id: String, label: String, cookie: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    post_json(
        "/run/browser/accounts/update",
        json!({
            "account_id": account_id,
            "label": label.trim(),
            "cookie_header": cookie.trim(),
        }),
    )
    .await
}

#[tauri::command]
pub async fn ruyi_twitter_delete(account_id: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    post_json("/run/browser/accounts/delete", json!({ "account_id": account_id })).await
}

#[tauri::command]
pub async fn ruyi_twitter_refresh(account_id: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    post_json(
        "/run/browser/refresh",
        json!({ "account_id": account_id, "type": TWITTER_TYPE }),
    )
    .await
}

#[tauri::command]
pub async fn ruyi_twitter_open(account_id: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    if nurture_blocks_open(&account_id) {
        return Err("这个账号正在养号，完成或取消后才能打开".into());
    }
    open_twitter_account(&account_id, false).await
}

const PIXIV_TYPE: &str = "pixiv";

#[tauri::command]
pub async fn ruyi_pixiv_create(label: String, cookie: String, proxy_name: String) -> Result<Value, String> {
    create_typed_account(PIXIV_TYPE, label, cookie, proxy_name).await
}

#[tauri::command]
pub async fn ruyi_pixiv_update(account_id: String, label: String, cookie: String) -> Result<Value, String> {
    ruyi_twitter_update(account_id, label, cookie).await
}

#[tauri::command]
pub async fn ruyi_pixiv_delete(account_id: String) -> Result<Value, String> {
    ruyi_twitter_delete(account_id).await
}

#[tauri::command]
pub async fn ruyi_pixiv_refresh(account_id: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    post_json(
        "/run/browser/refresh",
        json!({ "account_id": account_id, "type": PIXIV_TYPE }),
    )
    .await
}

#[tauri::command]
pub async fn ruyi_pixiv_open(account_id: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    open_typed_account(&account_id, PIXIV_TYPE).await
}

fn pixiv_api_error(value: &Value, fallback: &str) -> Result<(), String> {
    if value.get("ok").and_then(|item| item.as_bool()) == Some(false) {
        let message = value
            .get("error")
            .and_then(|item| item.as_str())
            .unwrap_or(fallback);
        return Err(message.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn ruyi_pixiv_likes(account_id: String, mode: Option<String>) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    let mode = match mode.unwrap_or_else(|| "latest".to_string()).as_str() {
        "cache" => "cache",
        "more" => "more",
        "peek" => "peek",
        _ => "latest",
    };
    let value = post_json(
        "/run/browser/pixiv/likes",
        json!({ "account_id": account_id, "mode": mode }),
    )
    .await?;
    pixiv_api_error(&value, "读取点赞失败")?;
    Ok(value)
}

#[tauri::command]
pub async fn ruyi_pixiv_stats(account_id: String, start: String, end: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    let value = post_json(
        "/run/browser/pixiv/stats",
        json!({ "account_id": account_id, "start": start.trim(), "end": end.trim() }),
    )
    .await?;
    pixiv_api_error(&value, "读取每日数据失败")?;
    Ok(value)
}

#[tauri::command]
pub async fn ruyi_pixiv_open_tab(account_id: String, url: String) -> Result<Value, String> {
    ensure_script().await?;
    let account_id = account_id.trim().to_string();
    if account_id.is_empty() {
        return Err("缺少账号编号".into());
    }
    let value = post_json(
        "/run/browser/pixiv/open-tab",
        json!({ "account_id": account_id, "url": url.trim() }),
    )
    .await?;
    pixiv_api_error(&value, "打开帖子失败")?;
    Ok(value)
}

const SCRIPT_ASSET: &str = "ruyipage-server.exe";
const SCRIPT_REPO: &str = "caiweida/web-novelai-ui";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPackageStatus {
    pub present: bool,
    pub version: String,
    pub latest: String,
    pub update_available: bool,
    pub path: String,
    pub bytes: u64,
    pub download_url: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptDownloadProgress {
    received: u64,
    total: u64,
    percent: f64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ScriptRelease {
    tag_name: String,
    assets: Vec<ScriptAsset>,
}

#[derive(Debug, Deserialize)]
struct ScriptAsset {
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

struct RemoteScript {
    version: String,
    url: String,
    bytes: u64,
}

fn script_exe() -> PathBuf {
    electron_root().join(SCRIPT_ASSET)
}

fn script_version_path() -> PathBuf {
    electron_root().join("ruyipage-server.version")
}

fn local_script_version() -> String {
    fs::read_to_string(script_version_path())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn script_status(remote: Option<&RemoteScript>) -> ScriptPackageStatus {
    let path = script_exe();
    let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let present = path.is_file() && bytes > 1024 * 1024;
    let version = if present { local_script_version() } else { String::new() };
    let latest = remote.map(|item| item.version.clone()).unwrap_or_default();
    let download_url = remote.map(|item| item.url.clone()).unwrap_or_else(|| {
        format!("https://github.com/{SCRIPT_REPO}/releases/latest/download/{SCRIPT_ASSET}")
    });
    let update_available = match remote {
        Some(_) if !present => true,
        Some(item) => version.is_empty() || crate::update::version_newer(&item.version, &version),
        None => !present,
    };
    let label = if version.is_empty() { "已就绪".to_string() } else { version.clone() };
    let message = if !present {
        "本机还没有脚本包，可以从发行版下载".to_string()
    } else if update_available && !latest.is_empty() {
        format!("发现新脚本包 {latest}")
    } else {
        format!("脚本包 {label} 已在本机")
    };
    ScriptPackageStatus {
        present,
        version,
        latest,
        update_available,
        path: path.to_string_lossy().to_string(),
        bytes,
        download_url,
        message,
    }
}

async fn fetch_remote_script() -> Result<RemoteScript, String> {
    let client = crate::nai::http_client()?;
    let url = format!("https://api.github.com/repos/{SCRIPT_REPO}/releases/latest");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Langbai-NovelAI-Studio")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("读取发行版失败 HTTP {}", response.status()));
    }
    let release: ScriptRelease = response.json().await.map_err(|error| error.to_string())?;
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(SCRIPT_ASSET))
        .ok_or_else(|| format!("发行版 {} 里还没有 {SCRIPT_ASSET}", release.tag_name))?;
    Ok(RemoteScript {
        version: release.tag_name.trim().trim_start_matches(['v', 'V']).to_string(),
        url: asset.browser_download_url,
        bytes: asset.size.unwrap_or(0),
    })
}

fn replace_script_file(partial: &Path, dest: &Path, version: &str) -> Result<(), String> {
    stop_if_spawned();
    for _ in 0..15 {
        if !dest.exists() {
            break;
        }
        if fs::remove_file(dest).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if dest.exists() {
        return Err("脚本包正在运行，暂时无法替换".into());
    }
    fs::rename(partial, dest).map_err(|error| error.to_string())?;
    fs::write(script_version_path(), version.as_bytes()).map_err(|error| error.to_string())?;
    Ok(())
}

fn emit_script_progress(app: &AppHandle, received: u64, total: u64, message: impl Into<String>) {
    let percent = if total == 0 {
        0.0
    } else {
        (received as f64 / total as f64) * 100.0
    };
    let _ = app.emit(
        "script-package-download",
        ScriptDownloadProgress {
            received,
            total,
            percent,
            message: message.into(),
        },
    );
}

async fn download_script_file(app: &AppHandle, remote: &RemoteScript, dest: &Path) -> Result<(), String> {
    let client = crate::nai::http_client_no_redirect()?;
    let partial = dest.with_extension("part");
    let mut url = remote.url.clone();
    for _ in 0..8 {
        let response = client.get(&url).send().await.map_err(|error| error.to_string())?;
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
            let base = reqwest::Url::parse(&url).map_err(|error| error.to_string())?;
            url = base.join(&location).map_err(|error| error.to_string())?.to_string();
            continue;
        }
        if !status.is_success() {
            return Err(format!("下载脚本包失败 HTTP {}", status.as_u16()));
        }
        if let Some(parent) = partial.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let total = response.content_length().unwrap_or(remote.bytes);
        let mut file = File::create(&partial).map_err(|error| error.to_string())?;
        let mut stream = response.bytes_stream();
        let mut received = 0u64;
        emit_script_progress(app, 0, total, "正在下载脚本包");
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;
            received += chunk.len() as u64;
            file.write_all(&chunk).map_err(|error| error.to_string())?;
            if received == chunk.len() as u64 || received % (8 * 1024 * 1024) < chunk.len() as u64 {
                emit_script_progress(app, received, total, "正在下载脚本包");
            }
        }
        drop(file);
        if received < 1024 * 1024 {
            let _ = fs::remove_file(&partial);
            return Err("下载到的脚本包太小".into());
        }
        let dest_path = dest.to_path_buf();
        let partial_path = partial.clone();
        let version = remote.version.clone();
        tokio::task::spawn_blocking(move || replace_script_file(&partial_path, &dest_path, &version))
            .await
            .map_err(|error| error.to_string())??;
        emit_script_progress(app, received, total.max(received), "脚本包已下载");
        return Ok(());
    }
    Err("下载跳转次数过多".into())
}

#[tauri::command]
pub fn ruyi_script_status() -> ScriptPackageStatus {
    script_status(None)
}

#[tauri::command]
pub async fn ruyi_script_check() -> Result<ScriptPackageStatus, String> {
    let remote = fetch_remote_script().await?;
    Ok(script_status(Some(&remote)))
}

#[tauri::command]
pub async fn ruyi_script_download(app: AppHandle) -> Result<ScriptPackageStatus, String> {
    let remote = fetch_remote_script().await?;
    let dest = script_exe();
    download_script_file(&app, &remote, &dest).await?;
    let _ = tokio::task::spawn_blocking(ensure_started).await;
    Ok(script_status(Some(&remote)))
}

const FIREFOX_ZIP_NAME: &str = "firefox-155.0a1.en-US.win64-20260803.zip";
const FIREFOX_ZIP_URL: &str = "https://github.com/LoseNine/firefox-fingerprintBrowser/releases/download/151-5/firefox-155.0a1.en-US.win64-20260803.zip";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirefoxBrowserStatus {
    pub present: bool,
    pub path: String,
    pub bytes: u64,
    pub message: String,
}

fn firefox_browser_status() -> FirefoxBrowserStatus {
    let path = firefox_exe(&electron_root());
    let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let present = path.is_file() && bytes > 1024 * 1024;
    let message = if present {
        "electron 里已经有指纹浏览器".to_string()
    } else {
        "electron 里没有指纹浏览器，可以下载 firefox-155.0a1.en-US.win64-20260803.zip 并解压".to_string()
    };
    FirefoxBrowserStatus {
        present,
        path: path.to_string_lossy().to_string(),
        bytes,
        message,
    }
}

fn find_named_file(dir: &Path, name: &str, depth: u8) -> Option<PathBuf> {
    if depth > 6 || !dir.is_dir() {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|item| item.to_str()) == Some(name) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_named_file(&path, name, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

fn install_extracted_firefox(staging: &Path, dest: &Path) -> Result<(), String> {
    let exe = find_named_file(staging, "firefox.exe", 0).ok_or_else(|| "压缩包里没有 firefox.exe".to_string())?;
    let source = exe
        .parent()
        .ok_or_else(|| "firefox.exe 没有所在目录".to_string())?
        .to_path_buf();
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|error| error.to_string())?;
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if source == staging {
        fs::rename(&source, dest).map_err(|error| error.to_string())?;
        return Ok(());
    }
    fs::rename(&source, dest).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(staging);
    Ok(())
}

fn emit_firefox_progress(app: &AppHandle, received: u64, total: u64, message: impl Into<String>) {
    let percent = if total == 0 {
        0.0
    } else {
        (received as f64 / total as f64) * 100.0
    };
    let _ = app.emit(
        "firefox-browser-download",
        ScriptDownloadProgress {
            received,
            total,
            percent,
            message: message.into(),
        },
    );
}

async fn download_firefox_zip(app: &AppHandle, partial: &Path) -> Result<(), String> {
    let client = crate::nai::http_client_no_redirect()?;
    let mut url = FIREFOX_ZIP_URL.to_string();
    for _ in 0..8 {
        let response = client
            .get(&url)
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .map_err(|error| error.to_string())?;
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
            let base = reqwest::Url::parse(&url).map_err(|error| error.to_string())?;
            url = base.join(&location).map_err(|error| error.to_string())?.to_string();
            continue;
        }
        if !status.is_success() {
            return Err(format!("下载指纹浏览器失败 HTTP {}", status.as_u16()));
        }
        if let Some(parent) = partial.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let total = response.content_length().unwrap_or(0);
        let mut file = File::create(partial).map_err(|error| error.to_string())?;
        let mut stream = response.bytes_stream();
        let mut received = 0u64;
        emit_firefox_progress(app, 0, total, "正在下载指纹浏览器");
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;
            received += chunk.len() as u64;
            file.write_all(&chunk).map_err(|error| error.to_string())?;
            if received == chunk.len() as u64 || received % (8 * 1024 * 1024) < chunk.len() as u64 {
                emit_firefox_progress(app, received, total, "正在下载指纹浏览器");
            }
        }
        drop(file);
        if received < 20 * 1024 * 1024 {
            let _ = fs::remove_file(partial);
            return Err("下载到的指纹浏览器压缩包太小".into());
        }
        emit_firefox_progress(app, received, total.max(received), "压缩包已下载，正在解压");
        return Ok(());
    }
    Err("下载跳转次数过多".into())
}

#[tauri::command]
pub fn ruyi_firefox_status() -> FirefoxBrowserStatus {
    firefox_browser_status()
}

#[tauri::command]
pub async fn ruyi_firefox_download(app: AppHandle) -> Result<FirefoxBrowserStatus, String> {
    if firefox_browser_status().present {
        return Ok(firefox_browser_status());
    }
    let root = electron_root();
    let partial = root.join(format!("{FIREFOX_ZIP_NAME}.part"));
    let staging = root.join("firefox-unpack");
    download_firefox_zip(&app, &partial).await?;
    let partial_path = partial.clone();
    let staging_path = staging.clone();
    let dest = root.join("firefox");
    let unpacked = tokio::task::spawn_blocking(move || {
        if staging_path.exists() {
            let _ = fs::remove_dir_all(&staging_path);
        }
        extract_zip(&partial_path, &staging_path)?;
        install_extracted_firefox(&staging_path, &dest)?;
        let _ = fs::remove_file(&partial_path);
        if staging_path.exists() {
            let _ = fs::remove_dir_all(&staging_path);
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())?;
    unpacked?;
    if !firefox_exe(&root).is_file() {
        return Err("已解压，但 electron/firefox/firefox.exe 不存在".into());
    }
    emit_firefox_progress(&app, 1, 1, "指纹浏览器已解压");
    Ok(firefox_browser_status())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NurtureEvent {
    pub action: String,
    pub handle: String,
    pub name: String,
    pub text: String,
    pub url: String,
    pub at: i64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NurtureTaskStatus {
    pub id: u64,
    pub account_id: String,
    pub label: String,
    pub status: String,
    pub mode: String,
    pub minutes: u32,
    pub likes: u32,
    pub reposts: u32,
    pub follows: u32,
    pub remaining_secs: i64,
    pub message: String,
    pub events: Vec<NurtureEvent>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NurtureStatus {
    pub running: bool,
    pub message: String,
    pub tasks: Vec<NurtureTaskStatus>,
    pub log: Vec<String>,
}

#[derive(Clone)]
struct NurtureTask {
    id: u64,
    account_id: String,
    label: String,
    mode: String,
    feed: String,
    handles: Vec<String>,
    rounds: u32,
    likes: u32,
    reposts: u32,
    follows: u32,
    delay_secs: u32,
    minutes: u32,
    headless: bool,
    status: String,
    deadline: f64,
    message: String,
    events: Vec<NurtureEvent>,
}

struct NurtureState {
    tasks: Vec<NurtureTask>,
    log: Vec<String>,
    message: String,
}

static NURTURE_CHILDREN: LazyLock<Mutex<HashMap<String, Child>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NURTURE_STOP: AtomicBool = AtomicBool::new(false);
static NURTURE_WORKERS: AtomicU32 = AtomicU32::new(0);
static NURTURE_PARALLEL: AtomicU32 = AtomicU32::new(4);
static NURTURE_SEQ: AtomicU64 = AtomicU64::new(1);
static NURTURE: Mutex<NurtureState> = Mutex::new(NurtureState {
    tasks: Vec::new(),
    log: Vec::new(),
    message: String::new(),
});

fn nurture_is_active(status: &str) -> bool {
    status == "queued" || status == "running"
}

fn nurture_blocks_open(account_id: &str) -> bool {
    NURTURE.lock().ok().is_some_and(|state| {
        state.tasks.iter().any(|task| task.account_id == account_id && nurture_is_active(&task.status))
    })
}

fn nurture_snapshot() -> NurtureStatus {
    let Some(state) = NURTURE.lock().ok() else {
        return NurtureStatus {
            running: false,
            message: String::new(),
            tasks: Vec::new(),
            log: Vec::new(),
        };
    };
    let now = chrono::Utc::now().timestamp() as f64;
    let tasks = state
        .tasks
        .iter()
        .map(|task| {
            let remaining_secs = if task.status == "running" {
                (task.deadline - now).ceil().max(0.0) as i64
            } else if task.status == "queued" {
                i64::from(task.minutes) * 60
            } else {
                0
            };
            NurtureTaskStatus {
                id: task.id,
                account_id: task.account_id.clone(),
                label: task.label.clone(),
                status: task.status.clone(),
                mode: task.mode.clone(),
                minutes: task.minutes,
                likes: task.likes,
                reposts: task.reposts,
                follows: task.follows,
                remaining_secs,
                message: task.message.clone(),
                events: task.events.clone(),
            }
        })
        .collect();
    NurtureStatus {
        running: state.tasks.iter().any(|task| nurture_is_active(&task.status)),
        message: state.message.clone(),
        tasks,
        log: state.log.clone(),
    }
}

fn nurture_log(task_id: u64, line: impl Into<String>) {
    let Ok(mut state) = NURTURE.lock() else {
        return;
    };
    let line = line.into();
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    state.message = line.to_string();
    if let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) {
        task.message = line.to_string();
    }
    state.log.push(line.to_string());
    if state.log.len() > 40 {
        let extra = state.log.len() - 40;
        state.log.drain(0..extra);
    }
}

fn nurture_record_event(task_id: u64, payload: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return;
    };
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let action = text("action");
    if action.is_empty() {
        return;
    }
    let event = NurtureEvent {
        action,
        handle: text("handle"),
        name: text("name"),
        text: text("text"),
        url: text("url"),
        at: chrono::Utc::now().timestamp(),
    };
    let Ok(mut state) = NURTURE.lock() else {
        return;
    };
    if let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) {
        task.events.push(event);
        if task.events.len() > 500 {
            let extra = task.events.len() - 500;
            task.events.drain(0..extra);
        }
    }
}

fn nurture_task_status(id: u64) -> String {
    NURTURE
        .lock()
        .ok()
        .and_then(|state| state.tasks.iter().find(|task| task.id == id).map(|task| task.status.clone()))
        .unwrap_or_default()
}

fn nurture_mark(id: u64, status: &str, message: impl Into<String>) {
    let message = message.into();
    let Ok(mut state) = NURTURE.lock() else {
        return;
    };
    if let Some(task) = state.tasks.iter_mut().find(|task| task.id == id) {
        if task.status == "cancelled" && status != "cancelled" {
            return;
        }
        task.status = status.to_string();
        if !message.is_empty() {
            task.message = message;
        }
    }
}

fn nurture_key(account_id: &str) -> String {
    let key: String = account_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    if key.is_empty() { "account".into() } else { key }
}

fn nurture_stop_path_for(account_id: &str) -> PathBuf {
    electron_root().join("data").join(format!("nurture-{}.stop", nurture_key(account_id)))
}

fn nurture_config_path_for(account_id: &str) -> PathBuf {
    electron_root().join("data").join(format!("nurture-job-{}.json", nurture_key(account_id)))
}

fn nurture_log_path_for(account_id: &str) -> PathBuf {
    electron_root().join("data").join(format!("nurture-{}.log", nurture_key(account_id)))
}

fn account_label(account_id: &str) -> String {
    read_accounts()
        .ok()
        .and_then(|store| {
            store.get("accounts")?.as_array()?.iter().find_map(|row| {
                let id = row.get("account_id").and_then(|value| value.as_str())?;
                if id != account_id {
                    return None;
                }
                let label = row.get("label").and_then(|value| value.as_str()).unwrap_or("");
                let username = row.get("username").and_then(|value| value.as_str()).unwrap_or("");
                let text = if !label.is_empty() { label } else { username };
                Some(if text.is_empty() { account_id.to_string() } else { text.to_string() })
            })
        })
        .unwrap_or_else(|| account_id.to_string())
}

fn firefox_command_lines() -> Vec<String> {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-Command",
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-CimInstance Win32_Process -Filter \"Name='firefox.exe'\" | ForEach-Object { $_.CommandLine }",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let Ok(output) = cmd.output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn debug_port_for_account(account_id: &str) -> Option<u16> {
    let needle = account_id.to_ascii_lowercase();
    for line in firefox_command_lines() {
        if !line.to_ascii_lowercase().contains(&needle) {
            continue;
        }
        let key = "--remote-debugging-port=";
        let Some(index) = line.find(key) else {
            continue;
        };
        let digits: String = line[index + key.len()..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        if let Ok(port) = digits.parse::<u16>() {
            return Some(port);
        }
    }
    None
}

fn clean_handles(raw: &str) -> Result<Vec<String>, String> {
    let mut handles = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if handles.len() >= 20 {
            return Err("一次最多 20 个用户或帖子".into());
        }
        if line.len() > 180 {
            return Err("有一行名单太长".into());
        }
        let username = line.trim_start_matches('@');
        let url = line.starts_with("https://x.com/") || line.starts_with("https://twitter.com/");
        let name_ok = !username.is_empty()
            && username
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if !url && !name_ok {
            return Err(format!("无法识别「{line}」。请填用户名，或 x.com 的主页/帖子链接。"));
        }
        handles.push(line.to_string());
    }
    Ok(handles)
}

fn read_new_nurture_log(task_id: u64, path: &Path, offset: &mut u64) {
    let Ok(file) = fs::read(path) else {
        return;
    };
    if file.len() as u64 <= *offset {
        return;
    }
    let start = *offset as usize;
    *offset = file.len() as u64;
    let text = String::from_utf8_lossy(&file[start..]);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(payload) = line.strip_prefix("@@") {
            nurture_record_event(task_id, payload);
            continue;
        }
        nurture_log(task_id, line);
    }
}

fn claim_next_nurture() -> Option<NurtureTask> {
    let Ok(mut state) = NURTURE.lock() else {
        return None;
    };
    let now = chrono::Utc::now().timestamp() as f64;
    let task = state.tasks.iter_mut().find(|task| task.status == "queued")?;
    task.status = "running".into();
    task.deadline = now + f64::from(task.minutes) * 60.0;
    task.message = "正在打开浏览器".into();
    Some(task.clone())
}

fn nurture_has_queued() -> bool {
    NURTURE
        .lock()
        .ok()
        .is_some_and(|state| state.tasks.iter().any(|task| task.status == "queued"))
}

fn nurture_parallel() -> u32 {
    NURTURE_PARALLEL.load(Ordering::Relaxed).clamp(1, 8)
}

fn ensure_nurture_workers() {
    let want = nurture_parallel();
    loop {
        let current = NURTURE_WORKERS.load(Ordering::Relaxed);
        if current >= want || !nurture_has_queued() || NURTURE_STOP.load(Ordering::Relaxed) {
            return;
        }
        if NURTURE_WORKERS
            .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            tokio::spawn(nurture_worker_once());
        }
    }
}

fn find_python() -> Result<(&'static str, Vec<&'static str>), String> {
    for (program, prefix) in [("py", vec!["-3"]), ("python", Vec::new())] {
        let mut cmd = Command::new(program);
        cmd.args(&prefix)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        if cmd.status().map(|status| status.success()).unwrap_or(false) {
            return Ok((program, prefix));
        }
    }
    Err("没有找到 Python。养号需要本机 Python 才能操作已经打开的指纹浏览器。".into())
}

fn ruyipage_python_root() -> Result<PathBuf, String> {
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ruyipage");
    if bundled.join("ruyipage").join("__init__.py").is_file() {
        return Ok(bundled);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join("ruyipage"), dir.join("resources").join("ruyipage")] {
                if candidate.join("ruyipage").join("__init__.py").is_file() {
                    return Ok(candidate);
                }
            }
        }
    }
    Err("没有找到 ruyipage，暂时不能操作指纹浏览器。".into())
}

fn nurture_script_path() -> Result<PathBuf, String> {
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("nurture_x.py");
    if bundled.is_file() {
        return Ok(bundled);
    }
    let dest = electron_root().join("data").join("nurture_x.py");
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&dest, include_str!("../scripts/nurture_x.py")).map_err(|error| error.to_string())?;
    Ok(dest)
}

fn stop_nurture_child(account_id: &str) {
    let Some(mut child) = NURTURE_CHILDREN.lock().ok().and_then(|mut slot| slot.remove(account_id)) else {
        return;
    };
    let _ = child.kill();
    let _ = child.wait();
}

fn close_firefox_for_account(account_id: &str) {
    let needle = account_id.to_ascii_lowercase();
    if needle.is_empty() {
        return;
    }
    let command = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='firefox.exe'\" | Where-Object {{ $_.CommandLine -and $_.CommandLine.ToLower().Contains('{needle}') }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}"
    );
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &command])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let _ = cmd.status();
}

fn start_nurture_python(account_id: &str, cfg_path: &Path, log_path: &Path) -> Result<(), String> {
    stop_nurture_child(account_id);
    let (program, prefix) = find_python()?;
    let script = nurture_script_path()?;
    let root = ruyipage_python_root()?;
    let log = File::options()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| format!("打开养号日志失败：{error}"))?;
    let log_err = log.try_clone().map_err(|error| format!("打开养号日志失败：{error}"))?;
    let mut cmd = Command::new(program);
    cmd.args(&prefix)
        .arg("-u")
        .arg(&script)
        .arg(cfg_path)
        .env("RUYIPAGE_ROOT", &root)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let child = cmd.spawn().map_err(|error| format!("启动养号脚本失败：{error}"))?;
    if let Ok(mut slot) = NURTURE_CHILDREN.lock() {
        slot.insert(account_id.to_string(), child);
    }
    Ok(())
}

async fn run_nurture_task(task: NurtureTask) {
    let stop_path = nurture_stop_path_for(&task.account_id);
    let config_path = nurture_config_path_for(&task.account_id);
    let log_path = nurture_log_path_for(&task.account_id);
    if nurture_task_status(task.id) == "cancelled" || NURTURE_STOP.load(Ordering::Relaxed) {
        return;
    }
    let _ = fs::remove_file(&config_path);
    let _ = fs::remove_file(&stop_path);
    nurture_log(
        task.id,
        format!(
            "正在{}打开 {}，这次养号 {} 分钟",
            if task.headless { "用无头浏览器" } else { "" },
            task.label,
            task.minutes
        ),
    );
    if task.headless {
        close_firefox_for_account(&task.account_id);
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    if let Err(error) = open_twitter_account(&task.account_id, task.headless).await {
        let message = format!("{} 打开失败：{error}", task.label);
        nurture_mark(task.id, "failed", message.clone());
        nurture_log(task.id, message);
        return;
    }
    let mut port = None;
    for _ in 0..40 {
        if nurture_task_status(task.id) != "running" || NURTURE_STOP.load(Ordering::Relaxed) {
            break;
        }
        if let Some(found) = debug_port_for_account(&task.account_id) {
            port = Some(found);
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let Some(port) = port else {
        if nurture_task_status(task.id) == "running" && !NURTURE_STOP.load(Ordering::Relaxed) {
            let message = format!("{} 的指纹浏览器没有暴露调试端口", task.label);
            nurture_mark(task.id, "failed", message.clone());
            nurture_log(task.id, message);
        }
        return;
    };
    if chrono::Utc::now().timestamp() as f64 >= task.deadline || nurture_task_status(task.id) != "running" {
        if nurture_task_status(task.id) == "running" {
            nurture_mark(task.id, "done", "养号时间到了");
            nurture_log(task.id, format!("{} 养号时间到了", task.label));
        }
        return;
    }
    let cfg = json!({
        "mode": task.mode,
        "account_id": task.account_id,
        "port": port,
        "feed": task.feed,
        "rounds": task.rounds,
        "likes": task.likes,
        "reposts": task.reposts,
        "follows": task.follows,
        "delay": task.delay_secs,
        "minutes": task.minutes,
        "deadline": task.deadline,
        "handles": task.handles,
        "stop": stop_path,
    });
    if let Err(error) = fs::write(&config_path, cfg.to_string()) {
        let message = format!("写养号配置失败：{error}");
        nurture_mark(task.id, "failed", message.clone());
        nurture_log(task.id, message);
        return;
    }
    let _ = fs::write(&log_path, b"");
    if let Err(error) = start_nurture_python(&task.account_id, &config_path, &log_path) {
        nurture_mark(task.id, "failed", error.clone());
        nurture_log(task.id, error);
        return;
    }
    nurture_mark(task.id, "running", format!("养号中，共 {} 分钟", task.minutes));
    nurture_log(task.id, format!("{} 已开始养号", task.label));
    let mut offset = 0u64;
    let mut quiet = 0u32;
    while chrono::Utc::now().timestamp() as f64 <= task.deadline {
        if nurture_task_status(task.id) != "running" || NURTURE_STOP.load(Ordering::Relaxed) {
            break;
        }
        let before = offset;
        read_new_nurture_log(task.id, &log_path, &mut offset);
        if offset == before {
            quiet += 1;
            if quiet == 25 {
                nurture_log(task.id, format!("{} 的窗口还没有开始滚动。请确认这个账号的指纹浏览器已经打开。", task.label));
            }
        } else {
            quiet = 0;
        }
        let exited = NURTURE_CHILDREN.lock().ok().and_then(|mut slot| {
            slot.get_mut(&task.account_id)
                .and_then(|child| child.try_wait().ok().flatten().map(|status| status.code()))
        });
        if let Some(code) = exited {
            if let Ok(mut slot) = NURTURE_CHILDREN.lock() {
                slot.remove(&task.account_id);
            }
            nurture_log(task.id, format!("{} 的养号脚本已结束（退出码 {}）", task.label, code.unwrap_or(-1)));
            break;
        }
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    read_new_nurture_log(task.id, &log_path, &mut offset);
    let status = nurture_task_status(task.id);
    if status == "running" && chrono::Utc::now().timestamp() as f64 >= task.deadline {
        let _ = fs::write(&stop_path, b"stop");
        nurture_mark(task.id, "done", "养号时间到了");
        nurture_log(task.id, format!("{} 养号时间到了", task.label));
    }
    stop_nurture_child(&task.account_id);
}

async fn nurture_worker_once() {
    let task = if NURTURE_STOP.load(Ordering::Relaxed) {
        None
    } else {
        claim_next_nurture()
    };
    if let Some(task) = task {
        run_nurture_task(task).await;
    }
    NURTURE_WORKERS.fetch_sub(1, Ordering::Release);
    if nurture_has_queued() && !NURTURE_STOP.load(Ordering::Relaxed) {
        ensure_nurture_workers();
    }
}

#[tauri::command]
pub async fn ruyi_nurture_start(
    mode: String,
    account_ids: Vec<String>,
    handles: String,
    feed: String,
    rounds: u32,
    likes: u32,
    reposts: u32,
    follows: u32,
    delay_secs: u32,
    minutes: u32,
    parallel: u32,
    headless: bool,
) -> Result<NurtureStatus, String> {
    let mode = match mode.as_str() {
        "browse" | "interact" | "plaza" => mode,
        _ => return Err("养号方式只能是浏览、互动或广场".into()),
    };
    let mut ids = Vec::new();
    for account_id in account_ids {
        let account_id = account_id.trim().to_string();
        if account_id.is_empty() || ids.contains(&account_id) {
            continue;
        }
        ids.push(account_id);
    }
    if ids.is_empty() {
        return Err("请先选择推特账号".into());
    }
    if ids.len() > 20 {
        return Err("一次最多加入 20 个养号任务".into());
    }
    let handles = clean_handles(&handles)?;
    let rounds = rounds.clamp(1, 40);
    let likes = likes.clamp(0, 15);
    let reposts = reposts.clamp(0, 8);
    let follows = follows.clamp(0, 15);
    let delay_secs = delay_secs.clamp(3, 20);
    let minutes = minutes.clamp(1, 180);
    let parallel = parallel.clamp(1, 8);
    NURTURE_PARALLEL.store(parallel, Ordering::Relaxed);
    let feed = if feed == "following" { "following" } else { "for_you" };
    let mut blocked = Vec::new();
    for account_id in &ids {
        if nurture_blocks_open(account_id) {
            blocked.push(account_label(account_id));
        }
    }
    if !blocked.is_empty() {
        return Err(format!("这些账号已在养号队列里：{}", blocked.join("、")));
    }
    let _ = fs::create_dir_all(electron_root().join("data"));
    NURTURE_STOP.store(false, Ordering::Relaxed);
    let mut added = 0u32;
    if let Ok(mut state) = NURTURE.lock() {
        for account_id in ids {
            let id = NURTURE_SEQ.fetch_add(1, Ordering::Relaxed);
            let label = account_label(&account_id);
            state.tasks.retain(|task| !(task.account_id == account_id && task.status == "failed"));
            state.tasks.push(NurtureTask {
                id,
                account_id,
                label,
                mode: mode.clone(),
                feed: feed.to_string(),
                handles: handles.clone(),
                rounds,
                likes,
                reposts,
                follows,
                delay_secs,
                minutes,
                headless,
                status: "queued".into(),
                deadline: 0.0,
                message: format!("等待中，计划 {minutes} 分钟"),
                events: Vec::new(),
            });
            added += 1;
        }
        let finished: Vec<usize> = state
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| !nurture_is_active(&task.status))
            .map(|(index, _)| index)
            .collect();
        let overflow = state.tasks.len().saturating_sub(30);
        for index in finished.into_iter().take(overflow).collect::<Vec<_>>().into_iter().rev() {
            state.tasks.remove(index);
        }
        state.message = format!("已加入 {added} 个养号任务");
        let message = state.message.clone();
        state.log.push(message);
        if state.log.len() > 40 {
            let extra = state.log.len() - 40;
            state.log.drain(0..extra);
        }
    }
    ensure_nurture_workers();
    Ok(nurture_snapshot())
}

fn cancel_nurture_task(task_id: u64) -> Option<String> {
    let Ok(mut state) = NURTURE.lock() else {
        return None;
    };
    let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) else {
        return None;
    };
    if !nurture_is_active(&task.status) {
        return None;
    }
    let running = task.status == "running";
    let account_id = task.account_id.clone();
    let label = task.label.clone();
    task.status = "cancelled".into();
    task.message = "已取消".into();
    state.message = format!("{label} 已取消");
    let message = state.message.clone();
    state.log.push(message);
    if running { Some(account_id) } else { None }
}

#[tauri::command]
pub async fn ruyi_nurture_cancel(task_id: u64) -> Result<NurtureStatus, String> {
    if let Some(account_id) = cancel_nurture_task(task_id) {
        let _ = fs::create_dir_all(electron_root().join("data"));
        let _ = fs::write(nurture_stop_path_for(&account_id), b"stop");
        stop_nurture_child(&account_id);
    }
    Ok(nurture_snapshot())
}

#[tauri::command]
pub async fn ruyi_nurture_stop() -> Result<NurtureStatus, String> {
    NURTURE_STOP.store(true, Ordering::Relaxed);
    let _ = fs::create_dir_all(electron_root().join("data"));
    if let Ok(mut state) = NURTURE.lock() {
        let mut count = 0u32;
        for task in &mut state.tasks {
            if nurture_is_active(&task.status) {
                if task.status == "running" {
                    let _ = fs::write(nurture_stop_path_for(&task.account_id), b"stop");
                    stop_nurture_child(&task.account_id);
                }
                task.status = "cancelled".into();
                task.message = "已取消".into();
                count += 1;
            }
        }
        state.message = if count == 0 {
            "没有正在养号的任务".into()
        } else {
            format!("已取消全部 {count} 个养号任务")
        };
        let message = state.message.clone();
        state.log.push(message);
    }
    Ok(nurture_snapshot())
}

#[tauri::command]
pub fn ruyi_nurture_status() -> NurtureStatus {
    nurture_snapshot()
}
