mod adblock;
mod ai;
pub(crate) mod audio_loopback;
mod downloads;
mod netfilter;
mod ram;
mod store;
mod stream;

use serde_json::json;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{
    Emitter, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Nombre maximal de webviews de sites gardées en vie (économie de RAM).
/// Les onglets les moins récemment utilisés sont déchargés (vue détruite,
/// l'onglet et son adresse restent, la page se recharge à la réactivation).
const MAX_LIVE: usize = 2;
const AI_PANEL_W: f64 = 420.0;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Default)]
struct State {
    top: f64,
    ai_panel_open: bool,
    downloads_panel_open: bool,
    active: Option<String>,
    active_is_internal: bool,
    /// ids d'onglets avec une vue vivante, du plus ancien au plus récent
    live: Vec<String>,
    /// ensemble des onglets privés en cours (cookies volatils isolés)
    private_tabs: HashSet<String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap();
    let st = guard.get_or_insert_with(|| State {
        top: 86.0,
        ai_panel_open: false,
        downloads_panel_open: false,
        active_is_internal: true,
        ..Default::default()
    });
    f(st)
}

fn label(tab_id: &str) -> String {
    let clean: String = tab_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("tab_{clean}")
}

fn win_size(window: &tauri::Window) -> Result<(f64, f64), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    Ok((size.width as f64 / scale, size.height as f64 / scale))
}

pub(crate) fn active_tab_webview(app: &tauri::AppHandle) -> Option<tauri::Webview> {
    let id = with_state(|s| s.active.clone())?;
    app.get_webview(&label(&id))
}

pub(crate) fn is_private_tab(_app: &tauri::AppHandle, view: &tauri::Webview) -> bool {
    let lbl = view.label();
    with_state(|s| s.private_tabs.iter().any(|pt| label(pt) == lbl))
}

fn apply_layout(app: &tauri::AppHandle) {
    let Some(win) = app.get_window("main") else {
        return;
    };
    let fs = win.is_fullscreen().unwrap_or(false);
    let Ok((win_w, win_h)) = win_size(&win) else {
        return;
    };
    let top = if fs { 0.0 } else { with_state(|s| s.top) };
    let (ai_open, dl_open, is_internal) = with_state(|s| {
        (
            s.ai_panel_open,
            s.downloads_panel_open,
            s.active_is_internal,
        )
    });
    let panel_open = ai_open || dl_open;
    let content_w = if panel_open && !fs && !is_internal {
        (win_w - AI_PANEL_W).max(200.0)
    } else {
        win_w
    };
    let content_h = if fs { win_h } else { (win_h - top).max(100.0) };

    if let Some(tab) = active_tab_webview(app) {
        let _ = tab.set_position(LogicalPosition::new(0.0, top));
        let _ = tab.set_size(LogicalSize::new(content_w, content_h));
    }
    if let Some(ui) = app.get_webview("main") {
        let _ = if fs { ui.hide() } else { ui.show() };
    }
    if fs && panel_open {
        with_state(|s| {
            s.ai_panel_open = false;
            s.downloads_panel_open = false;
        });
        let _ = app.emit("ai-close-sidebar", ());
        let _ = app.emit("downloads-close-sidebar", ());
    }

    let gs = app.global_shortcut();
    if fs {
        if let Ok(esc) = "Escape".parse::<Shortcut>() {
            let _ = gs.register(esc);
        }
        if let Ok(f11) = "F11".parse::<Shortcut>() {
            let _ = gs.register(f11);
        }
    } else {
        if let Ok(esc) = "Escape".parse::<Shortcut>() {
            let _ = gs.unregister(esc);
        }
        if let Ok(f11) = "F11".parse::<Shortcut>() {
            let _ = gs.unregister(f11);
        }
    }
}

#[tauri::command]
fn exit_fullscreen(app: tauri::AppHandle) {
    if let Some(tab) = active_tab_webview(&app) {
        let _ = tab.eval("document.exitFullscreen && document.exitFullscreen()");
    }
    if let Some(win) = app.get_window("main") {
        let _ = win.set_fullscreen(false);
    }
    apply_layout(&app);
}

#[tauri::command]
fn toggle_fullscreen(app: tauri::AppHandle) {
    if let Some(win) = app.get_window("main") {
        let fs = win.is_fullscreen().unwrap_or(false);
        if fs {
            exit_fullscreen(app);
        } else {
            let _ = win.set_fullscreen(true);
            apply_layout(&app);
        }
    }
}

/// Affiche l'onglet `tab_id`. `navigate` = true : (re)charge `url` ;
/// false : montre simplement la vue existante (ou la crée si elle a été déchargée).
/// `url` vide = page d'accueil : toutes les vues sont masquées.
#[tauri::command]
async fn tab_show(
    app: tauri::AppHandle,
    tab_id: String,
    url: String,
    navigate: bool,
    top_px: f64,
    incognito: Option<bool>,
) -> Result<(), String> {
    if top_px > 0.0 {
        with_state(|s| s.top = top_px);
    }
    let top = with_state(|s| s.top);
    let window = app.get_window("main").ok_or("Fenêtre 'main' introuvable")?;
    let (win_w, win_h) = win_size(&window)?;
    let content_h = (win_h - top).max(100.0);

    if let Some(win) = app.get_window("main") {
        if win.is_fullscreen().unwrap_or(false) {
            exit_fullscreen(app.clone());
        }
    }

    let is_incognito = incognito.unwrap_or(false);
    if is_incognito {
        with_state(|s| {
            s.private_tabs.insert(tab_id.clone());
        });
    }
    let is_streaming = app
        .try_state::<stream::StreamState>()
        .map(|s| s.is_active())
        .unwrap_or(false);

    // masque toutes les vues de sites
    for (name, view) in app.webviews() {
        if name.starts_with("tab_") {
            let _ = view.hide();
        }
    }

    let is_internal = url.trim().is_empty();
    with_state(|s| {
        s.active = Some(tab_id.clone());
        s.active_is_internal = is_internal;
    });

    if is_internal {
        return Ok(());
    }

    let (ai_open, dl_open) = with_state(|s| (s.ai_panel_open, s.downloads_panel_open));
    let panel_open = ai_open || dl_open;
    let content_w = if panel_open {
        (win_w - AI_PANEL_W).max(200.0)
    } else {
        win_w
    };

    let lbl = label(&tab_id);
    if let Some(view) = app.get_webview(&lbl) {
        view.set_position(LogicalPosition::new(0.0, top))
            .map_err(|e| e.to_string())?;
        view.set_size(LogicalSize::new(content_w, content_h))
            .map_err(|e| e.to_string())?;
        if navigate {
            let parsed = tauri::Url::parse(&url).map_err(|e| e.to_string())?;
            view.navigate(parsed).map_err(|e| e.to_string())?;
        }
        view.show().map_err(|e| e.to_string())?;
    } else {
        let parsed = tauri::Url::parse(&url).map_err(|e| e.to_string())?;
        let handle = app.clone();
        let tid = tab_id.clone();
        window
            .add_child(
                WebviewBuilder::new(&lbl, WebviewUrl::External(parsed))
                    .user_agent(USER_AGENT)
                    .initialization_script(&adblock::init_script())
                    .incognito(is_incognito)
                    .on_download(downloads::create_download_handler(
                        handle.clone(),
                        is_incognito,
                    ))
                    .on_navigation(move |u| {
                        if !adblock::allow_navigation(u.host_str()) {
                            return false;
                        }
                        if let Some(win) = handle.get_window("main") {
                            if win.is_fullscreen().unwrap_or(false) {
                                exit_fullscreen(handle.clone());
                            }
                        }
                        apply_layout(&handle);
                        let _ = handle.emit_to(
                            "main",
                            "url-changed",
                            json!({ "tab": tid, "url": u.to_string() }),
                        );
                        true
                    }),
                LogicalPosition::new(0.0, top),
                LogicalSize::new(content_w, content_h),
            )
            .map_err(|e| e.to_string())?;

        if let Some(view) = app.get_webview(&lbl) {
            let _ = netfilter::install(&view);
        }
    }

    // LRU : les onglets privés et toute navigation pendant une diffusion active sont exclus
    // du déchargement automatique pour préserver la session volatile et la capture en cours.
    let to_unload: Vec<String> = with_state(|s| {
        if !is_streaming && !s.private_tabs.contains(&tab_id) {
            s.live
                .retain(|t| t != &tab_id && !s.private_tabs.contains(t));
            s.live.push(tab_id.clone());
            let mut out = Vec::new();
            while s.live.len() > MAX_LIVE {
                out.push(s.live.remove(0));
            }
            out
        } else {
            Vec::new()
        }
    });
    for t in to_unload {
        if let Some(view) = app.get_webview(&label(&t)) {
            let _ = view.close();
        }
        let _ = app.emit_to("main", "tab-unloaded", json!({ "tab": t }));
    }
    Ok(())
}

