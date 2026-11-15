// Seuils d'alerte mémoire RAM (Mo)
const RAM_WARN_MB = 800;
const RAM_ALERT_MB = 1500;
let lastDisplayedRamMb = null;

const urlInput = document.getElementById("url");
const goButton = document.getElementById("go");
const backButton = document.getElementById("back");
const forwardButton = document.getElementById("forward");
const reloadButton = document.getElementById("reload");
const tabsList = document.getElementById("tabs-list");
const newTabBtn = document.getElementById("new-tab-btn");
const newPrivateTabBtn = document.getElementById("new-private-tab-btn");
const startPage = document.getElementById("start-page");
const privateBadge = document.getElementById("private-badge");
const toolbarEl = document.querySelector(".toolbar");
const starBtn = document.getElementById("star");
const suggestionsBox = document.getElementById("suggestions");
const bookmarksListEl = document.getElementById("bookmarks-list");
const historyListEl = document.getElementById("history-list");
const historySearchInput = document.getElementById("history-search");
const clearHistoryBtn = document.getElementById("clear-history-btn");

// Barre de favoris et panneau déroulant
const bookmarksBar = document.getElementById("bookmarks-bar");
const bookmarksBarList = document.getElementById("bookmarks-bar-list");
const bookmarksBarOverflow = document.getElementById("bookmarks-bar-overflow");
const bookmarksMenuBtn = document.getElementById("bookmarks-menu-btn");
const bookmarksDropdown = document.getElementById("bookmarks-dropdown");
const dropdownSearch = document.getElementById("dropdown-search");
const dropdownClose = document.getElementById("dropdown-close");
const dropdownBookmarks = document.getElementById("dropdown-bookmarks");
const dropdownHistory = document.getElementById("dropdown-history");

// Panneau de diffusion
const streamMenuBtn = document.getElementById("stream-menu-btn");
const streamDropdown = document.getElementById("stream-dropdown");
const streamCloseBtn = document.getElementById("stream-close-btn");
const streamSource = document.getElementById("stream-source");
const streamTarget = document.getElementById("stream-target");
const destYoutube = document.getElementById("dest-youtube");
const destTwitch = document.getElementById("dest-twitch");
const destKick = document.getElementById("dest-kick");
const destTiktok = document.getElementById("dest-tiktok");
const urlKick = document.getElementById("url-kick");
const urlTiktok = document.getElementById("url-tiktok");
const keyPlatform = document.getElementById("key-platform");
const streamKeyInput = document.getElementById("stream-key-input");
const saveKeyBtn = document.getElementById("save-key-btn");
const startStreamBtn = document.getElementById("start-stream-btn");
const stopStreamBtn = document.getElementById("stop-stream-btn");
const streamIndicator = document.getElementById("stream-indicator");
const streamMic = document.getElementById("stream-mic");
const streamSys = document.getElementById("stream-sys");
const streamCam = document.getElementById("stream-cam");
const micVol = document.getElementById("mic-vol");
const micVolVal = document.getElementById("mic-vol-val");
const sysVol = document.getElementById("sys-vol");
const sysVolVal = document.getElementById("sys-vol-val");
const streamOverlay = document.getElementById("stream-overlay");
const streamOverlayControls = document.getElementById("stream-overlay-controls");
const streamCorner = document.getElementById("stream-corner");
const streamOvSize = document.getElementById("stream-ov-size");
const streamOvSizeVal = document.getElementById("stream-ov-size-val");
const streamOvMargin = document.getElementById("stream-ov-margin");
const streamOvMarginVal = document.getElementById("stream-ov-margin-val");

let currentBookmarks = [];
let suggestionsVisible = false;
let dropdownOpen = false;
let streamDropdownOpen = false;
let toastTimer = null;

function showToast(message, durationMs = 3000) {
    const toast = document.getElementById("toast");
    if (!toast) return;
    toast.textContent = message;
    toast.style.display = "flex";
    void toast.offsetWidth;
    toast.classList.add("show");

    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
        toast.classList.remove("show");
        setTimeout(() => {
            if (!toast.classList.contains("show")) {
                toast.style.display = "none";
            }
        }, 250);
        toastTimer = null;
    }, durationMs);
}

let tabs = [
    { id: "tab-1", title: "Nouvel onglet", url: "", isPrivate: false }
];
let activeTabId = "tab-1";

async function invokeTauri(cmd, args = {}) {
    if (window.__TAURI__ && window.__TAURI__.core) {
        return await window.__TAURI__.core.invoke(cmd, args);
    } else if (window.__TAURI_INTERNALS__) {
        return await window.__TAURI_INTERNALS__.invoke(cmd, args);
    } else {
        console.error("API Tauri introuvable. Vérifiez withGlobalTauri dans tauri.conf.json");
    }
}

function hostOf(u) {
    try { return new URL(u).hostname || u; } catch (_) { return u; }
}

