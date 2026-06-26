# Registry Security Model

> **Status:** Baseline / Experimental (Phase 6)
> Audit: P0 — remote code execution surface

## Current Protection Layer

Since audit `56013c6`:

1. **SHA-256 required** — installs without hash require `--allow-unsigned`
2. **Confirmation prompt** — before install, shows script metadata (description, author, risk, capabilities) and asks `[y/N]`
3. **Max size limit** — 1MB enforced via Content-Length header and actual content size
4. **Overwrite protection** — will not overwrite existing files without explicit removal
5. **Provenance storage** — after install, writes `.provenance/<id>.json` with author, version, hash, timestamp
6. **Script validation** — basic non-empty check after download

## Remaining Gaps (not yet implemented)

| Gap | Priority | Plan |
|-----|----------|------|
| Registry index signature | P0 | GPG-signed index.json; verify before use |
| Code signing | P1 | Publishers sign scripts; terio verifies against known keys |
| Permissions/capabilities model | P1 | Enforce declared capabilities at runtime |
| Sandbox for registry scripts | P1 | Default untrusted; require first-run audit |
| `terio registry verify <id>` | P2 | Verify hash + signature after install |
| Auto-update for registry index | P2 | Cache index, check delta, detect tampering |
| Revocation | P2 | Publish revoked script IDs list |

## Recommendations for Users

- Only install from trusted publishers
- Review capabilities before confirming install
- Use `terio registry inspect <id>` before install
- Run registry scripts in sandbox mode (once implemented)
- Do not use `--allow-unsigned` in production

## Design Principles

- **Defense in depth**: hash + confirmation + size + provenance
- **User consent**: no silent remote code execution
- **Provenance**: every installed script has traceable metadata
- **Progressive hardening**: baseline today, signatures + sandbox tomorrow

## See Also

- `docs/release-process.md` — how signed releases are published
- `src/registry.rs` — implementation
- `ROADMAP.md` — Phase 6 future items
