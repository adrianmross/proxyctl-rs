# Repository Guidelines

## Project Structure & Module Organization
- Core CLI entry lives in `src/main.rs` with shared logic split across focused modules: `config.rs` for persistent settings and SSH host management, `proxy.rs` for shell and environment toggles, `detect.rs` for regional auto-detection, and `defaults.rs` for bundled values.
- The new `src/db.rs` module encapsulates Turso-backed persistence for proxy environment state. Access it through the async helpers re-exported from `src/lib.rs`.
- Library-level helpers sit in `src/lib.rs`, enabling unit tests and future library reuse. Re-export new modules here when adding functionality so tests can `use proxyctl_rs::<module>`.
- Integration coverage resides under `tests/`; add more files under `tests/` for scenario-driven cases. Existing suites cover proxy toggling, database persistence, and SSH config flows—mirror their fixture patterns when extending behavior.
- Operational assets such as install scripts and release tooling are under `scripts/`, while `default_hosts.example.txt` provides a template for user-managed host lists. Shell integration helpers live alongside release tooling in this directory.

## Build, Test, and Development Commands
- `cargo build` compiles the workspace in debug mode; prefer `cargo build --release` before benchmarking or packaging.
- `cargo run -- <args>` executes the CLI with local changes.
- `cargo fmt` and `cargo clippy --all-targets --all-features` enforce formatting and lint correctness; run both before submitting.
- `cargo test` runs unit and integration suites; use `cargo test -- --nocapture` when debugging assertions.
- `./scripts/release.sh <version>` tags a release and prepares artifacts; keep it coordinated with `Cargo.toml` version bumps.

## Coding Style & Naming Conventions
- Follow Rust 2021 edition defaults: four-space indentation, snake_case for modules/files, lower_snake_case for functions and variables, UpperCamelCase for types.
- Keep functions short and compose through helpers in `src/lib.rs` where practical.
- Favor explicit `Result` types and structured error propagation via `anyhow`/`thiserror` patterns already present.

## Testing Guidelines
- Co-locate focused unit tests within each module under `#[cfg(test)]` blocks named `<module>_tests`.
- Place cross-module scenarios in `tests/` and name files after the behavior under test (e.g., `ssh_config.rs`, `db.rs`).
- Async behavior (database helpers, proxy commands) should use `#[tokio::test]` with the multi-thread runtime when interacting with Turso.
- SSH config tests rely on the `OnceLock`-guarded fixtures in `tests/ssh_config.rs`; reuse those helpers to avoid interfering with concurrent runs.
- Ensure new functionality is exercised by both positive and failure cases; aim to cover CLI flags, environment-variable branches, and persistence edge cases.

## Commit & Pull Request Guidelines
- Use concise, imperative conventional commit messages with category prefixes seen in history (`fix:`, `ci:`, `feat:`). Squash noisy WIP commits before pushing.
- Open PRs against `main` with a summary of changes, test evidence (command output or CI link), and any configuration updates highlighted.
- Reference related issues or discussions via `Fixes #<id>` when applicable and include screenshots for UX-facing changes such as terminal output tweaks.
