#!/usr/bin/env bash
# Phase 6: Desktop packaging script
# Builds .deb, .rpm, and tarball packages for terio
set -euo pipefail

VERSION="${1:-$(git describe --tags --always 2>/dev/null || echo "0.1.0")}"
OUTDIR="${2:-dist}"
PROFILE="${3:-release}"

echo "=== terio package build v${VERSION} ==="
mkdir -p "${OUTDIR}"

# Build binary
echo "Building terio (${PROFILE})..."
cargo build --profile "${PROFILE}" --features desktop

BINARY="target/${PROFILE}/terio"
if [ ! -f "${BINARY}" ]; then
    echo "Error: binary not found at ${BINARY}"
    exit 1
fi

# Detect architecture
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64)  DEB_ARCH="amd64"; RPM_ARCH="x86_64" ;;
    aarch64) DEB_ARCH="arm64";  RPM_ARCH="aarch64" ;;
    *)       DEB_ARCH="${ARCH}"; RPM_ARCH="${ARCH}" ;;
esac

# ---------------------------------------------------------------------------
# 1. Generic tarball
# ---------------------------------------------------------------------------
echo "Creating tarball..."
TARBALL="${OUTDIR}/terio-${VERSION}-${ARCH}.tar.gz"
tar czf "${TARBALL}" -C "$(dirname "${BINARY}")" "$(basename "${BINARY}")"
echo "  -> ${TARBALL}"

# ---------------------------------------------------------------------------
# 2. .deb package
# ---------------------------------------------------------------------------
if command -v dpkg-deb &>/dev/null; then
    echo "Creating .deb package..."
    DEB_DIR="${OUTDIR}/deb-work"
    mkdir -p "${DEB_DIR}/DEBIAN"
    mkdir -p "${DEB_DIR}/usr/bin"
    mkdir -p "${DEB_DIR}/usr/share/doc/terio"
    mkdir -p "${DEB_DIR}/usr/share/bash-completion/completions"
    mkdir -p "${DEB_DIR}/usr/share/zsh/vendor-completions"

    cp "${BINARY}" "${DEB_DIR}/usr/bin/terio"
    cp README.md "${DEB_DIR}/usr/share/doc/terio/" 2>/dev/null || true

    # Generate man page stub
    cat > "${DEB_DIR}/usr/share/doc/terio/README" << 'DESC'
terio — интегратор интерфейсов: терминал с LLM, кешем скриптов и песочницей.
DESC

    cat > "${DEB_DIR}/DEBIAN/control" << CONTROL
Package: terio
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: terio team <terio@bortoq.com>
Description: Terminal LLM integrator with script cache and sandbox
 terio is a terminal-based interface integrator that uses LLM
 under the hood, script cache and sandbox, and returns output
 as windows — from text to video.
CONTROL

    # Completion stubs
    ${BINARY} help 2>/dev/null | grep -q "COMMAND" && {
        ${BINARY} completion bash > "${DEB_DIR}/usr/share/bash-completion/completions/terio" 2>/dev/null || true
        ${BINARY} completion zsh > "${DEB_DIR}/usr/share/zsh/vendor-completions/_terio" 2>/dev/null || true
    }

    dpkg-deb --build "${DEB_DIR}" "${OUTDIR}/terio_${VERSION}_${DEB_ARCH}.deb"
    rm -rf "${DEB_DIR}"
    echo "  -> ${OUTDIR}/terio_${VERSION}_${DEB_ARCH}.deb"
else
    echo "  (dpkg-deb not found, skipping .deb)"
fi

# ---------------------------------------------------------------------------
# 3. .rpm package
# ---------------------------------------------------------------------------
if command -v rpmbuild &>/dev/null; then
    echo "Creating .rpm package..."
    RPM_DIR="${OUTDIR}/rpm-work"
    mkdir -p "${RPM_DIR}/BUILD" "${RPM_DIR}/RPMS" "${RPM_DIR}/SOURCES" "${RPM_DIR}/SPECS"

    cat > "${RPM_DIR}/SPECS/terio.spec" << SPEC
Name: terio
Version: ${VERSION}
Release: 1%{?dist}
Summary: Terminal LLM integrator with script cache and sandbox
License: Apache-2.0
URL: https://github.com/bortoq/terio
Source0: %{name}-%{version}.tar.gz

%description
terio is a terminal-based interface integrator that uses LLM
under the hood, script cache and sandbox, and returns output
as windows — from text to video.

%install
mkdir -p %{buildroot}%{_bindir}
cp %{_sourcedir}/terio %{buildroot}%{_bindir}/terio

%files
%{_bindir}/terio

%changelog
* $(date "+%a %b %d %Y")  terio team <terio@bortoq.com> - ${VERSION}
- Initial package
SPEC

    cp "${BINARY}" "${RPM_DIR}/SOURCES/"
    rpmbuild --define "_topdir ${RPM_DIR}" -bb "${RPM_DIR}/SPECS/terio.spec"
    find "${RPM_DIR}/RPMS" -name "*.rpm" -exec cp {} "${OUTDIR}/" \;
    rm -rf "${RPM_DIR}"
    echo "  -> ${OUTDIR}/terio-${VERSION}-1.${RPM_ARCH}.rpm"
else
    echo "  (rpmbuild not found, skipping .rpm)"
fi

echo "=== Done. Packages in ${OUTDIR}/ ==="
ls -la "${OUTDIR}/"
