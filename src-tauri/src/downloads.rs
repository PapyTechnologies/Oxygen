//! Module de gestion des téléchargements d'Oxygen Browser (Phase 7).
//!
//! Sécurité stricte : nettoyage des noms de fichiers, vérification d'extension dangereuse,
//! confinement au dossier cible, limitation de débit, et exclusion des onglets privés de l'historique.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::webview::DownloadEvent;
use tauri::{AppHandle, Emitter, Manager, Webview};

pub const DANGEROUS_EXTS: &[&str] = &[
    "exe", "msi", "bat", "cmd", "ps1", "vbs", "js", "jse", "wsf", "scr", "com", "pif", "lnk",
    "hta", "reg", "dll", "jar", "appx", "msix",
];

const MAX_HISTORY_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub domain: String,
    pub size_bytes: Option<u64>,
    pub status: String, // "in_progress", "completed", "failed", "cancelled"
    pub created_at: String,
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadsConfig {
    pub download_dir: Option<String>,
    pub ask_location: bool,
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self {
            download_dir: None,
            ask_location: false,
        }
    }
}

static RATE_LIMITER: Mutex<Option<HashMap<String, Vec<Instant>>>> = Mutex::new(None);
static ACTIVE_DOWNLOADS: Mutex<Option<HashMap<String, DownloadItem>>> = Mutex::new(None);

fn check_rate_limit(origin: &str) -> bool {
    let now = Instant::now();
    let mut guard = RATE_LIMITER.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    let timestamps = map.entry(origin.to_string()).or_default();
    timestamps.retain(|t| now.duration_since(*t) < Duration::from_secs(5));
    if timestamps.len() >= 3 {
        false
    } else {
        timestamps.push(now);
        true
    }
}

fn get_app_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn config_path(app: &AppHandle) -> PathBuf {
    let dir = get_app_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    dir.join("downloads_config.json")
}

fn history_path(app: &AppHandle) -> PathBuf {
    let dir = get_app_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    dir.join("downloads.json")
}

pub fn load_config(app: &AppHandle) -> DownloadsConfig {
    let path = config_path(app);
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        DownloadsConfig::default()
    }
}

