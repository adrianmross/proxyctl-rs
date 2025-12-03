#!/bin/bash

set -e

echo "Installing proxyctl-rs..."

# Detect OS
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macos"
else
    echo "Unsupported OS: $OSTYPE"
    exit 1
fi

# Detect architecture
ARCH=$(uname -m)
if [[ "$ARCH" == "x86_64" ]]; then
    ARCH="x86_64"
elif [[ "$ARCH" == "aarch64" ]]; then
    ARCH="aarch64"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

# Download latest release
LATEST_URL=$(curl -s https://api.github.com/repos/adrianmross/proxyctl-rs/releases/latest | grep "browser_download_url.*${OS}-${ARCH}" | cut -d '"' -f 4)

if [[ -z "$LATEST_URL" ]]; then
    echo "Could not find release for $OS-$ARCH"
    exit 1
fi

echo "Downloading from: $LATEST_URL"

# Download and install
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

curl -L -o proxyctl-rs "$LATEST_URL"
chmod +x proxyctl-rs

# Install to /usr/local/bin or ~/bin
if [[ -w /usr/local/bin ]]; then
    sudo mv proxyctl-rs /usr/local/bin/
else
    mkdir -p ~/bin
    mv proxyctl-rs ~/bin/
    export PATH="$HOME/bin:$PATH"
    echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
    echo 'export PATH="$HOME/bin:$PATH"' >> ~/.zshrc
fi

echo "proxyctl-rs installed successfully!"
echo "Run 'proxyctl-rs --help' to get started."