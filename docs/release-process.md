# Release Process

> **Status:** Baseline / Manual
> Audit: P1 — missing signed releases, CI pipeline

## Current Process

1. **Tag** — `git tag v0.1.0 && git push --tags`
2. **Build** — `cargo build --release && scripts/package.sh v0.1.0`
3. **Release** — manual upload to GitHub Releases with tarball + .deb + .rpm
4. **Update** — users run `scripts/update.sh` which fetches latest GitHub Release

## Security Boundaries

- Releases are NOT signed (no GPG, no checksum file)
- Update script verifies tarball integrity but NOT provenance
- Package scripts generate .deb/.rpm without dependency metadata

## Future Hardening

- [ ] GPG-signed releases with checksum file (`SHA256SUMS.asc`)
- [ ] GitHub Actions CI that builds, signs, and uploads
- [ ] Package dependencies for .deb (gtk3, webkit2gtk, etc.)
- [ ] `.desktop` file + icon for desktop integration
- [ ] Reproducible builds verification
- [ ] Update script verifies GPG signature before install

## Creating a Release

```bash
# 1. Ensure clean state
cargo test
cargo clippy -- -D warnings
cargo fmt --check

# 2. Bump version in Cargo.toml
#    (follow semver)

# 3. Tag and push
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0

# 4. Build packages
cargo build --release
./scripts/package.sh v0.2.0

# 5. Upload dist/*.tar.gz, dist/*.deb, dist/*.rpm to GitHub Releases
#    (future: automated via CI)
```

## Package Dependencies (for .deb)

When .deb dependencies are added, include:

```
Depends: libgtk-3-0, libwebkit2gtk-4.1-0, libc6
```

## Upgrade Path

- Current: manual `scripts/update.sh`
- Future: `terio upgrade` command with signature verification
