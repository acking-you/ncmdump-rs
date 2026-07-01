use bilibili_api::auth::BiliSession;
use bilibili_api::BilibiliClient;
use download_core::{
    apply_progress_event, DownloadProgressEvent, DownloadProgressPhase, DownloadProgressReporter,
    DownloadProgressSnapshot,
};
#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};
#[cfg(target_os = "android")]
use jni::JavaVM;
use netease_api::auth::Session as NeteaseSession;
use netease_api::types::{Quality, SearchType};
use netease_api::NeteaseClient;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

const NETEASE_LOGIN_URL: &str = "https://music.163.com/";
const BILIBILI_LOGIN_URL: &str = "https://www.bilibili.com/";
const MAIN_WINDOW_LABEL: &str = "main";
const NCMDUMP_CONFIG_DIR_ENV: &str = "NCMDUMP_CONFIG_DIR";
const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

#[cfg(target_os = "android")]
static ANDROID_EXTERNAL_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Provider {
    Netease,
    Bilibili,
}

#[derive(Debug, Serialize)]
struct ProviderStatus {
    provider: &'static str,
    is_logged_in: bool,
    session_path: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct DownloadResult {
    provider: &'static str,
    saved_path: String,
}

#[derive(Debug, Serialize)]
struct NeteaseSearchItem {
    id: u64,
    name: String,
    artists: String,
    album: String,
}

#[derive(Debug, Serialize)]
struct BilibiliSearchItem {
    bvid: String,
    title: String,
    author: String,
    duration: String,
}

struct TauriProgressReporter {
    app: tauri::AppHandle,
    snapshot: Mutex<Option<DownloadProgressSnapshot>>,
}

impl TauriProgressReporter {
    fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            snapshot: Mutex::new(None),
        }
    }
}

impl DownloadProgressReporter for TauriProgressReporter {
    fn emit(&self, event: DownloadProgressEvent) {
        let Ok(mut snapshot_guard) = self.snapshot.lock() else {
            return;
        };

        let mut snapshot = apply_progress_event(snapshot_guard.take(), event.clone());
        match event.phase {
            DownloadProgressPhase::Completed => {
                snapshot.filename = event.detail.clone();
                snapshot.error = None;
            }
            DownloadProgressPhase::Failed => {
                snapshot.error = Some(event.message.clone());
            }
            _ => {}
        }

        let _ = self.app.emit(DOWNLOAD_PROGRESS_EVENT, snapshot.clone());
        *snapshot_guard = Some(snapshot);
    }
}

fn main_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window not found".to_string())
}

fn app_work_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        let _ = app;
        let base = ANDROID_EXTERNAL_DATA_DIR
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|_| "android external data dir mutex poisoned".to_string())?
            .clone()
            .ok_or_else(|| "android external data dir not initialized".to_string())?;
        fs::create_dir_all(&base).map_err(|e| format!("failed to create app work dir: {e}"))?;
        Ok(base)
    }

    #[cfg(not(target_os = "android"))]
    {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
        fs::create_dir_all(&dir).map_err(|e| format!("failed to create app work dir: {e}"))?;
        Ok(dir)
    }
}

fn app_config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_work_dir(app)?.join("ncmdump");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create app config dir: {e}"))?;
    Ok(dir)
}

#[allow(dead_code)]
fn download_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app_work_dir(app)
}