function normalizeUrl(value) {
    const text = value.trim();
    if (!text) return null;
    const hasProtocol = /^https?:\/\//i.test(text);
    const looksLikeDomain = /^[\w-]+(\.[\w-]+)+(:\d+)?([\/?#]|$)/.test(text);
    const isLocal = /^(localhost|\d{1,3}(\.\d{1,3}){3})(:\d+)?([\/?#]|$)/i.test(text);
    if (hasProtocol) return text;
    if (isLocal) return `http://${text}`;
    if (looksLikeDomain) return `https://${text}`;
    return `https://duckduckgo.com/?q=${encodeURIComponent(text)}`;
}

function computeTopPx() {
    const bar = document.getElementById("tabs-bar");
    const tool = document.querySelector(".toolbar");
    const bBar = document.getElementById("bookmarks-bar");
    let top = 0;
    if (bar) top += bar.getBoundingClientRect().height;
    if (tool) top += tool.getBoundingClientRect().height;
    if (bBar && bBar.style.display !== "none") {
        top += bBar.getBoundingClientRect().height;
    }
    return Math.ceil(top);
}

async function syncTop() {
    const topPx = computeTopPx();
    try {
        await invokeTauri("set_top", { topPx });
    } catch (e) {
        console.error("Erreur set_top :", e);
    }
}

function getBookmarksBarPref() {
    try {
        const v = localStorage.getItem("lb_show_bookmarks_bar");
        return v === null ? true : v === "1";
    } catch (_) {
        return true;
    }
}

function setBookmarksBarPref(show) {
    try {
        localStorage.setItem("lb_show_bookmarks_bar", show ? "1" : "0");
    } catch (_) {}
}

async function toggleBookmarksBarPref() {
    const current = getBookmarksBarPref();
    setBookmarksBarPref(!current);
    await updateBookmarksBar();
}

function updateStarButton() {
    if (!starBtn) return;
    const current = tabs.find(t => t.id === activeTabId);
    const url = current && current.url ? current.url : urlInput.value.trim();
    const isStarred = Boolean(url && currentBookmarks.some(b => b.url === url));
    starBtn.classList.toggle("starred", isStarred);
    if (current && current.isPrivate) {
        starBtn.title = isStarred ? "★ Enregistré dans vos favoris (clic pour retirer)" : "Ajouter aux favoris";
    } else {
        starBtn.title = isStarred ? "Retirer des favoris" : "Ajouter aux favoris";
    }
}

async function updateBookmarksBar() {
    if (!bookmarksBar || !bookmarksBarList) return;
    const shouldShow = getBookmarksBarPref() && currentBookmarks.length > 0;
    const wasVisible = bookmarksBar.style.display !== "none";

    if (shouldShow) {
        bookmarksBar.style.display = "flex";
        bookmarksBarList.innerHTML = "";
        currentBookmarks.forEach(b => {
            const el = document.createElement("div");
            el.className = "bookmark-bar-item";
            el.title = b.url;

            const title = document.createElement("span");
            title.className = "bookmark-bar-title";
            title.textContent = b.title || hostOf(b.url);

            const close = document.createElement("span");
            close.className = "bookmark-bar-close";
            close.textContent = "×";
            close.title = "Retirer";
            close.addEventListener("click", async (e) => {
                e.stopPropagation();
                await invokeTauri("bookmark_remove", { url: b.url });
                await refreshBookmarks();
            });

            el.append(title, close);
            el.addEventListener("click", (e) => {
                if (e.target === close) return;
                const current = tabs.find(t => t.id === activeTabId);
                const isPriv = Boolean(current && current.isPrivate);
                if (e.ctrlKey) {
                    createTab(b.url, isPriv);
                } else {
                    urlInput.value = b.url;
                    openUrl();
                }
            });
            el.addEventListener("auxclick", (e) => {
                if (e.button === 1) {
                    e.preventDefault();
                    const current = tabs.find(t => t.id === activeTabId);
                    createTab(b.url, Boolean(current && current.isPrivate));
                }
            });
            bookmarksBarList.appendChild(el);
        });

        const hasOverflow = bookmarksBarList.scrollWidth > bookmarksBarList.clientWidth;
        if (bookmarksBarOverflow) {
            bookmarksBarOverflow.style.display = hasOverflow ? "flex" : "none";
        }
    } else {
        bookmarksBar.style.display = "none";
    }

    if (wasVisible !== shouldShow) {
        await syncTop();
    }
}

async function refreshBookmarks() {
    try {
        currentBookmarks = (await invokeTauri("bookmark_list")) || [];
        updateStarButton();
        await updateBookmarksBar();

        if (!bookmarksListEl) return;
        bookmarksListEl.innerHTML = "";
        if (currentBookmarks.length === 0) {
            bookmarksListEl.innerHTML = '<span class="empty-hint">Aucun favori pour le moment</span>';
            return;
        }
        currentBookmarks.forEach(b => {
            const card = document.createElement("div");
            card.className = "bookmark-card";
            const title = document.createElement("span");
            title.className = "bookmark-title";
            title.textContent = b.title || hostOf(b.url);
            title.title = b.url;

            const remove = document.createElement("span");
            remove.className = "bookmark-remove";
            remove.textContent = "×";
            remove.title = "Supprimer ce favori";
            remove.addEventListener("click", async (e) => {
                e.stopPropagation();
                await invokeTauri("bookmark_remove", { url: b.url });
                await refreshBookmarks();
            });

            card.append(title, remove);
            card.addEventListener("click", () => {
                urlInput.value = b.url;
                openUrl();
            });
            bookmarksListEl.appendChild(card);
        });
    } catch (e) {
        console.error("Erreur refreshBookmarks:", e);
    }
}

async function refreshHistory(query = "") {
    if (!historyListEl) return;
    try {
        const list = (await invokeTauri("history_list", { query: query.trim() || null, limit: 50 })) || [];
        historyListEl.innerHTML = "";
        if (list.length === 0) {
            historyListEl.innerHTML = '<span class="empty-hint">Historique vide</span>';
            return;
        }
        list.forEach(h => {
            const row = document.createElement("div");
            row.className = "history-row";
            const title = document.createElement("span");
            title.className = "history-title";
            title.textContent = h.title || hostOf(h.url);
            const urlSpan = document.createElement("span");
            urlSpan.className = "history-url";
            urlSpan.textContent = h.url;
            row.append(title, urlSpan);
            row.addEventListener("click", () => {
                urlInput.value = h.url;
                openUrl();
            });
            historyListEl.appendChild(row);
        });
    } catch (e) {
        console.error("Erreur refreshHistory:", e);
    }
}

async function setAiPanelOpen(open) {
    const panel = document.getElementById("ai-panel");
    if (!panel) return;
    if (open) {
        closeDropdown();
        closeStreamDropdown();
        hideSuggestions();
        panel.hidden = false;
        try {
            await invokeTauri("ai_set_panel_open", { open: true });
        } catch (e) {
            console.error("Erreur ai_set_panel_open(true) :", e);
        }
        if (typeof aiLoadConfig === "function") {
            await aiLoadConfig();
        }
    } else {
        panel.hidden = true;
        try {
            await invokeTauri("ai_set_panel_open", { open: false });
        } catch (e) {
            console.error("Erreur ai_set_panel_open(false) :", e);
        }
    }
}

async function openDropdown() {
    if (dropdownOpen) return;
    dropdownOpen = true;
    closeStreamDropdown();
    setAiPanelOpen(false);
    hideSuggestions();
    if (bookmarksDropdown) bookmarksDropdown.style.display = "flex";
    if (dropdownSearch) dropdownSearch.value = "";
    await renderDropdownContent();
    const current = tabs.find(t => t.id === activeTabId);
    if (current && current.url) {
        await invokeTauri("active_view_visible", { visible: false });
    }
}

async function closeDropdown() {
    if (!dropdownOpen) return;
    dropdownOpen = false;
    if (bookmarksDropdown) bookmarksDropdown.style.display = "none";
    const current = tabs.find(t => t.id === activeTabId);
    if (current && current.url && !streamDropdownOpen) {
        await invokeTauri("active_view_visible", { visible: true });
    }
}

function updateStreamUi(isStreaming) {
    if (streamIndicator) {
        if (isStreaming) {
            streamIndicator.textContent = "🔴 En direct";
            streamIndicator.classList.add("live");
        } else {
            streamIndicator.textContent = "⚪ Inactif";
            streamIndicator.classList.remove("live");
        }
    }
    if (startStreamBtn) startStreamBtn.disabled = isStreaming;
    if (stopStreamBtn) stopStreamBtn.disabled = !isStreaming;
    if (streamMenuBtn) {
        streamMenuBtn.style.color = isStreaming ? "#ef4444" : "";
    }
}

async function loadStreamDevices() {
    try {
        const d = await invokeTauri("list_av_devices");
        if (!d) return;
        const fill = (el, items, withNone) => {
            if (!el) return;
            const cur = el.value;
            el.replaceChildren();
            if (withNone) el.append(new Option("Aucun", ""));
            (items || []).forEach(n => el.append(new Option(n, n)));
            if (cur && Array.from(el.options).some(o => o.value === cur)) {
                el.value = cur;
            }
        };
        const fillSys = (el, items) => {
            if (!el) return;
            const cur = el.value || "loopback:default";
            el.replaceChildren();
            el.append(new Option("Son de l'ordinateur (automatique)", "loopback:default"));
            el.append(new Option("Aucun", ""));
            (items || []).forEach(n => el.append(new Option(n, n)));
            if (Array.from(el.options).some(o => o.value === cur)) {
                el.value = cur;
            } else {
                el.value = "loopback:default";
            }
        };
        fill(streamMic, d.audio, true);
        fillSys(streamSys, d.audio);
        fill(streamCam, d.video, false);
    } catch (e) {
        console.error("Erreur list_av_devices :", e);
    }
}

let screenConfirmTimeout = null;
let screenConfirmed = false;

function resetScreenConfirm() {
    if (screenConfirmTimeout) {
        clearTimeout(screenConfirmTimeout);
        screenConfirmTimeout = null;
    }
    screenConfirmed = false;
    if (startStreamBtn && !startStreamBtn.disabled) {
        startStreamBtn.textContent = "Démarrer";
    }
}

async function openStreamDropdown() {
    if (streamDropdownOpen) return;
    streamDropdownOpen = true;
    await closeDropdown();
    setAiPanelOpen(false);
    hideSuggestions();
    if (streamDropdown) streamDropdown.style.display = "flex";
    if (streamSource) streamSource.value = "AppWindow";
    const warn = document.getElementById("stream-screen-warn");
    if (warn) warn.style.display = "none";
    if (streamTarget) streamTarget.style.display = "none";
    updateCamVisibility();
    resetScreenConfirm();
    const current = tabs.find(t => t.id === activeTabId);
    if (current && current.url) {
        await invokeTauri("active_view_visible", { visible: false });
    }
    await loadStreamDevices();
    await updateKeyIndicators();
    try {
        const live = await invokeTauri("stream_status");
        updateStreamUi(Boolean(live));
    } catch (_) {}
}

async function closeStreamDropdown() {
    if (!streamDropdownOpen) return;
    streamDropdownOpen = false;
    if (streamDropdown) streamDropdown.style.display = "none";
    const current = tabs.find(t => t.id === activeTabId);
    if (current && current.url && !dropdownOpen) {
        await invokeTauri("active_view_visible", { visible: true });
    }
}

async function renderDropdownContent(query = "") {
    const q = query.trim().toLowerCase();
    if (dropdownBookmarks) {
        dropdownBookmarks.innerHTML = "";
        const filteredB = currentBookmarks.filter(b => !q || b.url.toLowerCase().includes(q) || b.title.toLowerCase().includes(q));
        if (filteredB.length === 0) {
            dropdownBookmarks.innerHTML = '<span class="empty-hint" style="padding:4px 12px;">Aucun favori</span>';
        } else {
            filteredB.forEach(b => {
                const item = document.createElement("div");
                item.className = "dropdown-item";
                const title = document.createElement("span");
                title.className = "dropdown-item-title";
                title.textContent = b.title || hostOf(b.url);
                title.title = b.url;

                const remove = document.createElement("span");
                remove.className = "dropdown-item-remove";
                remove.textContent = "×";
                remove.title = "Supprimer";
                remove.addEventListener("click", async (e) => {
                    e.stopPropagation();
                    await invokeTauri("bookmark_remove", { url: b.url });
                    await refreshBookmarks();
                    await renderDropdownContent(dropdownSearch ? dropdownSearch.value : "");
                });

                item.append(title, remove);
                item.addEventListener("click", async (e) => {
                    if (e.target === remove) return;
                    const current = tabs.find(t => t.id === activeTabId);
                    const isPriv = Boolean(current && current.isPrivate);
                    if (e.ctrlKey) {
                        createTab(b.url, isPriv);
                    } else {
                        urlInput.value = b.url;
                        await closeDropdown();
                        openUrl();
                    }
                });
                item.addEventListener("auxclick", (e) => {
                    if (e.button === 1) {
                        e.preventDefault();
                        const current = tabs.find(t => t.id === activeTabId);
                        createTab(b.url, Boolean(current && current.isPrivate));
                    }
                });
                dropdownBookmarks.appendChild(item);
            });
        }
    }

    if (dropdownHistory) {
        dropdownHistory.innerHTML = "";
        const history = (await invokeTauri("history_list", { query: q || null, limit: 20 })) || [];
        if (history.length === 0) {
            dropdownHistory.innerHTML = '<span class="empty-hint" style="padding:4px 12px;">Historique vide</span>';
        } else {
            history.forEach(h => {
                const item = document.createElement("div");
                item.className = "dropdown-item";
                const title = document.createElement("span");
                title.className = "dropdown-item-title";
                title.textContent = h.title || hostOf(h.url);
                title.title = h.url;
                item.appendChild(title);
                item.addEventListener("click", async (e) => {
                    const current = tabs.find(t => t.id === activeTabId);
                    const isPriv = Boolean(current && current.isPrivate);
                    if (e.ctrlKey) {
                        createTab(h.url, isPriv);
                    } else {
                        urlInput.value = h.url;
                        await closeDropdown();
                        openUrl();
                    }
                });
                item.addEventListener("auxclick", (e) => {
                    if (e.button === 1) {
                        e.preventDefault();
                        const current = tabs.find(t => t.id === activeTabId);
                        createTab(h.url, Boolean(current && current.isPrivate));
                    }
                });
                dropdownHistory.appendChild(item);
            });
        }
    }
}

function renderTabs() {
    tabsList.innerHTML = "";
    tabs.forEach(tab => {
        const el = document.createElement("div");
        el.className = `tab ${tab.id === activeTabId ? "active" : ""} ${tab.isPrivate ? "private" : ""}`;
        const title = document.createElement("span");
        title.className = "tab-title";
        if (tab.isPrivate) {
            title.innerHTML = `<span class="tab-private-icon">🕵️</span>${tab.title}`;
        } else {
            title.textContent = tab.title;
        }
        const close = document.createElement("span");
        close.className = "tab-close";
        close.dataset.id = tab.id;
        close.textContent = "×";
        el.append(title, close);
        el.addEventListener("click", (e) => {
            if (e.target.classList.contains("tab-close")) {
                closeTab(tab.id);
            } else {
                switchTab(tab.id);
            }
        });
        tabsList.appendChild(el);
    });
}

async function switchTab(id) {
    activeTabId = id;
    hideSuggestions();
    closeDropdown();
    const current = tabs.find(t => t.id === id);
    if (current) {
        urlInput.value = current.url;
        updateStarButton();

        // Teinte et indicateurs de mode privé
        if (toolbarEl) {
            toolbarEl.classList.toggle("private-mode", Boolean(current.isPrivate));
        }
        if (privateBadge) {
            privateBadge.style.display = current.isPrivate ? "block" : "none";
        }
        // Masquer l'historique sur la start page en navigation privée
        const historySec = historyListEl ? historyListEl.closest(".store-section") : null;
        if (historySec) {
            if (current.isPrivate) {
                historySec.style.display = "none";
            } else {
                historySec.style.display = "block";
            }
        }

        if (current.url) {
            if (startPage) startPage.style.display = "none";
            await navigateChildView(current.url, false);
        } else {
            if (startPage) startPage.style.display = "flex";
            await navigateChildView("", false);
            refreshBookmarks();
            if (!current.isPrivate) refreshHistory();
        }
    }
    renderTabs();
}

function createTab(url = "", isPrivate = false) {
    const newId = `tab-${Date.now()}`;
    const defaultTitle = isPrivate ? "Onglet privé" : "Nouvel onglet";
    const title = url ? hostOf(url) : defaultTitle;
    tabs.push({ id: newId, title, url, isPrivate });
    switchTab(newId);
}

function closeTab(id) {
    closeDropdown();
    closeStreamDropdown();
    invokeTauri("tab_close", { tabId: id });
    if (tabs.length === 1) {
        tabs[0] = { id: `tab-${Date.now()}`, title: "Nouvel onglet", url: "", isPrivate: false };
        switchTab(tabs[0].id);
        return;
    }
    const idx = tabs.findIndex(t => t.id === id);
    tabs = tabs.filter(t => t.id !== id);
    if (activeTabId === id) {
        const nextTab = tabs[Math.max(0, idx - 1)];
        switchTab(nextTab.id);
    } else {
        renderTabs();
    }
}

async function navigateChildView(target, navigate = true) {
    try {
        const topPx = computeTopPx();
        const current = tabs.find(t => t.id === activeTabId);
        const incognito = Boolean(current && current.isPrivate);
        await window.__TAURI__.core.invoke("tab_show", {
            tabId: activeTabId,
            url: target,
            navigate,
            topPx,
            incognito
        });
    } catch (e) {
        console.error("Erreur tab_show :", e);
        urlInput.value = "Erreur : " + e;
    }
}

function openUrl() {
    hideSuggestions();
    closeDropdown();
    const target = normalizeUrl(urlInput.value);
    if (!target) return;
    const current = tabs.find(t => t.id === activeTabId);
    if (current) {
        current.url = target;
        current.title = hostOf(target);
    }
    if (startPage) startPage.style.display = "none";
    updateStarButton();
    renderTabs();
    navigateChildView(target);
}

// Suggestions sous la barre d'adresse
async function showSuggestions(items) {
    if (!suggestionsBox || !items || items.length === 0) {
        hideSuggestions();
        return;
    }
    suggestionsBox.innerHTML = "";
    items.forEach(item => {
        const div = document.createElement("div");
        div.className = "suggestion-item";
        const title = document.createElement("span");
        title.className = "suggestion-title";
        title.textContent = `${item.title} — ${item.url}`;
        const badge = document.createElement("span");
        badge.className = "suggestion-type";
        badge.textContent = item.type === "bookmark" ? "⭐ Favori" : "🕒 Historique";
        div.append(title, badge);
        div.addEventListener("mousedown", (e) => {
            e.preventDefault();
            urlInput.value = item.url;
            hideSuggestions();
            openUrl();
        });
        suggestionsBox.appendChild(div);
    });
    suggestionsBox.style.display = "block";
    if (!suggestionsVisible) {
        suggestionsVisible = true;
        const current = tabs.find(t => t.id === activeTabId);
        if (current && current.url) {
            await invokeTauri("active_view_visible", { visible: false });
        }
    }
}

async function hideSuggestions() {
    if (!suggestionsBox || suggestionsBox.style.display === "none") return;
    suggestionsBox.style.display = "none";
    if (suggestionsVisible) {
        suggestionsVisible = false;
        const current = tabs.find(t => t.id === activeTabId);
        if (current && current.url) {
            await invokeTauri("active_view_visible", { visible: true });
        }
    }
}

urlInput.addEventListener("input", async (e) => {
    const q = e.target.value.trim().toLowerCase();
    if (!q) {
        hideSuggestions();
        return;
    }
    const matchingBookmarks = currentBookmarks
        .filter(b => b.url.toLowerCase().includes(q) || b.title.toLowerCase().includes(q))
        .map(b => ({ url: b.url, title: b.title, type: "bookmark" }));

    // Les suggestions ne puisent l'historique que pour les onglets normaux
    const current = tabs.find(t => t.id === activeTabId);
    let historyMatches = [];
    if (!current || !current.isPrivate) {
        historyMatches = (await invokeTauri("history_list", { query: q, limit: 5 }) || [])
            .filter(h => !matchingBookmarks.some(b => b.url === h.url))
            .map(h => ({ url: h.url, title: h.title, type: "history" }));
    }

    const combined = [...matchingBookmarks, ...historyMatches].slice(0, 5);
    showSuggestions(combined);
});

starBtn.addEventListener("click", async () => {
    const current = tabs.find(t => t.id === activeTabId);
    const url = current && current.url ? current.url : urlInput.value.trim();
    if (!url || url.startsWith("about:")) return;
    const isStarred = currentBookmarks.some(b => b.url === url);
    if (isStarred) {
        await invokeTauri("bookmark_remove", { url });
    } else {
        const title = current && current.title ? current.title : hostOf(url);
        await invokeTauri("bookmark_add", { url, title });
    }
    await refreshBookmarks();
});

if (bookmarksMenuBtn) {
    bookmarksMenuBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        if (dropdownOpen) {
            closeDropdown();
        } else {
            openDropdown();
        }
    });
}

if (bookmarksBarOverflow) {
    bookmarksBarOverflow.addEventListener("click", (e) => {
        e.stopPropagation();
        openDropdown();
    });
}

if (dropdownClose) {
    dropdownClose.addEventListener("click", () => closeDropdown());
}

if (dropdownSearch) {
    dropdownSearch.addEventListener("input", (e) => {
        renderDropdownContent(e.target.value);
    });
}

document.addEventListener("click", (e) => {
    if (!e.target.closest(".url-bar-container")) {
        hideSuggestions();
    }
    if (dropdownOpen && !e.target.closest("#bookmarks-dropdown") && !e.target.closest("#bookmarks-menu-btn") && !e.target.closest("#bookmarks-bar-overflow")) {
        closeDropdown();
    }
    if (streamDropdownOpen && !e.target.closest("#stream-dropdown") && !e.target.closest("#stream-menu-btn")) {
        closeStreamDropdown();
    }
});

document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
        hideSuggestions();
        closeDropdown();
        closeStreamDropdown();
        setAiPanelOpen(false);
    }
    if (e.ctrlKey && e.shiftKey && (e.key === "b" || e.key === "B")) {
        e.preventDefault();
        toggleBookmarksBarPref();
    }
    if (e.ctrlKey && e.shiftKey && (e.key === "n" || e.key === "N")) {
        e.preventDefault();
        createTab("", true);
    }
});