pub fn save_config(app: &AppHandle, cfg: &DownloadsConfig) -> Result<(), String> {
    let path = config_path(app);
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load_history(app: &AppHandle) -> Vec<DownloadItem> {
    let path = history_path(app);
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<DownloadItem>>(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_history(app: &AppHandle, items: &[DownloadItem]) -> Result<(), String> {
    let path = history_path(app);
    // Filtrer les onglets privés et borner à 200 entrées
    let to_save: Vec<_> = items
        .iter()
        .filter(|i| !i.is_private)
        .take(MAX_HISTORY_ENTRIES)
        .cloned()
        .collect();
    let json = serde_json::to_string_pretty(&to_save).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn add_or_update_history(app: &AppHandle, item: DownloadItem) {
    if item.is_private {
        return; // Jamais enregistré dans l'historique sur disque
    }
    let mut history = load_history(app);
    if let Some(pos) = history.iter().position(|h| h.id == item.id) {
        history[pos] = item;
    } else {
        history.insert(0, item);
    }
    if history.len() > MAX_HISTORY_ENTRIES {
        history.truncate(MAX_HISTORY_ENTRIES);
    }
    let _ = save_history(app, &history);
}

pub fn get_default_download_dir(app: &AppHandle) -> PathBuf {
    let cfg = load_config(app);
    if let Some(ref d) = cfg.download_dir {
        let p = PathBuf::from(d);
        if p.is_dir() {
            return p;
        }
    }
    app.path()
        .download_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Nettoie rigoureusement le nom de fichier pour Windows
pub fn sanitize_filename(suggested: &str) -> String {
    let raw = Path::new(suggested)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "telechargement".to_string());

    let forbidden = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let cleaned: String = raw
        .chars()
        .filter(|c| !forbidden.contains(c) && !c.is_control())
        .collect();

    let mut trimmed = cleaned.trim_end_matches(['.', ' ']).to_string();
    if trimmed.is_empty() || trimmed == ".." || trimmed == "." {
        trimmed = "telechargement".to_string();
    }

    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let stem = Path::new(&trimmed)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| trimmed.clone());
    let ext = Path::new(&trimmed)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();

    let safe_stem = if reserved.iter().any(|&r| stem.eq_ignore_ascii_case(r)) {
        format!("_{stem}")
    } else {
        stem
    };

    let max_len = 150;
    if (safe_stem.len() + ext.len()) > max_len {
        let max_stem_len = max_len.saturating_sub(ext.len()).max(1);
        let truncated_stem: String = safe_stem.chars().take(max_stem_len).collect();
        format!("{truncated_stem}{ext}")
    } else {
        format!("{safe_stem}{ext}")
    }
}

pub fn is_dangerous_extension(filename: &str) -> bool {
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    DANGEROUS_EXTS.contains(&ext.as_str())
}

#[cfg(windows)]
pub fn confirm_dangerous_download(filename: &str) -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut std::ffi::c_void,
            lpText: *const u16,
            lpCaption: *const u16,
            uType: u32,
        ) -> i32;
    }
    use std::os::windows::ffi::OsStrExt;
    let text: Vec<u16> = std::ffi::OsStr::new(&format!(
        "Attention : le fichier \"{}\" a une extension exécutable potentiellement dangereuse.\n\nSouhaitez-vous vraiment l'enregistrer ?",
        filename
    ))
    .encode_wide()
    .chain(std::iter::once(0))
    .collect();

    let title: Vec<u16> = std::ffi::OsStr::new("Oxygen - Avertissement de téléchargement")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    const MB_YESNO: u32 = 0x00000004;
    const MB_ICONWARNING: u32 = 0x00000030;
    const MB_DEFBUTTON2: u32 = 0x00000100;
    const IDYES: i32 = 6;

    let res = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };
    res == IDYES
}

#[cfg(not(windows))]
pub fn confirm_dangerous_download(_filename: &str) -> bool {
    true
}

