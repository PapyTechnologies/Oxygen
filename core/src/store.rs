//! Logique pure des favoris et de l'historique de LightBrowser.
//! Aucune dépendance à Tauri ou WebView2.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_HISTORY_ENTRIES: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub added_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visited_at: u64,
}

pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn should_record_url(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.starts_with("about:") {
        return false;
    }
    // Si l'hôte est bloqué, ne pas consigner dans l'historique
    if let Some(host) = extract_host(u) {
        if crate::adblock::is_blocked_host(&host) {
            return false;
        }
    }
    true
}

fn extract_host(url: &str) -> Option<String> {
    let u = url.trim();
    let without_proto = if let Some(idx) = u.find("://") {
        &u[idx + 3..]
    } else {
        u
    };
    let host_part = without_proto.split(&['/', '?', '#', ':'][..]).next()?;
    if host_part.is_empty() {
        None
    } else {
        Some(host_part.to_ascii_lowercase())
    }
}

pub fn add_bookmark_pure(list: &mut Vec<Bookmark>, url: String, title: String) -> bool {
    let u = url.trim();
    if u.is_empty() {
        return false;
    }
    if let Some(existing) = list.iter_mut().find(|b| b.url == u) {
        existing.title = title;
        return true;
    }
    list.push(Bookmark {
        url: u.to_string(),
        title,
        added_at: now_timestamp(),
    });
    true
}

pub fn remove_bookmark_pure(list: &mut Vec<Bookmark>, url: &str) -> bool {
    let len_before = list.len();
    list.retain(|b| b.url != url.trim());
    list.len() < len_before
}

pub fn add_history_pure(list: &mut Vec<HistoryEntry>, url: String, title: String) -> bool {
    if !should_record_url(&url) {
        return false;
    }
    let u = url.trim().to_string();
    if let Some(last) = list.first() {
        if last.url == u {
            return false;
        }
    }
    list.insert(
        0,
        HistoryEntry {
            url: u,
            title,
            visited_at: now_timestamp(),
        },
    );
    if list.len() > MAX_HISTORY_ENTRIES {
        list.truncate(MAX_HISTORY_ENTRIES);
    }
    true
}

pub fn filter_history_pure(
    list: &[HistoryEntry],
    query: Option<&str>,
    limit: Option<usize>,
) -> Vec<HistoryEntry> {
    let q = query.unwrap_or("").trim().to_lowercase();
    let max = limit.unwrap_or(50);
    list.iter()
        .filter(|e| {
            if q.is_empty() {
                true
            } else {
                e.url.to_lowercase().contains(&q) || e.title.to_lowercase().contains(&q)
            }
        })
        .take(max)
        .cloned()
        .collect()
}

pub fn load_json_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => T::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bookmark_add_update_remove() {
        let mut list = Vec::new();
        assert!(add_bookmark_pure(
            &mut list,
            "https://wikipedia.org".into(),
            "Wikipedia".into()
        ));
        assert_eq!(list.len(), 1);
        // Même URL -> mise à jour du titre, pas de doublon
        assert!(add_bookmark_pure(
            &mut list,
            "https://wikipedia.org".into(),
            "Wiki".into()
        ));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Wiki");
        // Suppression
        assert!(remove_bookmark_pure(&mut list, "https://wikipedia.org"));
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_history_duplicates_and_cap() {
        let mut list = Vec::new();
        assert!(add_history_pure(
            &mut list,
            "https://duckduckgo.com".into(),
            "DuckDuckGo".into()
        ));
        // Doublon consécutif ignoré
        assert!(!add_history_pure(
            &mut list,
            "https://duckduckgo.com".into(),
            "DuckDuckGo".into()
        ));
        assert_eq!(list.len(), 1);

        assert!(add_history_pure(
            &mut list,
            "https://github.com".into(),
            "GitHub".into()
        ));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].url, "https://github.com");

        // Plafonnement à 2000 entrées
        for i in 0..2100 {
            add_history_pure(
                &mut list,
                format!("https://site{i}.org"),
                format!("Site {i}"),
            );
        }
        assert_eq!(list.len(), MAX_HISTORY_ENTRIES);
    }

    #[test]
    fn test_corrupted_json_fallback() {
        let temp_dir = std::env::temp_dir().join("lightbrowser_test_store_core");
        let _ = fs::create_dir_all(&temp_dir);
        let corrupt_file = temp_dir.join("history_corrupt.json");
        let _ = fs::write(&corrupt_file, "{ malformed json non valide !");
        let loaded: Vec<HistoryEntry> = load_json_or_default(&corrupt_file);
        assert!(loaded.is_empty());
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_history() {
        let list = vec![
            HistoryEntry {
                url: "https://doc.rust-lang.org".into(),
                title: "Rust Doc".into(),
                visited_at: 100,
            },
            HistoryEntry {
                url: "https://duckduckgo.com".into(),
                title: "DuckDuckGo".into(),
                visited_at: 90,
            },
        ];
        let res = filter_history_pure(&list, Some("rust"), None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].title, "Rust Doc");
    }
}