window.addEventListener("resize", async () => {
    closeDropdown();
    await syncTop();
});

goButton.addEventListener("click", openUrl);

urlInput.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || !e.ctrlKey) return;
    const t = urlInput.value.trim();
    if (!t || /\s/.test(t) || t.includes(".") || t.includes(":")) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    urlInput.value = t + ".com";
    openUrl();
}, true);

urlInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") openUrl();
});

newTabBtn.addEventListener("click", () => createTab());
if (newPrivateTabBtn) {
    newPrivateTabBtn.addEventListener("click", () => createTab("", true));
}

document.querySelectorAll("[data-url]").forEach((button) => {
    button.addEventListener("click", () => {
        urlInput.value = button.dataset.url;
        openUrl();
    });
});

backButton.addEventListener("click", () => invokeTauri("action_navigation", { action: "back" }));
forwardButton.addEventListener("click", () => invokeTauri("action_navigation", { action: "forward" }));
reloadButton.addEventListener("click", () => invokeTauri("action_navigation", { action: "reload" }));

if (clearHistoryBtn) {
    let confirmTimer = null;
    clearHistoryBtn.addEventListener("click", async () => {
        if (!clearHistoryBtn.classList.contains("confirming")) {
            clearHistoryBtn.classList.add("confirming");
            clearHistoryBtn.textContent = "Confirmer ?";
            if (confirmTimer) clearTimeout(confirmTimer);
            confirmTimer = setTimeout(() => {
                clearHistoryBtn.classList.remove("confirming");
                clearHistoryBtn.textContent = "Effacer";
                confirmTimer = null;
            }, 3000);
            return;
        }

        if (confirmTimer) {
            clearTimeout(confirmTimer);
            confirmTimer = null;
        }
        clearHistoryBtn.classList.remove("confirming");
        clearHistoryBtn.textContent = "Effacer";
        await invokeTauri("history_clear");
        await refreshHistory();
        if (dropdownOpen) await renderDropdownContent(dropdownSearch ? dropdownSearch.value : "");
        showToast("Historique de navigation effacé.");
    });
}

