//! Interception réseau native WebView2 (WebResourceRequested) pour LightBrowser.
//!
//! Chaque requête (document, script, image, XHR, fetch, iframe...) est vérifiée
//! avant d'être envoyée. Si l'hôte est dans la blocklist, on répond 403 sans
//! jamais toucher le réseau. Le script JS d'adblock.rs reste en place pour le
//! masquage CSS et YouTube.
//!
//! À appeler pour CHAQUE webview de site créée (le LRU les détruit/recrée).
//! Le handler s'exécute sur le thread UI de WebView2 : rien de bloquant ici.

#[cfg(windows)]
pub fn install(view: &tauri::Webview) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_2, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
    };
    use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
    use windows::core::{w, Interface, PWSTR};

    view.with_webview(|platform| unsafe {
        let core = match platform.controller().CoreWebView2() {
            Ok(c) => c,
            Err(_) => return,
        };
        // L'environnement sert à fabriquer la réponse 403.
        let env = match core.cast::<ICoreWebView2_2>().and_then(|c| c.Environment()) {
            Ok(e) => e,
            Err(_) => return,
        };

        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let request = args.Request()?;

            let mut uri = PWSTR::null();
            request.Uri(&mut uri)?;
            let uri = take_pwstr(uri);

            if let Ok(parsed) = tauri::Url::parse(&uri) {
                if crate::adblock::block_request(parsed.host_str()) {
                    let response =
                        env.CreateWebResourceResponse(None, 403, w!("Blocked"), w!(""))?;
                    args.SetResponse(&response)?;
                }
            }
            Ok(())
        }));

        let mut token = Default::default();
        let _ = core.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
        let _ = core.add_WebResourceRequested(&handler, &mut token);
    })
    .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn install(_view: &tauri::Webview) -> Result<(), String> {
    Ok(())
}
