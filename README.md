# proxyctl-rs

[![CI](https://github.com/adrianmross/proxyctl-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/adrianmross/proxyctl-rs/actions/workflows/ci.yml)
[![Release](https://github.com/adrianmross/proxyctl-rs/actions/workflows/release.yml/badge.svg)](https://github.com/adrianmross/proxyctl-rs/actions/workflows/release.yml)

A Rust CLI tool for managing proxy configurations.

## Features

- Enable/disable proxy settings
- Automatic detection of best regional proxy
- SSH configuration management for proxy hosts
- Shell integration for bash, zsh, and other shells
- Environment-based configuration for custom deployments

## Installation

### Pre-built binaries

Download the latest release from [GitHub Releases](https://github.com/adrianmross/proxyctl-rs/releases) for your platform and add it to your PATH.

### One-line install

```bash
curl -fsSL https://raw.githubusercontent.com/adrianmross/proxyctl-rs/main/install.sh | bash
```

### From source

1. Clone the repository:
   ```bash
   git clone https://github.com/adrianmross/proxyctl-rs.git
   cd proxyctl-rs
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Install the binary:
   ```bash
   sudo cp target/release/proxyctl-rs /usr/local/bin/
   ```

## Usage

```bash
# Enable proxy (will auto-detect if no URL provided)
proxyctl-rs on

# Enable proxy with specific URL
proxyctl-rs on --proxy http://proxy.example.com:8080

# Disable proxy
proxyctl-rs off

# Detect best regional proxy
proxyctl-rs detect

# Add SSH proxy hosts (uses ~/.config/proxyctl-rs/hosts.txt by default)
proxyctl-rs ssh add

# Add SSH proxy hosts from custom file
proxyctl-rs ssh add --hosts-file /path/to/custom/hosts.txt

# Remove SSH proxy hosts
proxyctl-rs ssh remove

# Show current proxy status
proxyctl-rs status

# Run diagnostic checks
proxyctl-rs doctor
```

## Shell Integration

The tool automatically integrates with your shell by modifying your shell profile (`.zshenv`, `.bash_profile`, etc.).

You can target a shell specifically:

```bash
# Detects shell automatically, but you can override
export SHELL=/bin/bash
proxyctl-rs on
```

Proxy entries in managed profiles are wrapped with:

```bash
### MANAGED BY PROXYCTL-RS START (DO NOT EDIT)
export http_proxy="..."
...
### MANAGED BY PROXYCTL-RS END (DO NOT EDIT)
```

## Configuration

proxyctl-rs uses a configuration directory at `~/.config/proxyctl-rs/` (or equivalent on Windows/macOS) to store settings.

### Config Files

- **`config.toml`**: Main configuration file in TOML format
- **`hosts.txt`**: List of proxy hosts for SSH configuration

### Example config.toml

```toml
# Default hosts file name (relative to config dir)
default_hosts_file = "hosts"

# Custom no_proxy domains (overrides defaults completely)
# Can be an array or comma-delimited string
no_proxy = ["example.com", "internal.domain"]

# Enable/disable WPAD proxy discovery
enable_wpad_discovery = true

# Custom WPAD URL (optional, defaults to generic WPAD)
wpad_url = "http://wpad.local/wpad.dat"

[proxy_settings]
# Enable/disable specific proxy environment variables
enable_http_proxy = true
enable_https_proxy = true
enable_ftp_proxy = true
enable_no_proxy = true

[shell_integration]
# autodetect the shell from $SHELL
detect_shell = true

# fallback when detection is disabled or missing
default_shell = "bash"

# manage additional shell profiles explicitly
shells = ["bash", "zsh"]

# optional paths to update
profile_paths = ["~/.bash_profile", "~/.zshenv"]
```

### Example hosts.txt

```
# Proxy hosts for SSH configuration
internal.server1
internal.server2
dev.example.com
```

### SSH Config

The tool modifies `~/.ssh/config` to add proxy commands for configured hosts.


## Development

### Dev Container

This project includes a dev container for VS Code. Open in VS Code and use "Dev Containers: Reopen in Container" to get started.

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running

```bash
cargo run -- <args>
```

### Environment Configuration

You can override default values using environment variables for testing the detection features without zero custom configuration. Create a `.env` file in the project root based on the provided `.env.example`:

```bash
# Default domains to exclude from proxy (comma-separated)
DEFAULT_NO_PROXY=localhost,127.0.0.1,.local

# Default WPAD URL for proxy discovery
DEFAULT_WPAD_URL=http://wpad.company.com/wpad.dat
```

## Releasing

This project uses [semantic versioning](https://semver.org/). To create a new release:

### Using the release script

```bash
./scripts/release.sh 1.2.3
```

### Manual process

1. Update the version in `Cargo.toml`
2. Create a git tag: `git tag v1.2.3`
3. Push the tag: `git push origin v1.2.3`

GitHub Actions will automatically:
- Build binaries for multiple platforms (Linux, macOS, Windows)
- Create a GitHub release with the binaries
- Publish to crates.io (requires `CRATES_IO_TOKEN` secret to be set in repository settings)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT
