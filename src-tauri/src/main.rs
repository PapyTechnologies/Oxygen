#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Configuration de réduction d'empreinte mémoire Chromium/WebView2 (sans --in-process-gpu pour stabilité VSync)
    let flags = [
        "--disable-crash-reporter",
        "--disable-breakpad",
        "--enable-features=NetworkServiceInProcess",
        "--renderer-process-limit=1",
        "--disable-background-networking",
        "--disable-background-timer-throttling",
        "--disable-backgrounding-occluded-windows",
        "--disable-renderer-backgrounding",
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,MediaRouter",
        "--disk-cache-size=33554432",
        "--media-cache-size=16777216",
    ].join(" ");

    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", flags);

    lightbrowser_lib::run();
}