if (historySearchInput) {
    historySearchInput.addEventListener("input", (e) => {
        refreshHistory(e.target.value);
    });
}

// Contrôles de diffusion en direct
if (streamMenuBtn) {
    streamMenuBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        if (streamDropdownOpen) {
            closeStreamDropdown();
        } else {
            openStreamDropdown();
        }
    });
}

if (streamCloseBtn) {
    streamCloseBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        closeStreamDropdown();
    });
}

function updateCamVisibility() {
    const isWebcamSource = streamSource && streamSource.value === "Webcam";
    const isOverlayOn = streamOverlay && streamOverlay.checked && !isWebcamSource;
    if (streamCam) {
        streamCam.style.display = (isWebcamSource || isOverlayOn) ? "block" : "none";
    }
}

if (streamSource) {
    streamSource.addEventListener("change", () => {
        const val = streamSource.value;
        const warn = document.getElementById("stream-screen-warn");
        if (warn) warn.style.display = val === "Screen" ? "block" : "none";
        resetScreenConfirm();
        if (streamTarget) streamTarget.style.display = val === "Window" ? "block" : "none";
        if (val === "Webcam") {
            if (streamOverlay) {
                streamOverlay.checked = false;
                streamOverlay.disabled = true;
            }
            if (streamOverlayControls) streamOverlayControls.style.display = "none";
        } else {
            if (streamOverlay) streamOverlay.disabled = false;
        }
        updateCamVisibility();
    });
}

