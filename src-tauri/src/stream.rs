//! Diffusion en direct (YouTube / Twitch / Kick / TikTok) via un sidecar FFmpeg.
#![allow(
    clippy::doc_overindented_list_items,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments,
    clippy::match_like_matches_macro,
    clippy::needless_borrow,
    clippy::lines_filter_map_ok
)]
//!
//! INTÉGRATION
//! 1. Cargo.toml (src-tauri) : keyring = "3"  |  serde = { version = "1", features = ["derive"] }
//!    et dépendance vers lightbrowser-core (ajouter `pub mod stream_args;` dans son lib.rs).
//! 2. Placer ffmpeg.exe à côté de LightBrowser.exe (ou dans le PATH).
//! 3. lib.rs : `mod stream;`, puis dans le builder
//!      .manage(stream::StreamState::default())
//!      et ajouter dans generate_handler![ ..., stream::start_stream, stream::stop_stream,
//!        stream::stream_status, stream::save_stream_key, stream::delete_stream_key ]
//! 4. Front (main.js) :
//!      await invoke('save_stream_key', { platform: 'twitch', key });
//!      await invoke('start_stream', { args: { source: { kind: 'Screen' }, destinations:
//!        [{ platform: 'twitch' }, { platform: 'youtube' }] } });
//!      listen('stream-ended', e => toast('Diffusion terminée (code ' + e.payload + ')'));
//!
//! Non compilé dans cet environnement (pas de toolchain Rust) : à valider avec `cargo check`.

use lightbrowser_core::stream_args::{
    build_args, list_devices_args, parse_dshow_devices, probe_args, probe_ddagrab_args,
    redact_args, Config, Corner, Encoder, Overlay, Source,
};
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default, Clone)]
pub struct StreamState {
    child: Arc<Mutex<Option<Child>>>,
    /// Numéro de session : tout changement (arrêt, nouveau flux) rend le superviseur en cours obsolète.
    gen: Arc<AtomicU64>,
    /// Session de diffusion en cours (y compris pendant l'attente d'une reconnexion).
    active: Arc<AtomicBool>,
    loopback: Arc<Mutex<Option<crate::audio_loopback::LoopbackHandle>>>,
    stderr_lines: Arc<Mutex<Vec<String>>>,
}

impl StreamState {
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "name")]
pub enum SourceDto {
    Screen,
    Window(String),
    Webcam(String),
    AppWindow,
}

#[derive(Deserialize)]
pub struct Dest {
    /// "youtube", "twitch", "kick", "tiktok" ou tout identifiant libre.
    pub platform: String,
    /// URL d'ingestion sans la clé. Requise sauf pour youtube/twitch (valeurs par défaut).
    pub url: Option<String>,
}

#[derive(Deserialize)]
pub struct OverlayDto {
    pub webcam: String,
    pub corner: String, // "top-left" | "top-right" | "bottom-left" | "bottom-right"
    pub size_pct: Option<u32>,
    pub margin_px: Option<u32>,
}

#[derive(Deserialize)]
pub struct StartArgs {
    pub source: SourceDto,
    pub audio_device: Option<String>,
    pub system_audio_device: Option<String>,
    pub mic_volume: Option<f32>,
    pub system_volume: Option<f32>,
    pub overlay: Option<OverlayDto>,
    pub fps: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub destinations: Vec<Dest>,
    pub record: Option<bool>,
    pub auto_reconnect: Option<bool>,
}

fn ffmpeg_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("ffmpeg.exe");
            if p.exists() {
                return p;
            }
        }
    }
    PathBuf::from("ffmpeg")
}

fn hide(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Premier encodeur matériel qui encode réellement une image de test, sinon x264.
fn pick_encoder(ffmpeg: &PathBuf) -> Encoder {
    for enc in [Encoder::Nvenc, Encoder::Qsv, Encoder::Amf] {
        let mut cmd = Command::new(ffmpeg);
        hide(&mut cmd)
            .args(probe_args(enc))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if matches!(cmd.status(), Ok(s) if s.success()) {
            return enc;
        }
    }
    Encoder::X264
}

static DDAGRAB_SUPPORTED: OnceLock<bool> = OnceLock::new();

#[cfg(windows)]
#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(
        hwnd: isize,
        dwAttribute: u32,
        pvAttribute: *mut std::ffi::c_void,
        cbAttribute: u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetWindowRect(hwnd: isize, lpRect: *mut RECT) -> i32;
    fn GetForegroundWindow() -> isize;
    fn IsIconic(hwnd: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, lpdwProcessId: *mut u32) -> u32;
}

