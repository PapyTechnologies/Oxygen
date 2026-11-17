# Changelog

All notable changes to Oxygen Browser are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive bilingual documentation (`README.md` and `README.fr.md`) with system requirements, step-by-step usage, and architecture flows.
- Modern vector visual assets (`assets/logo.svg`, `assets/logo-icon.svg`) and high-resolution social preview banner (`assets/social-preview.png`).
- GitHub community health templates: Issue forms (`bug_report.yml`, `feature_request.yml`, `config.yml`), `pull_request_template.md`, and `CODEOWNERS`.
- GitHub Actions CI workflow (`.github/workflows/ci.yml`) on `windows-latest` running formatting, clippy, and unit tests across the workspace.
- Automated installation and diagnostic script (`scripts/setup_and_build.ps1`) checking Rust, WebView2, and optional FFmpeg runtimes.
- Repository hygiene rules in `.gitignore` covering sub-crate Cargo.lock files, native binaries, and private screen captures.

### Changed
- Standardized documentation commands to match actual project crates (`cargo test -p lightbrowser-core` and `cargo build -p lightbrowser --bin Oxygen --release`).
- Extended `THIRD_PARTY_NOTICES.md` to cover all active workspace dependencies (`tauri-plugin-global-shortcut`, `windows`, `webview2-com`, `keyring-rs`, `cpal`, `ureq`, `serde`).

## [0.1.0] - 2026-11-15

### Added
- Multi-tab web browser powered by Tauri 2 and WebView2 Evergreen architecture.
- Built-in network ad-blocking engine with in-engine URL pattern filtration.
- Integrated AI assistant panel supporting Anthropic, OpenAI, Google Gemini, DeepSeek, OpenRouter, and GitHub Models.
- Windows Credential Manager integration for secure API token storage (`ai-key:<provider>`).
- Live streaming and video capture supervisor using external FFmpeg pipelines with hardware acceleration support.
- Native WASAPI automatic loopback audio capture for game and system sound.
- Dynamic tab memory optimizer with active working-set compaction.