#[tauri::command]
async fn tab_close(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    if let Some(win) = app.get_window("main") {
        if win.is_fullscreen().unwrap_or(false) {
            exit_fullscreen(app.clone());
        }
    }
    if let Some(view) = app.get_webview(&label(&tab_id)) {
        let _ = view.close();
    }
    with_state(|s| {
        s.live.retain(|t| t != &tab_id);
        s.private_tabs.remove(&tab_id);
        if s.active.as_deref() == Some(tab_id.as_str()) {
            s.active = None;
            s.active_is_internal = true;
        }
    });
    apply_layout(&app);
    Ok(())
}

#[tauri::command]
async fn action_navigation(app: tauri::AppHandle, action: String) -> Result<(), String> {
    let active = with_state(|s| s.active.clone());
    if let Some(id) = active {
        if let Some(view) = app.get_webview(&label(&id)) {
            let js = match action.as_str() {
                "back" => "window.history.back()",
                "forward" => "window.history.forward()",
                "reload" => "window.location.reload()",
                _ => return Ok(()),
            };
            view.eval(js).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn set_top(app: tauri::AppHandle, top_px: f64) -> Result<(), String> {
    if top_px > 0.0 {
        with_state(|s| s.top = top_px);
    }
    apply_layout(&app);
    Ok(())
}

#[tauri::command]
async fn ai_set_panel_open(app: tauri::AppHandle, open: bool) -> Result<(), String> {
    with_state(|s| {
        s.ai_panel_open = open;
        if open {
            s.downloads_panel_open = false;
        }
    });
    if open {
        let _ = app.emit("downloads-close-sidebar", ());
    }
    apply_layout(&app);
    Ok(())
}

#[tauri::command]
async fn downloads_set_panel_open(app: tauri::AppHandle, open: bool) -> Result<(), String> {
    with_state(|s| {
        s.downloads_panel_open = open;
        if open {
            s.ai_panel_open = false;
        }
    });
    if open {
        let _ = app.emit("ai-close-sidebar", ());
    }
    apply_layout(&app);
    Ok(())
}

#[tauri::command]
async fn active_view_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    let active = with_state(|s| s.active.clone());
    if let Some(id) = active {
        if let Some(view) = app.get_webview(&label(&id)) {
            if visible {
                let _ = view.show();
            } else {
                let _ = view.hide();
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let is_esc = "Escape"
                            .parse::<Shortcut>()
                            .map(|s| s == *shortcut)
                            .unwrap_or(false);
                        let is_f11 = "F11"
                            .parse::<Shortcut>()
                            .map(|s| s == *shortcut)
                            .unwrap_or(false);
                        if is_esc || is_f11 {
                            exit_fullscreen(app.clone());
                        }
                    }
                })
                .build(),
        )
        .manage(stream::StreamState::default())
        .manage(ai::PageTexts::default())
        .setup(|app| {
            store::init(&app.handle());
            ram::start_monitoring(app.handle().clone());
            if let Some(window) = app.get_window("main") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        apply_layout(&handle);
                    }
                    WindowEvent::Destroyed => {
                        ram::stop_monitoring();
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tab_show,
            tab_close,
            set_top,
            ai_set_panel_open,
            action_navigation,
            active_view_visible,
            exit_fullscreen,
            toggle_fullscreen,
            adblock::adblock_count,
            ram::ram_usage,
            ram::ram_diagnostic,
            store::bookmark_add,
            store::bookmark_remove,
            store::bookmark_list,
            store::history_add,
            store::history_list,
            store::history_clear,
            stream::start_stream,
            stream::stop_stream,
            stream::stream_status,
            stream::save_stream_key,
            stream::delete_stream_key,
            stream::has_stream_key,
            stream::list_av_devices,
            ai::ai_save_key,
            ai::ai_delete_key,
            ai::ai_has_key,
            ai::ai_check_key,
            ai::ai_chat,
            ai::ai_get_page,
            ai::ai_push_page_text,
            ai::ai_get_config,
            ai::ai_set_config,
            ai::ai_list_models,
            downloads_set_panel_open,
            downloads::downloads_get_history,
            downloads::downloads_clear_history,
            downloads::downloads_remove_item,
            downloads::downloads_open_folder,
            downloads::downloads_get_config,
            downloads::downloads_set_config,
            downloads::downloads_choose_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
