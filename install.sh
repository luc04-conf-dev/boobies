#!/usr/bin/env bash
set -euo pipefail

APP_NAME="boobies"
BIN_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/boobies"

REPOSITORY="${BOOBIES_REPOSITORY:-https://luc04-conf-dev.github.io/boobies/examples/repo}"

REPO_OWNER="${BOOBIES_REPO_OWNER:-luc04-conf-dev}"
REPO_NAME="${BOOBIES_REPO_NAME:-boobies}"
REPO_BRANCH="${BOOBIES_REPO_BRANCH:-main}"

printf '%s\n' "Installing ${APP_NAME}..."
printf '%s\n' "Repository: ${REPOSITORY}"

mkdir -p "$BIN_DIR"
mkdir -p "$CONFIG_DIR"

# The public installer should install the current release binary.
if ! command -v gh >/dev/null 2>&1; then
    printf '%s\n' "Error: GitHub CLI (gh) is required by this installer."
    exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

printf '%s\n' "Downloading latest Boobies release..."

gh release download \
    --repo "${REPO_OWNER}/${REPO_NAME}" \
    --pattern "boobies" \
    --dir "$TMP_DIR"

if [ ! -f "${TMP_DIR}/boobies" ]; then
    printf '%s\n' "Error: release binary `boobies` was not found."
    exit 1
fi

install -m 0755 "${TMP_DIR}/boobies" "${BIN_DIR}/boobies"

cat > "${CONFIG_DIR}/config.json" <<EOF
{
  "repository": "${REPOSITORY}",
  "root": "/"
}
EOF

printf '\n'
printf '%s\n' "Boobies installed!"
printf '%s\n' "Binary: ${BIN_DIR}/boobies"
printf '%s\n' "Repository: ${REPOSITORY}"

case ":${PATH}:" in
    *:"${BIN_DIR}":*)
        ;;
    *)
        printf '\n'
        printf '%s\n' "Warning: ${BIN_DIR} is not currently in PATH."
        printf '%s\n' "Add this to your shell configuration:"
        printf '%s\n' "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        ;;
esac

printf '\n'
printf '%s\n' "Next:"
printf '%s\n' "  boobies grow"
printf '%s\n' "  sudo boobies bigger firefox"