fn sanitize_file_component(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

fn next_job_id(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{ts}")
}

fn bilibili_output_path(
    app: &tauri::AppHandle,
    title: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let dir = download_dir(app)?;
    Ok(dir.join(format!("{}.{}", sanitize_file_component(title), extension)))
}

fn netease_output_path(
    app: &tauri::AppHandle,
    title: &str,
    quality: Quality,
) -> Result<PathBuf, String> {
    let dir = download_dir(app)?;
    let extension = if matches!(quality, Quality::Lossless) {
        "flac"
    } else {
        "mp3"
    };
    Ok(dir.join(format!("{}.{}", sanitize_file_component(title), extension)))
}

fn set_config_dir_env(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_config_dir(app)?;
    unsafe {
        std::env::set_var(NCMDUMP_CONFIG_DIR_ENV, &dir);
    }
    Ok(dir)
}

fn netease_session_path(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(app_config_dir(app)?
        .join("session.json")
        .display()
        .to_string())
}

fn bilibili_session_path(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(app_config_dir(app)?
        .join("bilibili_session.json")
        .display()
        .to_string())
}

fn load_netease_status(app: &tauri::AppHandle) -> Result<ProviderStatus, String> {
    set_config_dir_env(app)?;
    let session = NeteaseSession::load().map_err(|e| e.to_string())?;
    Ok(ProviderStatus {
        provider: "netease",
        is_logged_in: session.is_logged_in(),
        session_path: netease_session_path(app)?,
        summary: if session.is_logged_in() {
            "MUSIC_U is present".to_string()
        } else {
            "No MUSIC_U saved".to_string()
        },
    })
}

fn load_bilibili_status(app: &tauri::AppHandle) -> Result<ProviderStatus, String> {
    set_config_dir_env(app)?;
    let session = BiliSession::load().map_err(|e| e.to_string())?;
    let mut fields = Vec::new();
    if session.sessdata.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("SESSDATA");
    }
    if session.bili_jct.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("bili_jct");
    }
    if session.dede_user_id.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("DedeUserID");
    }
    if session.buvid3.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("buvid3");
    }
    if session.buvid4.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("buvid4");
    }

    let is_logged_in = session.is_logged_in();
    Ok(ProviderStatus {
        provider: "bilibili",
        is_logged_in,
        session_path: bilibili_session_path(app)?,
        summary: if fields.is_empty() {
            "No Bilibili session cookies saved".to_string()
        } else {
            format!("Saved cookies: {}", fields.join(", "))
        },
    })
}

fn load_status(app: &tauri::AppHandle, provider: Provider) -> Result<ProviderStatus, String> {
    match provider {
        Provider::Netease => load_netease_status(app),
        Provider::Bilibili => load_bilibili_status(app),
    }
}

fn extract_music_u_from_webview(app: &tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        if let Some(value) = extract_music_u_from_android_cookie_manager()? {
            return Ok(value);
        }
    }

    let browser = main_window(app)?;
    let cookies = browser
        .cookies()
        .map_err(|e| format!("failed to read browser cookies: {e}"))?;

    for cookie in cookies {
        let domain = cookie
            .domain_raw()
            .or_else(|| cookie.domain())
            .unwrap_or("")
            .to_ascii_lowercase();

        if !domain.contains("music.163.com") && !domain.contains(".163.com") {
            continue;
        }

        if cookie.name() == "MUSIC_U" {
            let value = cookie.value().to_string();
            if !value.is_empty() {
                return Ok(value);
            }
        }
    }

    Err("MUSIC_U cookie not found in the Tauri webview. Log in on music.163.com in this window first.".to_string())
}

fn extract_bilibili_session_from_webview(app: &tauri::AppHandle) -> Result<BiliSession, String> {
    #[cfg(target_os = "android")]
    {
        let session = extract_bilibili_session_from_android_cookie_manager()?;
        if session.is_logged_in() {
            return Ok(session);
        }
    }

    let browser = main_window(app)?;
    let cookies = browser
        .cookies()
        .map_err(|e| format!("failed to read browser cookies: {e}"))?;

    let mut session = BiliSession::default();

    for cookie in cookies {
        let domain = cookie
            .domain_raw()
            .or_else(|| cookie.domain())
            .unwrap_or("")
            .to_ascii_lowercase();

        if !domain.contains("bilibili.com") {
            continue;
        }

        let value = cookie.value().to_string();
        if value.is_empty() {
            continue;
        }

        match cookie.name() {
            "SESSDATA" => session.sessdata = Some(value),
            "bili_jct" => session.bili_jct = Some(value),
            "DedeUserID" => session.dede_user_id = Some(value),
            "buvid3" => session.buvid3 = Some(value),
            "buvid4" => session.buvid4 = Some(value),
            _ => {}
        }
    }

    if !session.is_logged_in() {
        return Err(
            "SESSDATA cookie not found in the Tauri webview. Log in on bilibili.com in this window first."
                .to_string(),
        );
    }

    Ok(session)
}