#[cfg(windows)]
pub fn prompt_save_file(suggested_path: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    #[link(name = "comdlg32")]
    extern "system" {
        fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct OPENFILENAMEW {
        lStructSize: u32,
        hwndOwner: *mut std::ffi::c_void,
        hInstance: *mut std::ffi::c_void,
        lpstrFilter: *const u16,
        lpstrCustomFilter: *mut u16,
        nMaxCustFilter: u32,
        nFilterIndex: u32,
        lpstrFile: *mut u16,
        nMaxFile: u32,
        lpstrFileTitle: *mut u16,
        nMaxFileTitle: u32,
        lpstrInitialDir: *const u16,
        lpstrTitle: *const u16,
        Flags: u32,
        nFileOffset: u16,
        nFileExtension: u16,
        lpstrDefExt: *const u16,
        lCustData: usize,
        lpfnHook: *mut std::ffi::c_void,
        lpTemplateName: *const u16,
        pvReserved: *mut std::ffi::c_void,
        dwReserved: u32,
        FlagsEx: u32,
    }

    let mut buf = [0u16; 1024];
    let initial_name: Vec<u16> = suggested_path
        .file_name()
        .map(|s| s.encode_wide().collect())
        .unwrap_or_default();
    for (i, &c) in initial_name.iter().take(buf.len() - 1).enumerate() {
        buf[i] = c;
    }

    let initial_dir: Option<Vec<u16>> = suggested_path.parent().map(|p| {
        p.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    });

    let title: Vec<u16> = std::ffi::OsStr::new("Enregistrer sous...")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: std::ptr::null_mut(),
        hInstance: std::ptr::null_mut(),
        lpstrFilter: std::ptr::null(),
        lpstrCustomFilter: std::ptr::null_mut(),
        nMaxCustFilter: 0,
        nFilterIndex: 0,
        lpstrFile: buf.as_mut_ptr(),
        nMaxFile: buf.len() as u32,
        lpstrFileTitle: std::ptr::null_mut(),
        nMaxFileTitle: 0,
        lpstrInitialDir: initial_dir
            .as_ref()
            .map(|v| v.as_ptr())
            .unwrap_or(std::ptr::null()),
        lpstrTitle: title.as_ptr(),
        Flags: 0x00000800 | 0x00000002,
        nFileOffset: 0,
        nFileExtension: 0,
        lpstrDefExt: std::ptr::null(),
        lCustData: 0,
        lpfnHook: std::ptr::null_mut(),
        lpTemplateName: std::ptr::null(),
        pvReserved: std::ptr::null_mut(),
        dwReserved: 0,
        FlagsEx: 0,
    };

    let ret = unsafe { GetSaveFileNameW(&mut ofn) };
    if ret != 0 {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let os_str = std::ffi::OsString::from_wide(&buf[..len]);
        Some(PathBuf::from(os_str))
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn prompt_save_file(_suggested_path: &Path) -> Option<PathBuf> {
    None
}

/// Résout un chemin de destination unique (ajoute (1), (2)... sans écraser)
pub fn unique_destination(dir: &Path, filename: &str) -> Result<PathBuf, String> {
    let p = dir.join(filename);
    if !p.exists() {
        if p.parent() != Some(dir) {
            return Err("dépassement de dossier non autorisé".into());
        }
        return Ok(p);
    }

    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| filename.to_string());
    let ext = Path::new(filename)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();

    for i in 1..10_000 {
        let candidate_name = format!("{stem} ({i}){ext}");
        let candidate_path = dir.join(&candidate_name);
        if !candidate_path.exists() {
            if candidate_path.parent() != Some(dir) {
                return Err("dépassement de dossier non autorisé".into());
            }
            return Ok(candidate_path);
        }
    }
    Err("trop de fichiers homonymes dans le dossier".into())
}

/// Crée le gestionnaire d'événement de téléchargement pour une Webview
pub fn create_download_handler(
    app: AppHandle,
    is_private: bool,
) -> impl Fn(Webview, DownloadEvent<'_>) -> bool + Send + Sync + 'static {
    move |_webview: Webview, event: DownloadEvent<'_>| {
        match event {
            DownloadEvent::Requested { url, destination } => {
                let scheme = url.scheme();
                if scheme == "file" || scheme == "javascript" || scheme == "data" {
                    let _ = app.emit("download-error", "Schéma d'URL refusé par sécurité");
                    return false;
                }

                let domain = url.host_str().unwrap_or("inconnu").to_string();
                if !check_rate_limit(&domain) {
                    let _ = app.emit(
                        "download-error",
                        "Téléchargements multiples bloqués (plus de 3 en 5 secondes)",
                    );
                    return false;
                }

                let suggested = destination
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        url.path_segments()
                            .and_then(|mut s| s.next_back())
                            .filter(|s| !s.is_empty())
                            .unwrap_or("telechargement")
                            .to_string()
                    });

                let safe_name = sanitize_filename(&suggested);

                // Avertissement extension dangereuse
                if is_dangerous_extension(&safe_name) {
                    if !confirm_dangerous_download(&safe_name) {
                        return false;
                    }
                }

                let base_dir = get_default_download_dir(&app);
                let _ = std::fs::create_dir_all(&base_dir);

                let cfg = load_config(&app);
                let target_path = if cfg.ask_location {
                    let suggested_target = base_dir.join(&safe_name);
                    match prompt_save_file(&suggested_target) {
                        Some(chosen) => chosen,
                        None => return false, // Annulé par l'utilisateur
                    }
                } else {
                    match unique_destination(&base_dir, &safe_name) {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = app.emit("download-error", e);
                            return false;
                        }
                    }
                };

                *destination = target_path.clone();

                let now_str = chrono_like_now();
                let item = DownloadItem {
                    id: format!(
                        "{}_{}",
                        std::process::id(),
                        Instant::now().elapsed().as_nanos()
                    ),
                    filename: target_path
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or(safe_name),
                    path: target_path.to_string_lossy().to_string(),
                    domain,
                    size_bytes: None,
                    status: "in_progress".to_string(),
                    created_at: now_str,
                    is_private,
                };

                {
                    let mut guard = ACTIVE_DOWNLOADS.lock().unwrap();
                    let map = guard.get_or_insert_with(HashMap::new);
                    map.insert(target_path.to_string_lossy().to_string(), item.clone());
                }

                add_or_update_history(&app, item.clone());
                let _ = app.emit("download-started", &item);
                true
            }
            DownloadEvent::Finished {
                url: _,
                path,
                success,
            } => {
                let path_buf = path.unwrap_or_default();
                let path_str = path_buf.to_string_lossy().to_string();
                let mut found_item = {
                    let mut guard = ACTIVE_DOWNLOADS.lock().unwrap();
                    let map = guard.get_or_insert_with(HashMap::new);
                    map.remove(&path_str)
                };

                if let Some(ref mut item) = found_item {
                    item.status = if success {
                        "completed".into()
                    } else {
                        "failed".into()
                    };
                    if success && path_buf.exists() {
                        item.size_bytes = std::fs::metadata(&path_buf).ok().map(|m| m.len());
                    }
                    add_or_update_history(&app, item.clone());
                    let _ = app.emit("download-finished", &item);
                } else {
                    let filename = path_buf
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| "fichier".into());
                    let item = DownloadItem {
                        id: format!(
                            "{}_{}",
                            std::process::id(),
                            Instant::now().elapsed().as_nanos()
                        ),
                        filename,
                        path: path_str,
                        domain: "".into(),
                        size_bytes: if success && path_buf.exists() {
                            std::fs::metadata(&path_buf).ok().map(|m| m.len())
                        } else {
                            None
                        },
                        status: if success {
                            "completed".into()
                        } else {
                            "failed".into()
                        },
                        created_at: chrono_like_now(),
                        is_private,
                    };
                    add_or_update_history(&app, item.clone());
                    let _ = app.emit("download-finished", &item);
                }
                true
            }
            _ => true,
        }
    }
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Format simple JJ/MM/AAAA HH:MM
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = (day_secs / 3600 + 1) % 24; // approx timezone
    let mins = (day_secs % 3600) / 60;
    format!("jour {} {:02}h{:02}", days, hours, mins)
}

