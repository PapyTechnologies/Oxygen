//! Logique pure d'adblocking et de filtrage d'hôtes pour LightBrowser.
//! Aucune dépendance à Tauri ou WebView2.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static BLOCKED: AtomicU64 = AtomicU64::new(0);

const BLOCKLIST_TXT: &str = include_str!("blocklist.txt");

pub fn parse_blocklist(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|l| {
            let l = l.split('#').next().unwrap_or("").trim();
            if l.is_empty() {
                return None;
            }
            let d = l.split_whitespace().last()?;
            if d == "localhost" || !d.contains('.') {
                None
            } else {
                Some(d)
            }
        })
        .collect()
}

pub fn domain_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| parse_blocklist(BLOCKLIST_TXT).into_iter().collect())
}

/// Sélecteurs CSS masqués visuellement.
pub const HIDE_SELECTORS: &[&str] = &[
    "ins.adsbygoogle",
    "[id^='google_ads']",
    "[id^='div-gpt-ad']",
    "[class*='ad-banner']",
    "[class*='adsbox']",
    "[class*='sponsored-']",
    "[data-ad-slot]",
    "[data-google-query-id]",
    "iframe[src*='doubleclick.net']",
    "iframe[src*='googlesyndication.com']",
    ".taboola, .trc_rbox_container, .OUTBRAIN",
];

/// Vrai si `host` est un domaine bloqué ou l'un de ses sous-domaines.
pub fn is_blocked_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let set = domain_set();
    let mut rest = host.as_str();
    loop {
        if set.contains(rest) {
            return true;
        }
        match rest.split_once('.') {
            Some((_, parent)) if parent.contains('.') => rest = parent,
            _ => return false,
        }
    }
}

/// Renvoie false (et incrémente le compteur) si bloqué.
pub fn allow_navigation(host: Option<&str>) -> bool {
    match host {
        Some(h) if is_blocked_host(h) => {
            BLOCKED.fetch_add(1, Ordering::Relaxed);
            false
        }
        _ => true,
    }
}