#[cfg(target_os = "android")]
fn extract_music_u_from_android_cookie_manager() -> Result<Option<String>, String> {
    for url in [
        "https://music.163.com/",
        "https://y.music.163.com/",
        "https://interface.music.163.com/",
    ] {
        let header = android_cookie_header(url)?;
        for pair in header.split(';') {
            let pair = pair.trim();
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            if name.trim() == "MUSIC_U" {
                let value = value.trim();
                if !value.is_empty() {
                    return Ok(Some(value.to_string()));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "android")]
fn extract_bilibili_session_from_android_cookie_manager() -> Result<BiliSession, String> {
    let mut session = BiliSession::default();
    for url in [
        "https://www.bilibili.com/",
        "https://m.bilibili.com/",
        "https://passport.bilibili.com/",
        "https://api.bilibili.com/",
    ] {
        let header = android_cookie_header(url)?;
        if header.is_empty() {
            continue;
        }
        for pair in header.split(';') {
            let pair = pair.trim();
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match name {
                "SESSDATA" => session.sessdata = Some(value.to_string()),
                "bili_jct" => session.bili_jct = Some(value.to_string()),
                "DedeUserID" => session.dede_user_id = Some(value.to_string()),
                "buvid3" => session.buvid3 = Some(value.to_string()),
                "buvid4" => session.buvid4 = Some(value.to_string()),
                _ => {}
            }
        }
    }
    Ok(session)
}

#[cfg(target_os = "android")]
fn android_cookie_header(url: &str) -> Result<String, String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut _) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach thread: {e}"))?;

    let cookie_manager_class = env
        .find_class("android/webkit/CookieManager")
        .map_err(|e| format!("Failed to find CookieManager: {e}"))?;
    let cookie_manager = env
        .call_static_method(
            cookie_manager_class,
            "getInstance",
            "()Landroid/webkit/CookieManager;",
            &[],
        )
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to get CookieManager instance: {e}"))?;

    let url_string = env
        .new_string(url)
        .map_err(|e| format!("Failed to allocate Java URL string: {e}"))?;
    let value = env
        .call_method(
            &cookie_manager,
            "getCookie",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&JObject::from(url_string))],
        )
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to read cookies for {url}: {e}"))?;

    if value.is_null() {
        return Ok(String::new());
    }

    env.get_string(&value.into())
        .map(|s| s.to_string_lossy().into_owned())
        .map_err(|e| format!("Failed to decode cookie header: {e}"))
}