#[cfg(windows)]
fn get_window_region(hwnd: isize) -> Option<(i32, i32, u32, u32, bool)> {
    unsafe {
        let is_min = IsIconic(hwnd) != 0;
        let is_vis = IsWindowVisible(hwnd) != 0 && !is_min;

        let mut rect: RECT = std::mem::zeroed();
        let dwm_res = DwmGetWindowAttribute(
            hwnd,
            9, // DWMWA_EXTENDED_FRAME_BOUNDS
            &mut rect as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        );
        if dwm_res != 0 && GetWindowRect(hwnd, &mut rect) == 0 {
            return None;
        }

        let screen_w = GetSystemMetrics(0); // SM_CXSCREEN
        let screen_h = GetSystemMetrics(1); // SM_CYSCREEN

        let raw_w = rect.right - rect.left;
        let raw_h = rect.bottom - rect.top;

        let x = rect.left.max(0);
        let y = rect.top.max(0);
        let max_w = (screen_w - x).max(0) as u32;
        let max_h = (screen_h - y).max(0) as u32;

        let mut w = (raw_w.max(2) as u32).min(max_w);
        let mut h = (raw_h.max(2) as u32).min(max_h);

        w = (w / 2) * 2;
        h = (h / 2) * 2;
        if w < 2 {
            w = 2;
        }
        if h < 2 {
            h = 2;
        }

        let fg = GetForegroundWindow();
        let mut fg_pid = 0u32;
        GetWindowThreadProcessId(fg, &mut fg_pid);
        let my_pid = std::process::id();
        let tree_pids = crate::ram::get_tree_pids(my_pid);
        let is_our_process = fg == hwnd || tree_pids.contains(&fg_pid);
        let is_fullscreen = x == 0
            && y == 0
            && w >= (screen_w as u32).saturating_sub(10)
            && h >= (screen_h as u32).saturating_sub(10);
        let is_active = is_vis && (is_our_process || is_fullscreen);

        Some((x, y, w, h, is_active))
    }
}

fn pause_image_path() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("pause.png");
            if p.exists() {
                return p.to_string_lossy().replace('\\', "/");
            }
        }
    }
    let p = PathBuf::from("src/pause.png");
    if p.exists() {
        return p.to_string_lossy().replace('\\', "/");
    }
    String::new()
}

fn is_ddagrab_supported(ffmpeg: &PathBuf) -> bool {
    *DDAGRAB_SUPPORTED.get_or_init(|| {
        let mut cmd = Command::new(ffmpeg);
        hide(&mut cmd)
            .args(probe_ddagrab_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        matches!(cmd.status(), Ok(s) if s.success())
    })
}

fn default_url(platform: &str) -> Option<&'static str> {
    match platform {
        "youtube" => Some("rtmp://a.rtmp.youtube.com/live2"),
        "twitch" => Some("rtmp://live.twitch.tv/app"),
        _ => None,
    }
}