if (streamOverlay) {
    streamOverlay.addEventListener("change", () => {
        if (streamOverlayControls) {
            streamOverlayControls.style.display = streamOverlay.checked ? "flex" : "none";
        }
        updateCamVisibility();
    });
}

if (streamOvSize && streamOvSizeVal) {
    streamOvSize.addEventListener("input", () => {
        streamOvSizeVal.textContent = `${streamOvSize.value}%`;
    });
}

if (streamOvMargin && streamOvMarginVal) {
    streamOvMargin.addEventListener("input", () => {
        streamOvMarginVal.textContent = `${streamOvMargin.value}px`;
    });
}

if (micVol && micVolVal) {
    micVol.addEventListener("input", () => {
        micVolVal.textContent = `${micVol.value}%`;
    });
}

if (sysVol && sysVolVal) {
    sysVol.addEventListener("input", () => {
        sysVolVal.textContent = `${sysVol.value}%`;
    });
}

if (destKick) {
    destKick.addEventListener("change", () => {
        if (urlKick) urlKick.style.display = destKick.checked ? "block" : "none";
    });
}

if (destTiktok) {
    destTiktok.addEventListener("change", () => {
        if (urlTiktok) urlTiktok.style.display = destTiktok.checked ? "block" : "none";
    });
}

const deleteKeyBtn = document.getElementById("delete-key-btn");

async function updateKeyIndicators() {
    for (const p of ["youtube", "twitch", "kick", "tiktok"]) {
        const el = document.getElementById(`key-status-${p}`);
        if (!el) continue;
        try {
            const has = await invokeTauri("has_stream_key", { platform: p });
            el.hidden = !has;
            el.style.display = has ? "" : "none";
        } catch (_) {
            el.hidden = true;
            el.style.display = "none";
        }
    }
    if (deleteKeyBtn && keyPlatform) {
        const p = (keyPlatform.value || "youtube").trim().toLowerCase();
        try {
            const has = await invokeTauri("has_stream_key", { platform: p });
            deleteKeyBtn.hidden = !has;
            deleteKeyBtn.style.display = has ? "" : "none";
        } catch (_) {
            deleteKeyBtn.hidden = true;
            deleteKeyBtn.style.display = "none";
        }
    }
}

if (keyPlatform) {
    keyPlatform.addEventListener("change", updateKeyIndicators);
}

if (saveKeyBtn) {
    saveKeyBtn.addEventListener("click", async () => {
        const platform = (keyPlatform ? keyPlatform.value : "youtube").trim().toLowerCase();
        const key = streamKeyInput ? streamKeyInput.value.trim() : "";
        if (!key) {
            showToast("Veuillez saisir une clé de flux.");
            return;
        }
        try {
            await invokeTauri("save_stream_key", { platform, key });
            if (streamKeyInput) streamKeyInput.value = "";
            showToast(`Clé enregistrée pour ${platform}`);
            await updateKeyIndicators();
        } catch (e) {
            showToast(`Erreur clé : ${e}`);
        }
    });
}

if (deleteKeyBtn) {
    deleteKeyBtn.addEventListener("click", async () => {
        const platform = (keyPlatform ? keyPlatform.value : "youtube").trim().toLowerCase();
        try {
            await invokeTauri("delete_stream_key", { platform });
            showToast(`Clé supprimée pour ${platform}`);
            await updateKeyIndicators();
        } catch (e) {
            showToast(`Erreur suppression clé : ${e}`);
        }
    });
}

if (startStreamBtn) {
    startStreamBtn.addEventListener("click", async () => {
        let sourceDto;
        const sourceKind = streamSource ? streamSource.value : "AppWindow";
        if (sourceKind === "Screen" && !screenConfirmed) {
            screenConfirmed = true;
            startStreamBtn.textContent = "Confirmer l'écran entier";
            screenConfirmTimeout = setTimeout(() => {
                resetScreenConfirm();
            }, 5000);
            return;
        }
        resetScreenConfirm();

        if (sourceKind === "AppWindow") {
            sourceDto = { kind: "AppWindow" };
        } else if (sourceKind === "Screen") {
            sourceDto = { kind: "Screen" };
        } else if (sourceKind === "Window") {
            const name = streamTarget ? streamTarget.value.trim() : "";
            if (!name) {
                showToast("Nom requis pour la fenêtre à capturer.");
                return;
            }
            sourceDto = { kind: "Window", name };
        } else if (sourceKind === "Webcam") {
            const name = streamCam ? streamCam.value : "";
            if (!name) {
                showToast("Veuillez sélectionner une webcam.");
                return;
            }
            sourceDto = { kind: "Webcam", name };
        }

        const destinations = [];
        if (destYoutube && destYoutube.checked) destinations.push({ platform: "youtube", url: null });
        if (destTwitch && destTwitch.checked) destinations.push({ platform: "twitch", url: null });
        if (destKick && destKick.checked) {
            const u = urlKick ? urlKick.value.trim() : "";
            if (!u) {
                showToast("URL d'ingestion Kick requise.");
                return;
            }
            destinations.push({ platform: "kick", url: u });
        }
        if (destTiktok && destTiktok.checked) {
            const u = urlTiktok ? urlTiktok.value.trim() : "";
            if (!u) {
                showToast("URL d'ingestion TikTok requise.");
                return;
            }
            destinations.push({ platform: "tiktok", url: u });
        }

        if (destinations.length === 0) {
            showToast("Sélectionnez au moins une plateforme de destination.");
            return;
        }

        const overlay = (streamOverlay && streamOverlay.checked && streamCam && streamCam.value)
            ? {
                webcam: streamCam.value,
                corner: streamCorner ? streamCorner.value : "bottom-right",
                size_pct: streamOvSize ? Number(streamOvSize.value) : 25,
                margin_px: streamOvMargin ? Number(streamOvMargin.value) : 20
            }
            : null;

        try {
            await invokeTauri("start_stream", {
                args: {
                    source: sourceDto,
                    audio_device: (streamMic && streamMic.value) ? streamMic.value : null,
                    system_audio_device: (streamSys && streamSys.value) ? streamSys.value : null,
                    mic_volume: micVol ? (Number(micVol.value) / 100) : 1.0,
                    system_volume: sysVol ? (Number(sysVol.value) / 100) : 1.0,
                    overlay,
                    fps: 30,
                    bitrate_kbps: 4500,
                    destinations,
                    record: document.querySelector('#stream-record') ? document.querySelector('#stream-record').checked : false,
                    auto_reconnect: document.querySelector('#stream-reconnect') ? document.querySelector('#stream-reconnect').checked : true
                }
            });
            updateStreamUi(true);
            showToast("Diffusion en direct démarrée !");
            await closeStreamDropdown();
        } catch (e) {
            showToast(`Erreur démarrage : ${e}`);
        }
    });
}