#[tauri::command]
fn open_login(app: tauri::AppHandle, provider: Provider) -> Result<(), String> {
    let window = main_window(&app)?;
    let url = match provider {
        Provider::Netease => NETEASE_LOGIN_URL,
        Provider::Bilibili => BILIBILI_LOGIN_URL,
    };

    window.set_focus().ok();
    window
        .navigate(url::Url::parse(url).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn capture_login_cookie(
    app: tauri::AppHandle,
    provider: Provider,
) -> Result<ProviderStatus, String> {
    set_config_dir_env(&app)?;
    match provider {
        Provider::Netease => {
            let music_u = extract_music_u_from_webview(&app)?;
            let session = NeteaseSession {
                music_u: Some(music_u),
            };
            session.save().map_err(|e| e.to_string())?;
            load_netease_status(&app)
        }
        Provider::Bilibili => {
            let session = extract_bilibili_session_from_webview(&app)?;
            session.save().map_err(|e| e.to_string())?;
            load_bilibili_status(&app)
        }
    }
}

#[tauri::command]
fn login_status(app: tauri::AppHandle, provider: Provider) -> Result<ProviderStatus, String> {
    load_status(&app, provider)
}

#[tauri::command]
fn logout(app: tauri::AppHandle, provider: Provider) -> Result<ProviderStatus, String> {
    set_config_dir_env(&app)?;
    match provider {
        Provider::Netease => {
            NeteaseSession::clear().map_err(|e| e.to_string())?;
            load_netease_status(&app)
        }
        Provider::Bilibili => {
            BiliSession::clear().map_err(|e| e.to_string())?;
            load_bilibili_status(&app)
        }
    }
}

#[tauri::command]
fn search_netease_tracks(
    app: tauri::AppHandle,
    keyword: String,
) -> Result<Vec<NeteaseSearchItem>, String> {
    set_config_dir_env(&app)?;
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let client = NeteaseClient::new().map_err(|e| e.to_string())?;
    let result = client
        .search(keyword, SearchType::Track, 10, 0)
        .map_err(|e| e.to_string())?;
    Ok(result
        .tracks
        .unwrap_or_default()
        .into_iter()
        .map(|track| NeteaseSearchItem {
            id: track.id,
            name: track.name,
            artists: track
                .artists
                .into_iter()
                .map(|artist| artist.name)
                .collect::<Vec<_>>()
                .join(", "),
            album: track.album.name,
        })
        .collect())
}

#[tauri::command]
fn search_bilibili_videos(
    app: tauri::AppHandle,
    keyword: String,
) -> Result<Vec<BilibiliSearchItem>, String> {
    set_config_dir_env(&app)?;
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let client = BilibiliClient::new().map_err(|e| e.to_string())?;
    let result = client
        .search_video(keyword, 1, 10)
        .map_err(|e| e.to_string())?;
    Ok(result
        .results
        .into_iter()
        .map(|video| BilibiliSearchItem {
            bvid: video.bvid,
            title: video
                .title
                .replace("<em class=\"keyword\">", "")
                .replace("</em>", ""),
            author: video.author,
            duration: video.duration,
        })
        .collect())
}

#[tauri::command]
fn download_bilibili_audio(app: tauri::AppHandle, input: String) -> Result<DownloadResult, String> {
    set_config_dir_env(&app)?;
    let client = BilibiliClient::new().map_err(|e| e.to_string())?;
    let bvid = client.resolve_bvid(&input).map_err(|e| e.to_string())?;
    let detail = client.video_detail(&bvid).map_err(|e| e.to_string())?;
    let output = bilibili_output_path(&app, &detail.title, "m4s")?;
    let reporter: Arc<dyn DownloadProgressReporter> =
        Arc::new(TauriProgressReporter::new(app.clone()));
    let request = bilibili_api::DownloadAudioRequest {
        job_id: next_job_id("bilibili"),
        bvid,
    };
    client
        .download_audio_raw_with_progress(&request, &output, reporter)
        .map_err(|e| e.to_string())?;

    Ok(DownloadResult {
        provider: "bilibili",
        saved_path: output.display().to_string(),
    })
}

#[tauri::command]
fn download_netease_track(app: tauri::AppHandle, track_id: u64) -> Result<DownloadResult, String> {
    set_config_dir_env(&app)?;
    let client = NeteaseClient::new().map_err(|e| e.to_string())?;
    let track = client.track_detail(track_id).map_err(|e| e.to_string())?;
    let output = netease_output_path(&app, &track.name, Quality::Exhigh)?;
    let reporter: Arc<dyn DownloadProgressReporter> =
        Arc::new(TauriProgressReporter::new(app.clone()));
    let request = netease_api::DownloadTrackRequest {
        job_id: next_job_id("netease"),
        track_id,
        quality: Quality::Exhigh,
    };
    client
        .download_track_with_progress(&request, &output, reporter)
        .map_err(|e| e.to_string())?;

    Ok(DownloadResult {
        provider: "netease",
        saved_path: output.display().to_string(),
    })
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ncmdump_tauri_MainActivity_nativeInitAndroidContext(
    mut env: jni::JNIEnv,
    this: JObject,
) {
    let raw_this = this.into_raw();
    let this_for_path = unsafe { JObject::from_raw(raw_this) };

    if let Ok(vm) = env.get_java_vm() {
        let vm_ptr = vm.get_java_vm_pointer();
        unsafe {
            ndk_context::initialize_android_context(vm_ptr as *mut _, raw_this as *mut _);
        }
    }

    if let Ok(dir) = external_files_dir(&mut env, &this_for_path) {
        let slot = ANDROID_EXTERNAL_DATA_DIR.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(dir);
        }
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ncmdump_tauri_MainActivity_nativeReleaseAndroidContext(
    _env: jni::JNIEnv,
    _this: JObject,
) {
    unsafe {
        let _ = std::panic::catch_unwind(|| ndk_context::release_android_context());
    }
    if let Some(slot) = ANDROID_EXTERNAL_DATA_DIR.get() {
        if let Ok(mut guard) = slot.lock() {
            *guard = None;
        }
    }
}

#[cfg(target_os = "android")]
fn external_files_dir(env: &mut jni::JNIEnv<'_>, this: &JObject<'_>) -> Result<PathBuf, String> {
    let null_dir = env
        .call_method(
            this,
            "getExternalFilesDir",
            "(Ljava/lang/String;)Ljava/io/File;",
            &[JValue::Object(&JObject::null())],
        )
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to get external files dir: {e}"))?;
    let path_obj = env
        .call_method(&null_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to resolve external files path: {e}"))?;
    let path = env
        .get_string(&path_obj.into())
        .map_err(|e| format!("Failed to decode external files path: {e}"))?
        .to_string_lossy()
        .into_owned();
    Ok(PathBuf::from(path))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            open_login,
            capture_login_cookie,
            login_status,
            logout,
            search_netease_tracks,
            search_bilibili_videos,
            download_bilibili_audio,
            download_netease_track
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