fn entry(platform: &str) -> Result<keyring::Entry, String> {
    let p = platform.trim().to_lowercase();
    keyring::Entry::new("LightBrowser", &format!("stream-key:{p}")).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_stream_key(platform: String, key: String) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("clé vide".into());
    }
    entry(&platform)?
        .set_password(key.trim())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_stream_key(platform: String) -> Result<(), String> {
    entry(&platform)?
        .delete_credential()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn has_stream_key(platform: String) -> bool {
    entry(&platform)
        .and_then(|e| e.get_password().map_err(|e| e.to_string()))
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
}

fn resolve_outputs(dests: &[Dest]) -> Result<Vec<String>, String> {
    dests
        .iter()
        .map(|d| {
            let base = d
                .url
                .as_deref()
                .or_else(|| default_url(&d.platform))
                .ok_or_else(|| format!("URL d'ingestion requise pour {}", d.platform))?;
            let key = entry(&d.platform)?
                .get_password()
                .map_err(|_| format!("clé de flux absente pour {}", d.platform))?;
            Ok(format!("{}/{}", base.trim_end_matches('/'), key.trim()))
        })
        .collect()
}

const MAX_ATTEMPTS: u32 = 12; // ≈ 4 min de tentatives (2, 4, 8, 16 puis 30 s)

#[derive(Clone, serde::Serialize)]
struct Reconnecting {
    attempt: u32,
    delay_s: u64,
}

/// Remet `active` à false quand le superviseur se termine, quelle que soit la sortie, sauf s'il est devenu obsolète.
struct EndGuard {
    active: Arc<AtomicBool>,
    gen: Arc<AtomicU64>,
    my_gen: u64,
}

impl Drop for EndGuard {
    fn drop(&mut self) {
        if self.gen.load(Ordering::SeqCst) == self.my_gen {
            self.active.store(false, Ordering::SeqCst);
        }
    }
}

fn spawn_ffmpeg(ffmpeg: &PathBuf, cfg: &Config, st: &StreamState) -> Result<Child, String> {
    let argv = build_args(cfg)?;
    eprintln!("[stream] ffmpeg {}", redact_args(&argv).join(" ")); // clés masquées
    let mut cmd = Command::new(ffmpeg);
    let mut child = hide(&mut cmd)
        .args(&argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("lancement de ffmpeg impossible : {e}"))?;

    if let Some(err_pipe) = child.stderr.take() {
        let err_buf = st.stderr_lines.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(err_pipe);
            for line in reader.lines().filter_map(|r| r.ok()) {
                if let Ok(mut g) = err_buf.lock() {
                    if g.len() >= 30 {
                        g.remove(0);
                    }
                    g.push(line);
                }
            }
        });
    }

    Ok(child)
}

/// Chemin du MKV : dossier Vidéos (repli Bureau), « / » partout. `part` > 1 après une reconnexion (nouveau fichier).
fn record_target(app: &AppHandle, stamp: u64, part: u32) -> Result<String, String> {
    let forbidden = |s: &str| s.chars().any(|c| matches!(c, '|' | '"' | '\'' | '[' | ']'));
    let pm = app.path();
    let mut dirs = Vec::new();
    if let Ok(d) = pm.video_dir() {
        dirs.push(d);
    }
    if let Ok(d) = pm.desktop_dir() {
        dirs.push(d);
    }
    for d in dirs {
        let s = d.to_string_lossy().replace('\\', "/");
        if forbidden(&s) || std::fs::create_dir_all(&d).is_err() {
            continue;
        }
        let name = if part <= 1 {
            format!("LightBrowser_{stamp}.mkv")
        } else {
            format!("LightBrowser_{stamp}_p{part}.mkv")
        };
        return Ok(format!("{}/{}", s.trim_end_matches('/'), name));
    }
    Err("dossier d'enregistrement inutilisable".into())
}

