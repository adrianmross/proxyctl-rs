#!/bin/bash

# Setup shell integration for proxyctl-rs

set -e

echo "Setting up shell integration for proxyctl-rs..."

# Detect shell
SHELL_NAME=$(basename "$SHELL")

if [[ "$SHELL_NAME" == "bash" ]]; then
    PROFILE_FILE="$HOME/.bashrc"
elif [[ "$SHELL_NAME" == "zsh" ]]; then
    PROFILE_FILE="$HOME/.zshrc"
else
    echo "Unsupported shell: $SHELL_NAME"
    echo "Please manually add proxyctl-rs functions to your shell profile."
    exit 1
fi

# Add functions to profile
cat >> "$PROFILE_FILE" << 'EOF'

# proxyctl-rs shell integration
proxyctl() {
    proxyctl-rs "$@"
}

# Alias for quick proxy toggle
alias proxyon='proxyctl-rs on'
alias proxyoff='proxyctl-rs off'
alias proxystatus='proxyctl-rs status'

EOF

echo "Shell integration added to $PROFILE_FILE"
echo "Please restart your shell or run 'source $PROFILE_FILE' to activate."