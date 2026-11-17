# Third-Party Notices

Oxygen Browser incorporates, links to, or interfaces with open-source software and components governed by their respective licenses. We acknowledge and thank the authors of these projects.

---

## 1. Rust Workspace Dependencies

| Library / Crate | Repository / Upstream | License |
| :--- | :--- | :--- |
| **tauri** | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | Apache-2.0 OR MIT |
| **tauri-plugin-global-shortcut** | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | Apache-2.0 OR MIT |
| **serde** & **serde_json** | [serde-rs/serde](https://github.com/serde-rs/serde) | Apache-2.0 OR MIT |
| **ureq** | [algesten/ureq](https://github.com/algesten/ureq) | Apache-2.0 OR MIT |
| **keyring** | [hwchen/keyring-rs](https://github.com/hwchen/keyring-rs) | Apache-2.0 OR MIT |
| **cpal** | [RustAudio/cpal](https://github.com/RustAudio/cpal) | Apache-2.0 |
| **windows** | [microsoft/windows-rs](https://github.com/microsoft/windows-rs) | Apache-2.0 OR MIT |
| **webview2-com** | [wravery/webview2-com](https://github.com/wravery/webview2-com) | Apache-2.0 OR MIT |

---

## 2. External Runtimes & Standalone Tools

### Microsoft Edge WebView2 Evergreen Runtime
- **Provider**: Microsoft Corporation
- **License**: Proprietary redistributable runtime.
- **Notice**: WebView2 is pre-installed on Windows 10 (build 19041+) and Windows 11. Oxygen interfaces with WebView2 via Microsoft's official COM APIs.

### FFmpeg (Optional Live Streaming & Encoding Pipeline)
- **Website**: [ffmpeg.org](https://ffmpeg.org)
- **License**: GNU Lesser General Public License (LGPL) version 2.1+ or GNU General Public License (GPL) version 3.0+ depending on build configuration and third-party encoders (e.g. `libx264`).
- **Notice**: Oxygen does **not** statically or dynamically link FFmpeg code into its binary. FFmpeg is executed as an external, optional standalone sub-process managed by Oxygen's streaming supervisor. Users and packagers providing FFmpeg binaries are responsible for complying with the corresponding LGPL or GPL terms.

---

## 3. Network Filter Lists & Data Rules

- **Adblock Rules**: URL and domain matching heuristics within the shield engine are derived from and inspired by community filter lists (EasyList, Peter Lowe's Blocklist), distributed under MPL-2.0 and GPL-3.0 licenses.

---

## 4. Trademarks & Brand Notices

All trademarks, service marks, trade names, and logos referenced in this project (including Microsoft, Windows, Edge, Google, Gemini, Anthropic, Claude, OpenAI, DeepSeek, GitHub, Twitch, YouTube, Kick, and TikTok) are the property of their respective owners. Their mention in this repository is strictly descriptive for architectural and compatibility purposes and does not imply endorsement.