/// Surveille FFmpeg et le relance s'il s'arrête sans demande de l'utilisateur.
fn supervise(
    app: AppHandle,
    ffmpeg: PathBuf,
    mut cfg: Config,
    st: StreamState,
    my_gen: u64,
    auto_reconnect: bool,
    stamp: u64,
    app_window_hwnd: Option<isize>,
) {
    let _guard = EndGuard {
        active: st.active.clone(),
        gen: st.gen.clone(),
        my_gen,
    };
    let current = || st.gen.load(Ordering::SeqCst) == my_gen;
    let (mut attempts, mut part) = (0u32, 1u32);
    let mut established = false; // au moins un flux a tenu 10 s (sinon : clé/URL invalide, pas de boucle)
    let mut have_child = true;
    let mut t0 = Instant::now();
    let mut was_minimized = false;
    let mut was_paused = matches!(cfg.source, Source::Pause(_));
    let mut last_min_check = Instant::now();
    let mut last_region_check = Instant::now();
    loop {
        let mut code: Option<i32> = None;
        let mut source_changed = false;
        if have_child {
            loop {
                std::thread::sleep(Duration::from_millis(300));
                if !current() {
                    return; // arrêt manuel ou nouveau flux
                }
                if last_min_check.elapsed() >= Duration::from_secs(2) {
                    last_min_check = Instant::now();
                    if let Some(win) = app.get_window("main") {
                        let is_fs = win.is_fullscreen().unwrap_or(false);
                        let is_min = win.is_minimized().unwrap_or(false) && !is_fs;
                        if is_min && !was_minimized {
                            was_minimized = true;
                            let _ =
                                app.emit("stream-warning", "Fenêtre réduite : l'image est figée");
                        } else if !is_min && was_minimized {
                            was_minimized = false;
                            let _ = app.emit("stream-warning", "Fenêtre restaurée");
                        }
                    }
                }

                if app_window_hwnd.is_some()
                    && last_region_check.elapsed() >= Duration::from_millis(500)
                {
                    last_region_check = Instant::now();
                    #[cfg(windows)]
                    if let Some(h) = app_window_hwnd {
                        if let Some((_cur_x, _cur_y, _cur_w, _cur_h, is_active)) =
                            get_window_region(h)
                        {
                            let should_pause = !is_active;
                            let target_source = if should_pause {
                                Source::Pause(pause_image_path())
                            } else {
                                Source::Screen
                            };

                            if target_source != cfg.source {
                                if should_pause && !was_paused {
                                    was_paused = true;
                                    let _ = app.emit(
                                        "stream-warning",
                                        "Diffusion en pause : Oxygen est en arrière-plan",
                                    );
                                } else if !should_pause && was_paused {
                                    was_paused = false;
                                    let _ = app.emit("stream-warning", "Diffusion reprise");
                                }
                                cfg.source = target_source;
                                let mut g = match st.child.lock() {
                                    Ok(g) => g,
                                    Err(_) => return,
                                };
                                if let Some(mut c) = g.take() {
                                    let _ = c.kill();
                                    let _ = c.wait();
                                }
                                source_changed = true;
                                break;
                            }
                        }
                    }
                }

                let mut g = match st.child.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                match g.as_mut() {
                    Some(c) => match c.try_wait() {
                        Ok(Some(status)) => {
                            code = status.code();
                            *g = None;
                            break;
                        }
                        Ok(None) => {}
                        Err(_) => return,
                    },
                    None => return,
                }
            }
        }
        if !current() {
            return;
        }

        if source_changed {
            t0 = Instant::now();
            attempts = 0; // Le changement de source (pause/reprise) ne doit pas épuiser les tentatives
            match spawn_ffmpeg(&ffmpeg, &cfg, &st) {
                Ok(child) => {
                    let mut g = match st.child.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    if !current() {
                        let mut c = child;
                        let _ = c.kill();
                        let _ = c.wait();
                        return;
                    }
                    *g = Some(child);
                    have_child = true;
                }
                Err(e) => {
                    eprintln!("[stream] Erreur relance FFmpeg changement source : {e}");
                    have_child = false;
                }
            }
            continue;
        }

        let lived = t0.elapsed();
        if lived >= Duration::from_secs(10) {
            established = true;
        }
        if lived >= Duration::from_secs(60) {
            attempts = 0; // le flux était stable : on repart d'un compteur neuf
        }
        attempts += 1;

        // Journalisation de diagnostic stderr (sans clé de flux) et de l'état du superviseur
        eprintln!(
            "[stream] FFmpeg s'est arrêté (code {:?}) après {:?}",
            code, lived
        );
        if let Ok(g) = st.stderr_lines.lock() {
            if !g.is_empty() {
                eprintln!("[stream] Dernières lignes stderr FFmpeg (clés masquées) :");
                for line in g.iter() {
                    eprintln!("  {}", lightbrowser_core::stream_args::redact(line));
                }
            }
        }
        let delay = (1u64 << attempts.min(5)).min(30);
        eprintln!(
            "[stream-superviseur] Tentative {}/{}, délai : {}s, flux actif : {}",
            attempts,
            MAX_ATTEMPTS,
            delay,
            st.active.load(Ordering::SeqCst)
        );

        if !auto_reconnect || !established || attempts > MAX_ATTEMPTS {
            st.active.store(false, Ordering::SeqCst);
            let _ = app.emit("stream-ended", code);
            return;
        }

        let _ = app.emit(
            "stream-reconnecting",
            Reconnecting {
                attempt: attempts,
                delay_s: delay,
            },
        );
        for _ in 0..(delay * 4) {
            std::thread::sleep(Duration::from_millis(250));
            if !current() {
                return;
            }
        }

        // Si capture loopback active, recréer le serveur loopback
        let is_loopback = cfg
            .system_audio_device
            .as_deref()
            .map(|s| s.starts_with("tcp://127.0.0.1:"))
            .unwrap_or(false);
        if is_loopback && current() {
            if let Ok(mut g) = st.loopback.lock() {
                g.take();
                if let Ok(new_handle) = crate::audio_loopback::start_loopback() {
                    cfg.system_audio_device = Some(format!("tcp://127.0.0.1:{}", new_handle.port));
                    *g = Some(new_handle);
                }
            }
        }

        // Nouveau fichier MKV à chaque relance (sinon le précédent serait écrasé).
        part += 1;
        if cfg.record_path.is_some() {
            cfg.record_path = record_target(&app, stamp, part).ok();
        }
        t0 = Instant::now();
        match spawn_ffmpeg(&ffmpeg, &cfg, &st) {
            Ok(child) => {
                let mut g = match st.child.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if !current() {
                    let mut c = child;
                    let _ = c.kill();
                    let _ = c.wait();
                    return;
                }
                *g = Some(child);
                have_child = true;
            }
            Err(_) => have_child = false, // compté comme tentative ratée au tour suivant
        }
    }
}

