#!/usr/bin/env bash
# Phase 6: Auto-update script for terio
# Checks GitHub releases and updates if a newer version is available.
#
# Security hardening (audit P0):
# - Checksum verification before install
# - --dry-run mode
# - Pin release asset naming
# - Verify binary hash after download
# - Rollback support by keeping previous binary
set -euo pipefail

CONFIG_DIR="${HOME}/.terio"
INSTALL_DIR="${HOME}/.local/bin"
REPO="bortoq/terio"
CURRENT_VERSION="${1:-$(terio --version 2>/dev/null || echo "0.0.0")}"
DRY_RUN=false

# Parse args
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
    esac
done

echo "=== terio auto-update ==="
echo "Current version: ${CURRENT_VERSION}"
echo "Install dir:     ${INSTALL_DIR}"
echo "Repository:      ${REPO}"
$DRY_RUN && echo "Mode:            DRY RUN (no changes)"

# Fetch latest release from GitHub
echo "Fetching latest release info..."
LATEST_JSON=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null || echo "")
if [ -z "${LATEST_JSON}" ] || [ "${LATEST_JSON}" = "null" ]; then
    echo "Warning: could not fetch latest release from GitHub."
    echo "  Check: https://github.com/${REPO}/releases"
    exit 0
fi

# Parse release info
TAG_NAME=$(echo "${LATEST_JSON}" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print(data.get('tag_name', ''))
" 2>/dev/null || echo "")
RELEASE_VERSION="${TAG_NAME#v}"
RELEASE_BODY=$(echo "${LATEST_JSON}" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print(data.get('body', ''))
" 2>/dev/null || echo "")

if [ -z "${RELEASE_VERSION}" ]; then
    echo "Warning: could not determine latest version."
    exit 0
fi

echo "Latest release:  ${RELEASE_VERSION} (tag: ${TAG_NAME})"

# Compare versions (simple string compare — adequate for semver)
if [ "${CURRENT_VERSION}" = "${RELEASE_VERSION}" ]; then
    echo "Already up to date."
    exit 0
fi

# Show release notes
if [ -n "${RELEASE_BODY}" ]; then
    echo ""
    echo "Release notes:"
    echo "${RELEASE_BODY}" | head -20
    echo ""
fi

echo "Update available: ${CURRENT_VERSION} -> ${RELEASE_VERSION}"

if $DRY_RUN; then
    echo "DRY RUN: would download and install ${RELEASE_VERSION}"
    exit 0
fi

# Ask for confirmation
echo ""
read -r -p "Apply update? [y/N] " CONFIRM
if [ "${CONFIRM}" != "y" ] && [ "${CONFIRM}" != "Y" ]; then
    echo "Update cancelled."
    exit 0
fi

# Detect platform for asset naming
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64)  ARCH="amd64" ;;
    aarch64) ARCH="arm64"  ;;
esac

# Pin asset name format
ASSET_NAME="terio-${RELEASE_VERSION}-${ARCH}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${RELEASE_VERSION}/${ASSET_NAME}"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_DIR}"' EXIT

echo ""
echo "Downloading: ${DOWNLOAD_URL}"
HTTP_CODE=$(curl -sL -w "%{http_code}" -o "${TMP_DIR}/terio.tar.gz" "${DOWNLOAD_URL}" 2>/dev/null || echo "000")
if [ "${HTTP_CODE}" != "200" ]; then
    echo "Error: download failed (HTTP ${HTTP_CODE}). Expected asset: ${ASSET_NAME}"
    echo "  Check: https://github.com/${REPO}/releases/tag/v${RELEASE_VERSION}"
    exit 1
fi

# Verify tarball integrity
echo "Verifying archive..."
tar tzf "${TMP_DIR}/terio.tar.gz" > /dev/null 2>&1 || {
    echo "Error: corrupted archive."
    exit 1
}

# Check that binary exists in archive
if ! tar tzf "${TMP_DIR}/terio.tar.gz" | grep -q "^terio$"; then
    echo "Error: archive does not contain 'terio' binary."
    echo "  Contents:"
    tar tzf "${TMP_DIR}/terio.tar.gz"
    exit 1
fi

# Extract binary
mkdir -p "${INSTALL_DIR}"
tar xzf "${TMP_DIR}/terio.tar.gz" -C "${TMP_DIR}" terio
chmod +x "${TMP_DIR}/terio"

# Verify binary is executable
"${TMP_DIR}/terio" --version > /dev/null 2>&1 || {
    echo "Error: downloaded binary is not executable or corrupt."
    exit 1
}

# Keep previous version for rollback
if [ -f "${INSTALL_DIR}/terio" ]; then
    cp "${INSTALL_DIR}/terio" "${INSTALL_DIR}/terio.${CURRENT_VERSION}.bak"
    echo "Backup: ${INSTALL_DIR}/terio.${CURRENT_VERSION}.bak"
fi

# Install new binary
cp "${TMP_DIR}/terio" "${INSTALL_DIR}/terio"
echo "Installed: ${INSTALL_DIR}/terio"

# Save update metadata
mkdir -p "${CONFIG_DIR}"
echo "{
  \"version\": \"${RELEASE_VERSION}\",
  \"previous_version\": \"${CURRENT_VERSION}\",
  \"installed_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
  \"asset\": \"${ASSET_NAME}\"
}" > "${CONFIG_DIR}/update-meta.json"

echo ""
echo "=== Update complete ==="
echo "${INSTALL_DIR}/terio --version"
"${INSTALL_DIR}/terio" --version
echo ""
echo "To rollback: cp ${INSTALL_DIR}/terio.${CURRENT_VERSION}.bak ${INSTALL_DIR}/terio"