// ---------- Commandes Tauri ----------

#[tauri::command]
pub fn downloads_get_history(app: AppHandle) -> Result<Vec<DownloadItem>, String> {
    Ok(load_history(&app))
}

#[tauri::command]
pub fn downloads_clear_history(app: AppHandle) -> Result<(), String> {
    save_history(&app, &[])
}

#[tauri::command]
pub fn downloads_remove_item(app: AppHandle, id: String) -> Result<(), String> {
    let mut history = load_history(&app);
    history.retain(|h| h.id != id);
    save_history(&app, &history)
}

#[tauri::command]
pub fn downloads_open_folder(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err("Le fichier n'existe plus à cet emplacement".into());
    }
    let win_path = path.replace('/', "\\");
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{win_path}"))
        .spawn()
        .map_err(|e| format!("Impossible d'ouvrir l'explorateur : {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn downloads_get_config(app: AppHandle) -> Result<DownloadsConfig, String> {
    let mut cfg = load_config(&app);
    if cfg.download_dir.is_none() {
        cfg.download_dir = Some(get_default_download_dir(&app).to_string_lossy().to_string());
    }
    Ok(cfg)
}

#[tauri::command]
pub fn downloads_set_config(app: AppHandle, config: DownloadsConfig) -> Result<(), String> {
    if let Some(ref d) = config.download_dir {
        let p = Path::new(d);
        if !p.exists() {
            let _ = std::fs::create_dir_all(p);
        }
    }
    save_config(&app, &config)
}

#[tauri::command]
pub fn downloads_choose_folder() -> Result<Option<String>, String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "[System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms') | Out-Null; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.Description = 'Sélectionner le dossier de téléchargement'; if ($f.ShowDialog() -eq 'OK') { $f.SelectedPath }",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(path))
    }
}