#[tauri::command]
pub fn start_stream(
    app: AppHandle,
    state: State<StreamState>,
    mut args: StartArgs,
) -> Result<(), String> {
    if state.active.load(Ordering::SeqCst) {
        return Err("diffusion déjà en cours".into());
    }

    if let Some(win) = app.get_window("main") {
        if win.is_minimized().unwrap_or(false) && !win.is_fullscreen().unwrap_or(false) {
            return Err("Fenêtre réduite : restaure-la avant de démarrer".into());
        }
    }

    let is_loopback = match args.system_audio_device.as_deref() {
        Some("loopback:default") | Some("auto") => true,
        _ => false,
    };
    if is_loopback {
        let handle = crate::audio_loopback::start_loopback().map_err(|e| {
            format!("Échec démarrage capture son ordinateur (WASAPI loopback) : {e}")
        })?;
        args.system_audio_device = Some(format!("tcp://127.0.0.1:{}", handle.port));
        if let Ok(mut g) = state.loopback.lock() {
            *g = Some(handle);
        }
    }

    let ffmpeg = ffmpeg_path();

    let app_window_hwnd: Option<isize> = if matches!(args.source, SourceDto::AppWindow) {
        #[cfg(windows)]
        {
            app.get_window("main")
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0 as isize)
        }
        #[cfg(not(windows))]
        {
            None
        }
    } else {
        None
    };

    let source = match args.source {
        SourceDto::Screen => Source::Screen,
        SourceDto::Window(t) => Source::Window(t),
        SourceDto::Webcam(n) => Source::Webcam(n),
        SourceDto::AppWindow => {
            #[cfg(windows)]
            {
                if let Some(h) = app_window_hwnd {
                    let (_x, _y, _w, _h, is_active) = get_window_region(h)
                        .ok_or("Impossible de lire les coordonnées de la fenêtre Oxygen")?;
                    if !is_active {
                        Source::Pause(pause_image_path())
                    } else {
                        // Capture stable de l'écran du moniteur : couvre le plein écran et la fenêtre sans interruption
                        Source::Screen
                    }
                } else {
                    Source::Screen
                }
            }
            #[cfg(not(windows))]
            {
                Source::Screen
            }
        }
    };
    let mut cfg = Config::new(source, resolve_outputs(&args.destinations)?);
    if matches!(cfg.source, Source::Screen) || matches!(cfg.source, Source::Region { .. }) {
        if is_ddagrab_supported(&ffmpeg) {
            cfg.use_ddagrab = true;
            eprintln!("[stream] Capture écran/région : ddagrab (DXGI matériel)");
        } else {
            eprintln!("[stream] Capture écran/région : gdigrab (GDI)");
        }
    }
    cfg.audio_device = args.audio_device;
    cfg.system_audio_device = args.system_audio_device;
    if let Some(v) = args.mic_volume {
        cfg.mic_volume = v;
    }
    if let Some(v) = args.system_volume {
        cfg.system_volume = v;
    }
    if let Some(o) = args.overlay {
        let corner = match o.corner.as_str() {
            "top-left" => Corner::TopLeft,
            "top-right" => Corner::TopRight,
            "bottom-left" => Corner::BottomLeft,
            "bottom-right" => Corner::BottomRight,
            other => return Err(format!("coin d'incrustation inconnu : {other}")),
        };
        cfg.overlay = Some(Overlay {
            webcam: o.webcam,
            corner,
            size_pct: o.size_pct.unwrap_or(25),
            margin_px: o.margin_px.unwrap_or(20),
        });
    }
    if let Some(f) = args.fps {
        cfg.fps = f;
    }
    if let Some(b) = args.bitrate_kbps {
        cfg.bitrate_kbps = b;
    }
    cfg.encoder = pick_encoder(&ffmpeg);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if args.record.unwrap_or(false) {
        cfg.record_path = Some(record_target(&app, stamp, 1)?);
    }

    let child = spawn_ffmpeg(&ffmpeg, &cfg, &state.inner())?;
    let st = state.inner().clone();
    let my_gen = st.gen.fetch_add(1, Ordering::SeqCst) + 1;
    *st.child.lock().map_err(|_| "état verrouillé".to_string())? = Some(child);
    st.active.store(true, Ordering::SeqCst);

    let reconnect = args.auto_reconnect.unwrap_or(true);
    std::thread::spawn(move || {
        supervise(
            app,
            ffmpeg,
            cfg,
            st,
            my_gen,
            reconnect,
            stamp,
            app_window_hwnd,
        )
    });
    Ok(())
}

