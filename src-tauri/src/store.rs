//! Enveloppe de persistance atomique et commandes Tauri pour les favoris et l'historique.
//! La logique pure réside dans le crate lightbrowser-core.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::Manager;

use lightbrowser_core::store::{
    add_bookmark_pure, add_history_pure, filter_history_pure, load_json_or_default,
    remove_bookmark_pure,
};
pub use lightbrowser_core::store::{Bookmark, HistoryEntry};

#[derive(Default, Debug)]
pub struct StoreData {
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<HistoryEntry>,
}

fn atomic_write_json<T: Serialize>(path: &Path, data: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    let json_bytes = serde_json::to_vec_pretty(data)?;
    fs::write(&tmp_path, json_bytes)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

pub struct Store {
    base_dir: PathBuf,
    data: Mutex<StoreData>,
}

impl Store {
    pub fn new(base_dir: PathBuf) -> Self {
        let bookmarks_path = base_dir.join("bookmarks.json");
        let history_path = base_dir.join("history.json");

        let bookmarks: Vec<Bookmark> = load_json_or_default(&bookmarks_path);
        let history: Vec<HistoryEntry> = load_json_or_default(&history_path);

        Self {
            base_dir,
            data: Mutex::new(StoreData { bookmarks, history }),
        }
    }

    pub fn save_bookmarks(&self) {
        let path = self.base_dir.join("bookmarks.json");
        let list = {
            let guard = self.data.lock().unwrap();
            guard.bookmarks.clone()
        };
        let _ = atomic_write_json(&path, &list);
    }

    pub fn save_history(&self) {
        let path = self.base_dir.join("history.json");
        let list = {
            let guard = self.data.lock().unwrap();
            guard.history.clone()
        };
        let _ = atomic_write_json(&path, &list);
    }
}

static STORE: OnceLock<Store> = OnceLock::new();

pub fn init(app: &tauri::AppHandle) {
    if STORE.get().is_some() {
        return;
    }
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("./data"));
    let _ = fs::create_dir_all(&dir);
    let _ = STORE.set(Store::new(dir));
}

fn get_store() -> &'static Store {
    STORE.get().expect("Store non initialisé")
}

#[tauri::command]
pub async fn bookmark_add(url: String, title: String) -> Result<bool, String> {
    let store = get_store();
    let res = {
        let mut guard = store.data.lock().unwrap();
        add_bookmark_pure(&mut guard.bookmarks, url, title)
    };
    if res {
        store.save_bookmarks();
    }
    Ok(res)
}

#[tauri::command]
pub async fn bookmark_remove(url: String) -> Result<bool, String> {
    let store = get_store();
    let res = {
        let mut guard = store.data.lock().unwrap();
        remove_bookmark_pure(&mut guard.bookmarks, &url)
    };
    if res {
        store.save_bookmarks();
    }
    Ok(res)
}

#[tauri::command]
pub async fn bookmark_list() -> Result<Vec<Bookmark>, String> {
    let store = get_store();
    let guard = store.data.lock().unwrap();
    Ok(guard.bookmarks.clone())
}

#[tauri::command]
pub async fn history_add(url: String, title: String) -> Result<bool, String> {
    let store = get_store();
    let res = {
        let mut guard = store.data.lock().unwrap();
        add_history_pure(&mut guard.history, url, title)
    };
    if res {
        store.save_history();
    }
    Ok(res)
}

#[tauri::command]
pub async fn history_list(
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<HistoryEntry>, String> {
    let store = get_store();
    let guard = store.data.lock().unwrap();
    Ok(filter_history_pure(&guard.history, query.as_deref(), limit))
}

#[tauri::command]
pub async fn history_clear() -> Result<(), String> {
    let store = get_store();
    {
        let mut guard = store.data.lock().unwrap();
        guard.history.clear();
    }
    store.save_history();
    Ok(())
}
