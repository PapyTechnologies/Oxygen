# Contributing to Oxygen

Thank you for your interest in contributing to Oxygen Browser!

## Development Guidelines

1. **Prerequisites**:
   - Rust (1.80+ recommended) with `cargo`
   - Node.js (20+ LTS recommended)
   - Microsoft Edge WebView2 Evergreen Runtime (pre-installed on Windows 10/11)

2. **Commit Conventions**:
   Follow Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `ci:`).

3. **Validation**:
   Before submitting a Pull Request, ensure that:
   - `cargo check` compiles without warnings.
   - `cargo test -p lightbrowser-core` passes all unit tests.
   - No hardcoded secrets, personal file paths, or private credentials are included.

4. **Pull Requests**:
   Target the `main` branch. Provide clear context and testing steps in your PR description.
