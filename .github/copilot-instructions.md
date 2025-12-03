# Copilot Instructions for proxyctl-rs

## Project Overview

proxyctl-rs is a Rust command-line tool for managing proxy configurations. It provides functionality to enable/disable proxy settings, automatically detect the best regional proxy, manage SSH configurations for proxy hosts, and integrate with various shells.

## Project Structure

- `src/main.rs`: Main CLI entry point using clap for command-line argument parsing
- `src/proxy.rs`: Core proxy configuration logic (enable/disable proxy settings)
- `src/config.rs`: Configuration management and XDG config directory handling
- `src/detect.rs`: Proxy detection and regional server selection
- `Cargo.toml`: Rust project dependencies and metadata
- `install.sh`: Installation script for easy deployment
- `scripts/setup_shell_integration.sh`: Shell integration setup
- `tests/`: Integration tests
- `default_hosts.example.txt`: Example proxy hosts configuration

## Development Setup

### Prerequisites
- Rust toolchain (install via rustup: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Git

## Development Setup

### Prerequisites
- Rust toolchain (install via rustup: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Git

### Setting up the Development Environment
1. Clone the repository:
   ```bash
   git clone https://github.com/adrianmross/proxyctl-rs.git
   cd proxyctl-rs
   ```

2. Install dependencies and build:
   ```bash
   cargo build
   ```

3. Run tests to ensure everything works:
   ```bash
   cargo test
   ```

4. (Optional) Install development tools:
   ```bash
   rustup component add clippy rustfmt
   ```

### Configuration
The tool uses XDG Base Directory specification for configuration:
- Config directory: `~/.config/proxyctl-rs/` (Linux), `~/Library/Application Support/proxyctl-rs/` (macOS), or equivalent on Windows
- Main config: `config.toml` (TOML format)
- Hosts file: `hosts.txt` (configurable via config)

Example config.toml:
```toml
default_hosts_file = "hosts"
# Custom no_proxy domains (overrides defaults completely)
# Can be an array or space-delimited string
no_proxy = ["example.com", "internal.domain"]
# Enable/disable WPAD proxy discovery
enable_wpad_discovery = true
# Custom WPAD URL (optional, defaults to Oracle's WPAD)
wpad_url = "http://wpad-amer.oraclecorp.com/wpad.dat"
[proxy_settings]
enable_http_proxy = true
enable_https_proxy = true
enable_ftp_proxy = true
enable_no_proxy = true
```

### Development Workflow
- Use `cargo check` for quick compilation checks
- Format code with `cargo fmt`
- Lint with `cargo clippy`
- Run tests with `cargo test`
- Build release with `cargo build --release`

### Pre-commit Hooks
Set up pre-commit hooks to enforce code quality:
```bash
# Install pre-commit if not already installed
pip install pre-commit
pre-commit install
```

## Conventional Commits

This project uses conventional commits for structured commit messages. Follow the format: `type[optional scope]: description`

See https://conventionalcommits.org/ for details.

## Development Guidelines

- Use `anyhow` for error handling throughout the codebase
- Follow Rust naming conventions and best practices
- Add unit tests for new functionality
- Update README.md when adding new commands or features
- Use clap for CLI argument parsing
- Handle errors gracefully and provide meaningful error messages
- Support cross-platform compatibility where possible

## Dependencies

Key crates used:
- `clap`: Command-line argument parsing
- `anyhow`: Error handling
- `tokio`: Async runtime (for main function)
- Other standard Rust crates for HTTP, JSON, etc.

## Contributing

When making changes:
1. Ensure code compiles with `cargo check`
2. Run tests with `cargo test`
3. Update documentation as needed
4. Follow the existing code style and patterns
5. Use conventional commit messages