if (stopStreamBtn) {
    stopStreamBtn.addEventListener("click", async () => {
        try {
            await invokeTauri("stop_stream");
            updateStreamUi(false);
            showToast("Diffusion arrêtée.");
        } catch (e) {
            showToast(`Erreur arrêt : ${e}`);
        }
    });
}

// Événement fin de diffusion
(async () => {
    const ev = window.__TAURI__ && window.__TAURI__.event;
    if (ev) {
        await ev.listen("stream-reconnecting", (e) => {
            showToast(`Reconnexion… tentative ${e.payload.attempt} dans ${e.payload.delay_s} s`);
        });
        await ev.listen("stream-warning", (e) => {
            showToast(e.payload);
        });
        await ev.listen("stream-ended", (e) => {
            updateStreamUi(false);
            const code = e.payload;
            showToast(`Diffusion terminée${code !== null && code !== undefined ? ` (code ${code})` : ""}.`);
        });
    }
})();

// F11 bascule le plein écran
window.addEventListener("keydown", async (e) => {
    if (e.key === "F11") {
        e.preventDefault();
        try {
            await invokeTauri("toggle_fullscreen");
        } catch (_) {}
    }
});

renderTabs();
refreshBookmarks();
refreshHistory();
updateKeyIndicators();

// Synchronise la barre d'adresse quand la page navigue d'elle-même
(async () => {
    const ev = window.__TAURI__ && window.__TAURI__.event;
    if (!ev) return;
    await ev.listen("url-changed", async (e) => {
        const { tab, url } = e.payload;
        const t = tabs.find(x => x.id === tab);
        if (!t) return;
        t.url = url;
        t.title = hostOf(url);
        if (tab === activeTabId) {
            urlInput.value = url;
            updateStarButton();
        }
        renderTabs();
        // Aucun enregistrement dans l'historique pour les onglets privés
        if (!t.isPrivate) {
            await invokeTauri("history_add", { url, title: hostOf(url) });
            if (startPage && startPage.style.display !== "none") {
                refreshHistory();
            }
            if (dropdownOpen) {
                await renderDropdownContent(dropdownSearch ? dropdownSearch.value : "");
            }
        }
    });
})();

function updateRamBadge(payload) {
    if (!payload) return;
    const badge = document.getElementById("ram-badge");
    if (!badge) return;

    const total = Math.round(payload.total_mb || 0);
    const appMb = Math.round(payload.app_mb || 0);
    const webviewMb = Math.round(payload.webview_mb || 0);
    const wvCount = payload.webview_count != null ? payload.webview_count : (payload.process_count > 1 ? payload.process_count - 1 : 0);

    badge.title = `App : ${appMb} Mo + WebView2 (${wvCount} processus) : ${webviewMb} Mo = ${total} Mo`;

    if (total !== lastDisplayedRamMb) {
        lastDisplayedRamMb = total;
        badge.textContent = `RAM ≈ ${total} Mo`;

        badge.classList.remove("ram-warn", "ram-alert");
        if (total >= RAM_ALERT_MB) {
            badge.classList.add("ram-alert");
        } else if (total >= RAM_WARN_MB) {
            badge.classList.add("ram-warn");
        }
    }
}

// Surveillance continue de la RAM
(async () => {
    const ev = window.__TAURI__ && window.__TAURI__.event;
    if (ev) {
        await ev.listen("ram-update", (e) => {
            updateRamBadge(e.payload);
        });
    }
    try {
        const initial = await invokeTauri("ram_usage");
        if (initial) updateRamBadge(initial);
    } catch (_) {}
})();

// ---- Assistant IA (Multi-fournisseurs) ----
const aiInvoke = (cmd, args) => invokeTauri(cmd, args);
const $ai = (id) => document.getElementById(id);
const AI = { history: [], busy: false, config: null, activeProviderId: 'anthropic' };
const aiEsc = (s) => String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');

const DEFAULT_AI_PROVIDERS = [
    { id: "anthropic", name: "Anthropic (Claude)", protocol: "anthropic", base_url: "https://api.anthropic.com", max_tokens: 4096, models: { fast: "claude-haiku-5-5", balanced: "claude-sonnet-5-5", strong: "claude-opus-5-5" } },
    { id: "openai", name: "OpenAI", protocol: "openai", base_url: "https://api.openai.com/v1", max_tokens: 4096, models: { fast: "gpt-6-luna", balanced: "gpt-6.1-terra", strong: "gpt-6.1-sol" } },
    { id: "gemini", name: "Google Gemini", protocol: "openai", base_url: "https://generativelanguage.googleapis.com/v1beta/openai", max_tokens: 4096, models: { fast: "gemini-4-hydrogen", balanced: "gemini-4-argon", strong: "gemini-4-argon" } },
    { id: "deepseek", name: "DeepSeek", protocol: "openai", base_url: "https://api.deepseek.com", max_tokens: 4096, models: { fast: "deepseek-v4-flash", balanced: "deepseek-v4-pro", strong: "deepseek-v4-pro" } },
    { id: "openrouter", name: "OpenRouter", protocol: "openai", base_url: "https://openrouter.ai/api/v1", max_tokens: 4096, models: { fast: "", balanced: "", strong: "" } },
    { id: "github", name: "GitHub Models", protocol: "openai", base_url: "https://models.github.ai/inference", max_tokens: 4096, models: { fast: "", balanced: "", strong: "" } },
    { id: "custom", name: "Personnalisé", protocol: "openai", base_url: "http://localhost:11434/v1", max_tokens: 4096, models: { fast: "", balanced: "", strong: "" } }
];

function aiAppend(cls, text) {
    const d = document.createElement('div');
    d.className = 'ai-msg ' + cls;
    d.textContent = text; // jamais innerHTML
    const out = $ai('ai-out');
    if (out) {
        out.append(d);
        out.scrollTop = out.scrollHeight;
    }
    return d;
}

function aiSetBusy(b) {
    AI.busy = b;
    ['ai-sum', 'ai-trad', 'ai-sel', 'ai-send'].forEach((id) => {
        const el = $ai(id);
        if (el) el.disabled = b;
    });
}

function aiPagePrompt(p, question) {
    const text = (p.text || '').replaceAll('</page', '<\\/page');
    const sel = p.selection ? `\n<selection>\n${p.selection.replaceAll('</selection', '<\\/selection')}\n</selection>` : '';
    return `<page url="${aiEsc(p.url)}" title="${aiEsc(p.title)}">\n${text}\n</page>${sel}\n\n${question}`;
}

async function aiRefreshKey() {
    try {
        const pid = AI.activeProviderId || 'anthropic';
        const hasKey = await aiInvoke('ai_has_key', { provider_id: pid });
        const el = $ai('ai-key-state');
        if (el) el.textContent = hasKey ? '✓ enregistrée' : '(absente)';
    } catch (_) {}
}

function aiGetActiveProvider() {
    const providers = (AI.config && Array.isArray(AI.config.providers) && AI.config.providers.length > 0)
        ? AI.config.providers
        : DEFAULT_AI_PROVIDERS;
    return providers.find((p) => p.id === AI.activeProviderId) || providers[0];
}

