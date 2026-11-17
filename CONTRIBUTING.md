# Contributing to Oxygen

Thank you for your interest in contributing to Oxygen Browser! We welcome contributions that preserve Oxygen's core mission: speed, resource efficiency, privacy, and sovereignty.

---

## 1. Prerequisites

Before setting up your local development environment on Windows:
- **Rust Toolchain**: Rust 1.80+ (Stable) installed via [rustup.rs](https://rustup.rs/) (edition 2021/2024).
- **Microsoft Edge WebView2**: Pre-installed on Windows 10 (19041+) and Windows 11.
- **Node.js**: Node 20+ LTS (for frontend asset tooling if modifying web views).
- **FFmpeg (Optional)**: Required only if testing live RTMP streaming or local video recording pipelines.

---

## 2. Development Workflow & Commands

All development and test commands reflect the multi-crate workspace structure (`lightbrowser-core` and `lightbrowser`):

```powershell
# Check workspace compilation
cargo check --workspace

# Run automated unit tests on the core engine
cargo test -p lightbrowser-core

# Verify formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --workspace --all-targets -- -D warnings

# Build the release binary
cargo build -p lightbrowser --bin Oxygen --release
```

---

## 3. Commit Discipline & Guidelines

We strictly follow [Conventional Commits](https://www.conventionalcommits.org/):
- `feat:` New user-facing or architectural capability.
- `fix:` Bug fix or crash resolution.
- `docs:` Documentation, README, or visual assets updates.
- `ci:` GitHub Actions, workflows, or build automation scripts.
- `chore:` Dependency maintenance, `.gitignore`, or configuration updates.

### Safety & Privacy Rules
- **No Hardcoded Credentials**: Never commit API keys, tokens, or personal paths (`E:\...`, `C:\Users\...`).
- **No Generated Binaries**: Never commit `.exe`, `.dll`, `.pdb`, `.zip`, or temporary screen captures.
- **Minimal Code Touches**: Keep PRs focused on the specific task. Avoid unsolicited large-scale refactors.

---

## 4. Submitting a Pull Request

1. Fork the repository and create your feature branch from `main` (e.g. `feat/my-feature` or `fix/issue-description`).
2. Run `cargo fmt`, `cargo clippy`, and `cargo test -p lightbrowser-core`.
3. Open a Pull Request targeting `main` using the provided [Pull Request Template](.github/pull_request_template.md).
4. Our CI pipeline will automatically validate your code formatting, clippy rules, and test suites.
