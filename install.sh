#!/usr/bin/env bash
set -e

BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/boobies"

mkdir -p "$BIN_DIR"
mkdir -p "$CONFIG_DIR"

echo "Installing boobies..."

# aqui vem o download/cópia do binário
# ...

cat > "$CONFIG_DIR/config.json" << EOF
{
  "repository": "https://luc04-conf-dev.github.io/boobies/examples/repo",
  "root": "$HOME/.local/share/boobies/root"
}
EOF

echo "Boobies installed!"