function aiPopulateProvidersSelect() {
    const providerSelect = $ai('ai-provider');
    if (!providerSelect) return;
    const providers = (AI.config && Array.isArray(AI.config.providers) && AI.config.providers.length > 0)
        ? AI.config.providers
        : DEFAULT_AI_PROVIDERS;

    providerSelect.replaceChildren();
    providers.forEach((prov) => {
        const opt = document.createElement('option');
        opt.value = prov.id;
        opt.textContent = prov.name;
        if (prov.id === AI.activeProviderId) opt.selected = true;
        providerSelect.appendChild(opt);
    });
}

function aiUpdateProviderUi() {
    const p = aiGetActiveProvider();
    if (!p) return;

    // S'assurer que le select provider affiche la bonne valeur
    const providerSelect = $ai('ai-provider');
    if (providerSelect && providerSelect.value !== p.id) {
        providerSelect.value = p.id;
    }

    // URL de base
    const baseUrlInput = $ai('ai-base-url');
    if (baseUrlInput) baseUrlInput.value = p.base_url || '';

    // Avertissement serveur custom
    const warn = $ai('ai-custom-warn');
    if (warn) warn.style.display = p.id === 'custom' ? 'block' : 'none';

    // Limite de tokens
    const maxTokensInput = $ai('ai-max-tokens');
    if (maxTokensInput) {
        maxTokensInput.value = p.max_tokens || 4096;
    }

    // Modèles configurés
    const mFast = (p.models && p.models.fast) || '';
    const mBalanced = (p.models && p.models.balanced) || '';
    const mStrong = (p.models && p.models.strong) || '';

    const emptyModelsPlaceholder = (p.id === 'openrouter' || p.id === 'github' || p.id === 'custom')
        ? 'Clique sur Lister les modèles'
        : '';

    if ($ai('ai-model-fast')) {
        $ai('ai-model-fast').value = mFast;
        $ai('ai-model-fast').placeholder = (!mFast && emptyModelsPlaceholder)
            ? emptyModelsPlaceholder
            : (p.id === 'github' ? 'ex: openai/gpt-4.1' : 'ex: claude-haiku-5-5');
    }
    if ($ai('ai-model-balanced')) {
        $ai('ai-model-balanced').value = mBalanced;
        $ai('ai-model-balanced').placeholder = (!mBalanced && emptyModelsPlaceholder)
            ? emptyModelsPlaceholder
            : (p.id === 'github' ? 'ex: openai/gpt-4.1' : 'ex: claude-sonnet-5-5');
    }
    if ($ai('ai-model-strong')) {
        $ai('ai-model-strong').value = mStrong;
        $ai('ai-model-strong').placeholder = (!mStrong && emptyModelsPlaceholder)
            ? emptyModelsPlaceholder
            : (p.id === 'github' ? 'ex: openai/gpt-4.1' : 'ex: claude-opus-5-5');
    }

    // Sélecteur de niveau (mettre à jour les libellés avec les identifiants)
    const modelSelect = $ai('ai-model');
    if (modelSelect) {
        const optFast = modelSelect.querySelector('option[value="fast"]');
        const optBalanced = modelSelect.querySelector('option[value="balanced"]');
        const optStrong = modelSelect.querySelector('option[value="strong"]');
        if (optFast) optFast.textContent = `Rapide (${mFast || 'non configuré'})`;
        if (optBalanced) optBalanced.textContent = `Équilibré (${mBalanced || 'non configuré'})`;
        if (optStrong) optStrong.textContent = `Puissant (${mStrong || 'non configuré'})`;
    }

    // Consentement par fournisseur
    const consentLabel = $ai('ai-consent-label');
    if (consentLabel) {
        consentLabel.textContent = `J'accepte que le texte de la page soit envoyé à ${p.name} (sur clic uniquement)`;
    }
    const consent = $ai('ai-consent');
    if (consent) {
        consent.checked = localStorage.getItem('lb-ai-consent:' + p.id) === '1';
    }

    // Vider le champ de saisie de clé et rafraîchir l'indicateur
    if ($ai('ai-key')) $ai('ai-key').value = '';
    aiRefreshKey();
}

async function aiLoadConfig() {
    try {
        const cfg = await aiInvoke('ai_get_config');
        if (cfg && Array.isArray(cfg.providers) && cfg.providers.length > 0) {
            AI.config = cfg;
        } else {
            AI.config = { active_provider: 'anthropic', providers: DEFAULT_AI_PROVIDERS };
        }
    } catch (err) {
        showToast("Erreur config IA : " + err);
        console.error("Erreur config IA :", err);
        AI.config = { active_provider: 'anthropic', providers: DEFAULT_AI_PROVIDERS };
    }
    AI.activeProviderId = (AI.config && AI.config.active_provider) || 'anthropic';
    aiPopulateProvidersSelect();
    aiUpdateProviderUi();
}

/** question : texte affiché et envoyé ; page : true = (re)charger la page courante et repartir d'une conversation neuve. */
async function aiAsk(question, { page }) {
    if (AI.busy) return;
    const p = aiGetActiveProvider();
    if (!p) { showToast("Aucun fournisseur IA configuré."); return; }

    const consentKey = 'lb-ai-consent:' + p.id;
    if (localStorage.getItem(consentKey) !== '1') {
        showToast(`Coche d'abord l'accord d'envoi du texte à ${p.name}.`);
        return;
    }
    if (!question || !question.trim()) return;

    aiSetBusy(true);
    let waiting = null;
    let pushed = false;
    try {
        let content = question;
        if (page) {
            const pageData = await aiInvoke('ai_get_page');
            if (question === AI_PROMPTS.selection && !pageData.selection) throw new Error("aucune sélection dans la page");
            AI.history = [];
            content = aiPagePrompt(pageData, question);
        }
        if (AI.history.length >= 12) throw new Error('conversation longue : clique sur « Nouvelle conversation »');
        aiAppend('user', question);
        waiting = aiAppend('assistant', '…');
        AI.history.push({ role: 'user', content });
        pushed = true;
        const modelSelect = $ai('ai-model');
        const tier = modelSelect ? modelSelect.value : 'balanced';
        const reply = await aiInvoke('ai_chat', {
            provider_id: p.id,
            tier,
            messages: AI.history
        });
        waiting.textContent = reply;
        AI.history.push({ role: 'assistant', content: reply });
    } catch (e) {
        if (pushed && AI.history.at(-1)?.role === 'user') AI.history.pop();
        if (waiting) { waiting.className = 'ai-msg error'; waiting.textContent = 'Erreur : ' + e; }
        else showToast('Assistant IA : ' + e);
    } finally {
        aiSetBusy(false);
    }
}

const AI_PROMPTS = {
    summary: 'Résume cette page en 5 à 8 points clés, puis une phrase de conclusion.',
    translate: 'Traduis en français le contenu principal de cette page (ou la sélection si elle existe).',
    selection: 'Explique simplement le passage sélectionné dans la page.',
};

