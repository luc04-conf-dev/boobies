#!/usr/bin/env bash

set -e

REPO="https://raw.githubusercontent.com/luc04-conf-dev/boobies/main"
INSTALL_DIR="$HOME/.local/bin"

echo "Installing boobies..."

mkdir -p "$INSTALL_DIR"

curl -fsSL "$REPO/target/release/boobies" \
    -o "$INSTALL_DIR/boobies"

chmod +x "$INSTALL_DIR/boobies"

echo "Boobies installed at $INSTALL_DIR/boobies"
echo "Run: boobies version"