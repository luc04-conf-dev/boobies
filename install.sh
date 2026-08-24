#!/usr/bin/env bash

set -e

REPO="https://github.com/luc04-conf-dev/boobies.git"
INSTALL_DIR="$HOME/.local/bin"

echo "Installing boobies..."

TEMP_DIR="$(mktemp -d)"

git clone --depth=1 "$REPO" "$TEMP_DIR/boobies"

cd "$TEMP_DIR/boobies"

cargo build --release

mkdir -p "$INSTALL_DIR"

install -Dm755 \
    target/release/boobies \
    "$INSTALL_DIR/boobies"

echo
echo "Boobies installed successfully!"
echo "Run:"
echo "  boobies version"

rm -rf "$TEMP_DIR"