async function initAssistantAI() {
    const panel = $ai('ai-panel');
    const btn = $ai('ai-btn');
    if (!panel || !btn) return;

    // Écouteur de changement sur le select fournisseur
    const providerSelect = $ai('ai-provider');
    if (providerSelect) {
        providerSelect.addEventListener('change', async () => {
            AI.activeProviderId = providerSelect.value;
            if (AI.config) {
                AI.config.active_provider = AI.activeProviderId;
                try {
                    await aiInvoke('ai_set_config', { config: AI.config });
                } catch (err) {
                    showToast("Erreur sauvegarde config : " + err);
                }
            }
            aiUpdateProviderUi();
        });
    }

    // Charger immédiatement la configuration
    await aiLoadConfig();

    btn.addEventListener('click', async () => {
        await setAiPanelOpen(panel.hidden);
    });

    const closeBtn = $ai('ai-close');
    if (closeBtn) closeBtn.addEventListener('click', () => { setAiPanelOpen(false); });

    if (window.__TAURI__ && window.__TAURI__.event) {
        window.__TAURI__.event.listen("ai-close-sidebar", () => {
            setAiPanelOpen(false);
        });
    }

    document.addEventListener("fullscreenchange", () => {
        if (document.fullscreenElement) {
            setAiPanelOpen(false);
        }
    });

    const consent = $ai('ai-consent');
    if (consent) {
        consent.addEventListener('change', (e) => {
            const pid = AI.activeProviderId || 'anthropic';
            localStorage.setItem('lb-ai-consent:' + pid, e.target.checked ? '1' : '0');
        });
    }

    // Réglages du fournisseur : enregistrer
    const saveCfgBtn = $ai('ai-save-cfg');
    if (saveCfgBtn) {
        saveCfgBtn.addEventListener('click', async () => {
            const p = aiGetActiveProvider();
            if (!p) return;
            const baseUrlInput = $ai('ai-base-url');
            if (baseUrlInput) p.base_url = baseUrlInput.value.trim();
            if (!p.models) p.models = {};
            if ($ai('ai-model-fast')) p.models.fast = $ai('ai-model-fast').value.trim();
            if ($ai('ai-model-balanced')) p.models.balanced = $ai('ai-model-balanced').value.trim();
            if ($ai('ai-model-strong')) p.models.strong = $ai('ai-model-strong').value.trim();

            const maxTokensInput = $ai('ai-max-tokens');
            if (maxTokensInput) {
                const val = parseInt(maxTokensInput.value, 10);
                p.max_tokens = isNaN(val) ? 4096 : Math.min(16384, Math.max(1, val));
                maxTokensInput.value = p.max_tokens;
            }

            try {
                await aiInvoke('ai_set_config', { config: AI.config });
                aiUpdateProviderUi();
                showToast('Réglages enregistrés');
            } catch (e) {
                showToast('Erreur réglages : ' + e);
            }
        });
    }

    // Lister les modèles
    const listModelsBtn = $ai('ai-list-models');
    if (listModelsBtn) {
        listModelsBtn.addEventListener('click', async () => {
            const pid = AI.activeProviderId || 'anthropic';
            try {
                showToast('Recherche des modèles…');
                const models = await aiInvoke('ai_list_models', { provider_id: pid });
                const datalist = $ai('ai-models-list');
                const p = aiGetActiveProvider();
                if (datalist && Array.isArray(models)) {
                    datalist.replaceChildren();

                    let sorted = [...models];
                    if (pid === 'openrouter') {
                        const families = ['anthropic/', 'openai/', 'google/', 'deepseek/'];
                        sorted.sort((a, b) => {
                            const aFam = families.some(f => a.startsWith(f));
                            const bFam = families.some(f => b.startsWith(f));
                            if (aFam && !bFam) return -1;
                            if (!aFam && bFam) return 1;
                            return a.localeCompare(b);
                        });

                        // Si les champs sont vides, pré-remplir avec les modèles récents
                        if (p && p.models && (!p.models.fast && !p.models.balanced && !p.models.strong)) {
                            const findOne = (keys) => sorted.find(m => keys.some(k => m.toLowerCase().includes(k)));
                            const f = findOne(['deepseek/deepseek-v4-flash', 'google/gemini-4-hydrogen', 'openai/gpt-6-luna', 'claude-haiku', 'flash', 'mini']) || sorted[0] || '';
                            const b = findOne(['anthropic/claude-sonnet-5-5', 'openai/gpt-6.1-terra', 'deepseek/deepseek-v4-pro', 'google/gemini-4-argon', 'sonnet', 'terra']) || sorted[1] || '';
                            const s = findOne(['anthropic/claude-opus-5-5', 'openai/gpt-6.1-sol', 'deepseek/deepseek-v4-pro', 'anthropic/claude-fable', 'opus', 'sol']) || sorted[2] || '';
                            if (f) p.models.fast = f;
                            if (b) p.models.balanced = b;
                            if (s) p.models.strong = s;
                            if ($ai('ai-model-fast')) $ai('ai-model-fast').value = p.models.fast;
                            if ($ai('ai-model-balanced')) $ai('ai-model-balanced').value = p.models.balanced;
                            if ($ai('ai-model-strong')) $ai('ai-model-strong').value = p.models.strong;
                            try {
                                await aiInvoke('ai_set_config', { config: AI.config });
                                aiUpdateProviderUi();
                            } catch (_) {}
                        }
                    }

                    sorted.forEach((m) => {
                        const opt = document.createElement('option');
                        opt.value = m;
                        datalist.appendChild(opt);
                    });
                }
                showToast(`${models.length} modèles disponibles`);
            } catch (e) {
                showToast('Erreur modèles : ' + e);
            }
        });
    }

    const sumBtn = $ai('ai-sum');
    if (sumBtn) sumBtn.addEventListener('click', () => aiAsk(AI_PROMPTS.summary, { page: true }));
    const tradBtn = $ai('ai-trad');
    if (tradBtn) tradBtn.addEventListener('click', () => aiAsk(AI_PROMPTS.translate, { page: true }));
    const selBtn = $ai('ai-sel');
    if (selBtn) selBtn.addEventListener('click', () => aiAsk(AI_PROMPTS.selection, { page: true }));
    const clearBtn = $ai('ai-clear');
    if (clearBtn) clearBtn.addEventListener('click', () => {
        AI.history = [];
        const out = $ai('ai-out');
        if (out) out.replaceChildren();
    });

    const send = () => {
        const qInput = $ai('ai-q');
        const q = qInput ? qInput.value : '';
        const withPage = $ai('ai-with-page');
        const first = AI.history.length === 0 && (!withPage || withPage.checked);
        if (qInput) qInput.value = '';
        aiAsk(q, { page: first });
    };
    const sendBtn = $ai('ai-send');
    if (sendBtn) sendBtn.addEventListener('click', send);
    const qInput = $ai('ai-q');
    if (qInput) {
        qInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                send();
            }
        });
    }

    const saveKeyBtn = $ai('ai-key-save');
    if (saveKeyBtn) {
        saveKeyBtn.addEventListener('click', async () => {
            const pid = AI.activeProviderId || 'anthropic';
            const kInput = $ai('ai-key');
            try {
                await aiInvoke('ai_save_key', { provider_id: pid, key: kInput ? kInput.value : '' });
                if (kInput) kInput.value = '';
                showToast('Clé enregistrée pour ' + pid);
            } catch (e) {
                showToast('Clé : ' + e);
            }
            aiRefreshKey();
        });
    }
    const checkKeyBtn = $ai('ai-key-check');
    if (checkKeyBtn) {
        checkKeyBtn.addEventListener('click', async () => {
            const pid = AI.activeProviderId || 'anthropic';
            try {
                showToast(await aiInvoke('ai_check_key', { provider_id: pid }));
            } catch (e) {
                showToast('Clé : ' + e);
            }
        });
    }
    const delKeyBtn = $ai('ai-key-del');
    if (delKeyBtn) {
        delKeyBtn.addEventListener('click', async () => {
            const pid = AI.activeProviderId || 'anthropic';
            try {
                await aiInvoke('ai_delete_key', { provider_id: pid });
                showToast('Clé supprimée');
            } catch (e) {
                showToast('Clé : ' + e);
            }
            aiRefreshKey();
        });
    }
}
initAssistantAI();