#[tauri::command]
pub fn stop_stream(state: State<StreamState>) -> Result<(), String> {
    // Invalider le superviseur AVANT de tuer FFmpeg : pas de reconnexion après un arrêt manuel.
    state.gen.fetch_add(1, Ordering::SeqCst);
    state.active.store(false, Ordering::SeqCst);
    if let Ok(mut g) = state.loopback.lock() {
        g.take();
    }
    let child = state
        .child
        .lock()
        .map_err(|_| "état verrouillé".to_string())?
        .take();
    if let Some(mut c) = child {
        if let Some(stdin) = c.stdin.as_mut() {
            let _ = stdin.write_all(b"q\n"); // arrêt propre : finalise le MKV
        }
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(3) {
            if matches!(c.try_wait(), Ok(Some(_))) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = c.kill();
        let _ = c.wait();
    }
    Ok(())
}

#[tauri::command]
pub fn stream_status(state: State<StreamState>) -> bool {
    state.active.load(Ordering::SeqCst) // vrai aussi pendant l'attente d'une reconnexion
}

#[derive(serde::Serialize)]
pub struct DevicesDto {
    pub video: Vec<String>,
    pub audio: Vec<String>,
}

#[tauri::command]
pub fn list_av_devices() -> Result<DevicesDto, String> {
    let mut cmd = Command::new(ffmpeg_path());
    let out = hide(&mut cmd)
        .args(list_devices_args())
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffmpeg introuvable : {e}"))?;
    // La liste sort sur stderr ; le code retour 1 est normal avec « -i dummy ».
    let d = parse_dshow_devices(&String::from_utf8_lossy(&out.stderr));
    Ok(DevicesDto {
        video: d.video,
        audio: d.audio,
    })
}
