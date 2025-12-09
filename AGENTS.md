# Repository Guidelines

## Project Structure & Module Organization
- Core CLI entry lives in `src/main.rs` with shared logic split across focused modules: `config.rs` for persistent settings, `proxy.rs` for shell and environment toggles, `detect.rs` for regional auto-detection, and `defaults.rs` for bundled values.
- Library-level helpers sit in `src/lib.rs`, enabling unit tests and future library reuse.
- Integration coverage resides in `tests/integration_test.rs`; add more files under `tests/` for scenario-driven cases.
- Operational assets such as install scripts and release tooling are under `scripts/`, while `default_hosts.example.txt` provides a template for user-managed host lists.

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
- Place cross-module scenarios in `tests/` and name files after the behavior under test (e.g., `ssh_workflow.rs`).
- Ensure new functionality is exercised by both positive and failure cases; aim to cover CLI flags and environment-variable branches.

## Commit & Pull Request Guidelines
- Use concise, imperative conventional commit messages with category prefixes seen in history (`fix:`, `ci:`, `feat:`). Squash noisy WIP commits before pushing.
- Open PRs against `main` with a summary of changes, test evidence (command output or CI link), and any configuration updates highlighted.
- Reference related issues or discussions via `Fixes #<id>` when applicable and include screenshots for UX-facing changes such as terminal output tweaks.
