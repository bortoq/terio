#!/usr/bin/env bash
# Phase 6: Auto-update script for terio
# Checks GitHub releases and updates if a newer version is available.
set -euo pipefail

CONFIG_DIR="${HOME}/.terio"
INSTALL_DIR="${HOME}/.local/bin"
REPO="bortoq/terio"
CURRENT_VERSION="${1:-$(terio --version 2>/dev/null || echo "0.0.0")}"

echo "=== terio auto-update ==="
echo "Current version: ${CURRENT_VERSION}"

# Fetch latest release from GitHub
LATEST=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || echo "")
if [ -z "${LATEST}" ] || [ "${LATEST}" = "null" ]; then
    echo "Warning: could not fetch latest release from GitHub."
    echo "  Check: https://github.com/${REPO}/releases"
    exit 0
fi

TAG_NAME=$(echo "${LATEST}" | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])" 2>/dev/null || echo "")
RELEASE_VERSION="${TAG_NAME#v}"

if [ -z "${RELEASE_VERSION}" ]; then
    echo "Warning: could not determine latest version."
    exit 0
fi

echo "Latest release:  ${RELEASE_VERSION}"

# Compare versions
if [ "${CURRENT_VERSION}" = "${RELEASE_VERSION}" ]; then
    echo "Already up to date."
    exit 0
fi

echo "Updating ${CURRENT_VERSION} -> ${RELEASE_VERSION}..."

# Detect platform
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64)  ARCH="amd64" ;;
    aarch64) ARCH="arm64"  ;;
esac

# Download binary from release
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${RELEASE_VERSION}/terio-${RELEASE_VERSION}-${ARCH}.tar.gz"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "Downloading: ${DOWNLOAD_URL}"
curl -sL "${DOWNLOAD_URL}" -o "${TMP_DIR}/terio.tar.gz" || {
    echo "Error: download failed."
    exit 1
}

mkdir -p "${INSTALL_DIR}"
tar xzf "${TMP_DIR}/terio.tar.gz" -C "${INSTALL_DIR}" terio
chmod +x "${INSTALL_DIR}/terio"
echo "Installed to: ${INSTALL_DIR}/terio"

# Save update timestamp
mkdir -p "${CONFIG_DIR}"
date -u +"%Y-%m-%dT%H:%M:%SZ" > "${CONFIG_DIR}/last_update"

echo "Done."