/// Pour l'interception native : renvoie true (et incrémente) si bloqué.
pub fn block_request(host: Option<&str>) -> bool {
    match host {
        Some(h) if is_blocked_host(h) => {
            BLOCKED.fetch_add(1, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

pub fn adblock_count() -> u64 {
    BLOCKED.load(Ordering::Relaxed)
}

pub fn js_array(items: &[&str]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", parts.join(","))
}

/// Script injecté avant le code de chaque page.
pub fn init_script() -> String {
    let template = r##"(function () {
  if (window.__lbAdblock) return;
  window.__lbAdblock = true;
  var DOMAINS = __DOMAINS__;
  var SELECTORS = __SELECTORS__;
  function blocked(u) {
    try {
      var h = new URL(u, location.href).hostname.toLowerCase();
      for (var i = 0; i < DOMAINS.length; i++) {
        var d = DOMAINS[i];
        if (h === d || h.endsWith("." + d)) return true;
      }
    } catch (e) {}
    return false;
  }
  // fetch / XHR / beacon
  var _fetch = window.fetch;
  if (_fetch) window.fetch = function (r) {
    var u = typeof r === "string" ? r : (r && r.url);
    if (u && blocked(u)) return Promise.reject(new TypeError("blocked"));
    return _fetch.apply(this, arguments);
  };
  var _open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (m, u) {
    this.__lbBlocked = blocked(u);
    return _open.apply(this, arguments);
  };
  var _send = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function () {
    if (this.__lbBlocked) return;
    return _send.apply(this, arguments);
  };
  if (navigator.sendBeacon) {
    var _beacon = navigator.sendBeacon.bind(navigator);
    navigator.sendBeacon = function (u, d) { return blocked(u) ? false : _beacon(u, d); };
  }
  // <script>, <iframe>, <img> ajoutés dynamiquement
  var _setAttr = Element.prototype.setAttribute;
  Element.prototype.setAttribute = function (n, v) {
    if ((n === "src" || n === "href") && /^(SCRIPT|IFRAME|IMG|LINK)$/.test(this.tagName) && blocked(v)) return;
    return _setAttr.apply(this, arguments);
  };
  [HTMLScriptElement, HTMLIFrameElement, HTMLImageElement].forEach(function (C) {
    var desc = Object.getOwnPropertyDescriptor(C.prototype, "src");
    if (!desc || !desc.set) return;
    Object.defineProperty(C.prototype, "src", {
      get: desc.get,
      set: function (v) { if (!blocked(v)) desc.set.call(this, v); },
      configurable: true
    });
  });
  // masquage visuel
  function addStyle() {
    if (document.getElementById("__lb_adblock_css")) return;
    var s = document.createElement("style");
    s.id = "__lb_adblock_css";
    s.textContent = SELECTORS.join(",") + "{display:none !important}";
    var root = document.head || document.documentElement;
    if (!root) return false;
    root.appendChild(s);
    return true;
  }
  if (!addStyle()) {
    var tries = 0;
    var t = setInterval(function () { if (addStyle() || ++tries > 50) clearInterval(t); }, 20);
  }
  document.addEventListener("DOMContentLoaded", addStyle);

  // ---- YouTube : pubs dans les listes + pubs vidéo ----
  if (/(^|\.)youtube\.com$/.test(location.hostname)) {
    var YT_HIDE = [
      "ytd-ad-slot-renderer", "ytd-in-feed-ad-layout-renderer", "ytd-display-ad-renderer",
      "ytd-promoted-sparkles-web-renderer", "ytd-promoted-video-renderer", "ytd-banner-promo-renderer",
      "ytd-statement-banner-renderer", "ytd-companion-slot-renderer", "ytd-player-legacy-desktop-watch-ads-renderer",
      "#masthead-ad", "#player-ads", ".ytd-merch-shelf-renderer", ".ytp-ad-overlay-container",
      ".ytp-ad-image-overlay", "tp-yt-paper-dialog:has(ytd-enforcement-message-view-model)"
    ];
    function ytStyle() {
      if (document.getElementById("__lb_yt_css")) return true;
      var root = document.head || document.documentElement;
      if (!root) return false;
      var st = document.createElement("style");
      st.id = "__lb_yt_css";
      st.textContent = YT_HIDE.join(",") + "{display:none !important}";
      root.appendChild(st);
      return true;
    }
    if (!ytStyle()) { var yt = setInterval(function () { if (ytStyle()) clearInterval(yt); }, 50); }
    // pub vidéo : cliquer sur "Passer", sinon accélérer + couper le son
    setInterval(function () {
      var player = document.querySelector(".html5-video-player");
      if (!player) return;
      var v = player.querySelector("video");
      var skip = player.querySelector(".ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-ad-skip-button-container button");
      if (skip) skip.click();
      if (player.classList.contains("ad-showing") && v) {
        v.muted = true;
        v.playbackRate = 16;
        if (isFinite(v.duration) && v.duration > 0 && v.currentTime < v.duration - 0.2) {
          try { v.currentTime = v.duration - 0.1; } catch (e) {}
        }
      } else if (v && v.playbackRate === 16) {
        v.playbackRate = 1;
        v.muted = false;
      }
    }, 300);
  }

  // retire les nœuds pub qui arrivent après coup
  function watch() {
    if (!document.documentElement) { setTimeout(watch, 20); return; }
    new MutationObserver(function (list) {
    list.forEach(function (m) {
      m.addedNodes.forEach(function (n) {
        if (n.nodeType === 1 && (n.tagName === "SCRIPT" || n.tagName === "IFRAME") && n.src && blocked(n.src)) n.remove();
      });
    });
    }).observe(document.documentElement, { childList: true, subtree: true });
  }
  watch();
})();"##;
    template
        .replace("__DOMAINS__", &js_array(&parse_blocklist(BLOCKLIST_TXT)))
        .replace("__SELECTORS__", &js_array(HIDE_SELECTORS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_matching() {
        assert!(is_blocked_host("doubleclick.net"));
        assert!(is_blocked_host("ad.doubleclick.net"));
        assert!(is_blocked_host("STATS.G.DOUBLECLICK.NET."));
        assert!(!is_blocked_host("notdoubleclick.net"));
        assert!(!is_blocked_host("github.com"));
        assert!(!is_blocked_host("wikipedia.org"));
        assert!(is_blocked_host("media.net"));
        assert!(is_blocked_host("analytics.google.com"));
    }

    #[test]
    fn navigation_counts() {
        let before = BLOCKED.load(Ordering::Relaxed);
        assert!(!allow_navigation(Some("ads.doubleclick.net")));
        assert!(allow_navigation(Some("duckduckgo.com")));
        assert!(allow_navigation(None));
        assert_eq!(BLOCKED.load(Ordering::Relaxed), before + 1);
    }

    #[test]
    fn script_is_filled() {
        let s = init_script();
        assert!(!s.contains("__DOMAINS__") && !s.contains("__SELECTORS__"));
        assert!(s.contains("\"doubleclick.net\""));
    }
}
