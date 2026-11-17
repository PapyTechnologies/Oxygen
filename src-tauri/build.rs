const COMMANDS: &[&str] = &[
    "tab_show",
    "tab_close",
    "set_top",
    "ai_set_panel_open",
    "action_navigation",
    "active_view_visible",
    "exit_fullscreen",
    "toggle_fullscreen",
    "adblock_count",
    "ram_usage",
    "ram_diagnostic",
    "bookmark_add",
    "bookmark_remove",
    "bookmark_list",
    "history_add",
    "history_list",
    "history_clear",
    "start_stream",
    "stop_stream",
    "stream_status",
    "save_stream_key",
    "delete_stream_key",
    "has_stream_key",
    "list_av_devices",
    "ai_save_key",
    "ai_delete_key",
    "ai_has_key",
    "ai_check_key",
    "ai_chat",
    "ai_get_page",
    "ai_push_page_text",
    "ai_get_config",
    "ai_set_config",
    "ai_list_models",
    "downloads_set_panel_open",
    "downloads_get_history",
    "downloads_clear_history",
    "downloads_remove_item",
    "downloads_open_folder",
    "downloads_get_config",
    "downloads_set_config",
    "downloads_choose_folder",
];

fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=capabilities/default.json");
    println!("cargo:rerun-if-changed=capabilities/ai-remote.json");

    // Validation automatique de la cohérence ACL entre COMMANDS et default.json
    let default_json_str = std::fs::read_to_string("capabilities/default.json")
        .expect("Lecture capabilities/default.json");
    for &cmd in COMMANDS {
        let perm_name = format!("allow-{}", cmd.replace('_', "-"));
        if cmd == "ai_push_page_text" {
            assert!(
                !default_json_str.contains(&perm_name),
                "SÉCURITÉ : {perm_name} ne doit JAMAIS être dans default.json !"
            );
        } else {
            assert!(
                default_json_str.contains(&perm_name),
                "ACL MANQUANTE : {perm_name} doit être présente dans capabilities/default.json !"
            );
        }
    }

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build");
}
