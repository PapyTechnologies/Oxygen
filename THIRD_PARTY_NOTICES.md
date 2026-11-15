# Third-Party Notices

Oxygen Browser incorporates or interfaces with open-source software and components governed by their respective licenses:

## 1. Rust Dependencies
- **Tauri** (https://github.com/tauri-apps/tauri) - Apache-2.0 or MIT
- **Serde & Serde JSON** (https://github.com/serde-rs/serde) - Apache-2.0 or MIT
- **Ureq** (https://github.com/algesten/ureq) - Apache-2.0 or MIT
- **Keyring-rs** (https://github.com/hwchen/keyring-rs) - Apache-2.0 or MIT
- **CPAL** (https://github.com/RustAudio/cpal) - Apache-2.0
- **Windows-rs** (https://github.com/microsoft/windows-rs) - Apache-2.0 or MIT
- **Webview2-com** (https://github.com/wravery/webview2-com) - Apache-2.0 or MIT

## 2. External Tools & Runtimes
- **FFmpeg** (https://ffmpeg.org)
  Used as an external standalone runtime executable for video capture and live streaming.
  FFmpeg is licensed under the GNU Lesser General Public License (LGPL) version 2.1+ or the GNU General Public License (GPL) version 3.0+ depending on compile options.
  Oxygen does not statically or dynamically link FFmpeg code; it launches it as an external sub-process.
- **Microsoft Edge WebView2 Runtime** (https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
  Redistributable evergreen webview component provided by Microsoft for rendering web contents.

## 3. Trademarks & Brand Notices
All product names, logos, and brands (including YouTube, Twitch, Kick, TikTok, Google, Anthropic, OpenAI, DeepSeek, and Microsoft) are property of their respective owners. Their mention in this repository is strictly for descriptive compatibility purposes.
