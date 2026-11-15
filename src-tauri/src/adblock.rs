//! Enveloppe Tauri pour le module adblock issu de lightbrowser-core.

pub use lightbrowser_core::adblock::*;

#[tauri::command]
pub async fn adblock_count() -> u64 {
    lightbrowser_core::adblock::adblock_count()
}
