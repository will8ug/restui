# Design: Publish restui to the npm public registry

**Date:** 2026-08-27
**Status:** Approved (pending spec review)
**Approach:** A — platform packages + `optionalDependencies`, published via manually-triggered GitHub Actions

## Goal

Users can install restui from the npm public registry:

```bash
npm install -g restui      # or: npx restui file.http
```

npm delivers the correct prebuilt Rust binary for the user's platform. No postinstall
scripts. No build-from-source. `cargo install --path .` remains a supported install path.

## Locked decisions

| Decision | Choice |
|---|---|
| Distribution model | Main package + 3 platform packages (`optionalDependencies`, exact-pinned) |
| npm names | `restui`, `restui-darwin-arm64`, `restui-darwin-x64`, `restui-linux-x64-gnu` (all verified unclaimed 2026-08-27) |
| Platforms | macOS arm64, macOS x64, Linux x64 glibc |
| glibc floor | 2.17 (via `cargo-zigbuild` target suffix, run inside the CI job) |
| Release trigger | GitHub Actions `workflow_dispatch` (manual "Run workflow" click) |
| Version source of truth | `Cargo.toml` `[package] version` — stamped into all npm packages at publish time |
| TLS backend | rustls with `rustls-tls-native-roots` + `rustls-tls-webpki-roots` (see §3) |
| Local publishing | Not supported — verification targets only (§7) |

## 1. Package architecture

Four packages, all published from this repo:

| Package | Contents | `os` / `cpu` |
|---|---|---|
| `restui` | `bin/restui.js` shim, README, LICENSE | — |
| `restui-darwin-arm64` | release binary | `darwin` / `arm64` |
| `restui-darwin-x64` | release binary | `darwin` / `x64` |
| `restui-linux-x64-gnu` | release binary | `linux` / `x64` |

- Main package `package.json`: `"bin": { "restui": "bin/restui.js" }`, `files: ["bin"]`,
  `optionalDependencies` listing all three platform packages at the **same exact version,
  no `^` range** (lockstep guarantee; npm's os/cpu matching installs only the user's match
  and silently skips the rest).
- Platform package `package.json`: `os`/`cpu` fields, `files: ["bin"]`, binary at
  `bin/restui`, **no `exports` field** (so the shim's deep `require.resolve` works), no
  dependencies.

### Bin shim (`npm/restui/bin/restui.js`)

Plain CommonJS, zero dependencies, `engines: { node: ">=14" }`:

1. Map `process.platform` + `process.arch` → platform package name.
2. Linux only: detect musl (absence of `glibcVersionRuntime` in
   `process.report.getReport().header`) → friendly error pointing to `cargo install`
   (we ship glibc binaries only).
3. Unsupported platform (e.g. Windows, linux-arm64) → error naming the platform,
   pointing to the GitHub repo and `cargo install`.
4. `require.resolve("<platform-pkg>/bin/restui")`; failure → error explaining the
   platform package is missing (e.g. installed with `--omit=optional`) with install fix.
5. `spawn(binary, process.argv.slice(2), { stdio: "inherit" })` — the TUI must receive
   the real TTY; forward exit code and signals to the parent.

## 2. Repository layout

Templates are committed; build artifacts never are.

```
npm/
  scripts/stage.mjs                       # Node ≥18: stamps versions, stages packages
  restui/package.json                     # main template (version 0.0.0 placeholder)
  restui/bin/restui.js                    # the shim
  restui/README.md                        # user-facing: install, usage, troubleshooting
  restui/LICENSE                          # copy of repo LICENSE
  restui-darwin-arm64/package.json
  restui-darwin-x64/package.json
  restui-linux-x64-gnu/package.json
  restui-darwin-arm64/README.md           # one-liner: "Binary package for restui (macOS arm64)"
  restui-darwin-x64/README.md
  restui-linux-x64-gnu/README.md
```

Staging destination: `target/npm/<pkg>/` (inside `/target/`, already gitignored).

### `.gitignore` fix (required)

The repo's root `.gitignore` contains a bare `package.json` pattern (line 59), which
would ignore the npm templates. Add a negation after it:

```gitignore
!npm/**/package.json
```

Works because no parent directory of those files is itself ignored.

## 3. TLS backend change (only Rust source change)

```toml
# Cargo.toml — before
reqwest = { version = "0.12", features = ["blocking", "json"] }

# after
reqwest = { version = "0.12", default-features = false, features = [
  "blocking", "json", "http2", "charset", "system-proxy",
  "rustls-tls-native-roots", "rustls-tls-webpki-roots",
] }
```

The `http2`, `charset`, and `system-proxy` features restore the reqwest defaults
that `default-features = false` would otherwise disable (only `default-tls` is
intentionally dropped); all three are cross-compile-safe.

Why: default features pull `native-tls` → `openssl-sys` on Linux, the worst-case
cross-compile story. Rustls is statically linked and builds cleanly everywhere.

Behavior preservation — the scenario "API serves a certificate chained to a custom CA
that the machine already trusts":

| Backend | Roots used | Custom-CA scenario |
|---|---|---|
| native-tls (today) | OS trust store, live | works |
| rustls, webpki-roots only | bundled Mozilla list | **breaks** (regression, rejected) |
| rustls, `rustls-tls-native-roots` | OS trust store via `rustls-native-certs` | works |

Both features enabled: native roots for custom CAs, Mozilla list as fallback for hosts
with a broken/empty system store. `--no-verify` is backend-agnostic and unchanged.
Existing wiremock tests are plain HTTP and unaffected. Combined-roots behavior is
verified during implementation (§9).

## 4. Release workflow — `.github/workflows/npm-publish.yml`

Trigger: `workflow_dispatch` only. No version input — the version is read from
`Cargo.toml` on the dispatched ref (single source of truth; no mismatch possible).

```yaml
permissions:
  contents: read

concurrency:
  group: npm-publish
  cancel-in-progress: false   # serialize accidental double-clicks
```

The `publish` job carries its own `permissions: { contents: read, id-token: write }`
for npm provenance — least privilege, so the build jobs never mint OIDC tokens.

### Jobs

**`build`** (matrix, 3 legs) — produces one binary artifact each:

| Leg | Runner | Build |
|---|---|---|
| darwin-arm64 | `macos-15` | `cargo build --release --target aarch64-apple-darwin` |
| darwin-x64 | `macos-15` | `cargo build --release --target x86_64-apple-darwin` (Apple toolchain cross-compiles mac↔mac; the Intel macos-13 x64 runners are retired, so cross-compile from the arm64 macos-15 runner) |
| linux-x64 | `ubuntu-latest` | `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17` (zig via `pip install ziglang`, cargo-zigbuild via `cargo install cargo-zigbuild --locked`; glibc 2.17 floor) |

Each leg: checkout → `dtolnay/rust-toolchain@stable` with the target added → cache
(`Swatinem/rust-cache`) → build → `file` sanity check on the binary (ELF/Mach-O + arch)
→ upload artifact (`actions/upload-artifact`, names `aarch64-apple-darwin`,
`x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`).

The Linux leg installs `ziglang` pinned to `0.14.1` via a standalone `setup-python`
interpreter (dodging PEP 668's externally-managed system python), then asserts the
built binary's maximum glibc symbol reference is `GLIBC_2.17` before upload.

**`publish`** (needs all three build legs, `ubuntu-latest`):

1. checkout, download the 3 artifacts, setup Node (LTS) + npm ≥ 9.5.
2. `node npm/scripts/stage.mjs` → `target/npm/` with version stamped.
3. Authenticate with `NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}`.
4. For each **platform package**, then the **main package last**:
   - `npm view <name>@<version>` — if it already exists, **skip** (resume-safe: a re-run
     after a half-failed publish fills only the gaps; also guards double-clicks).
   - else `npm publish --access public --provenance`.
5. Publish summary to the job log (published vs skipped per package).

Platform-before-main ordering ensures the main package never references unpublished
optionals.

## 5. Staging script — `npm/scripts/stage.mjs`

Node ≥ 18, stdlib only. Idempotent (clears stale staging first).

1. Read version from `Cargo.toml` (`version = "..."` under `[package]`).
2. Copy each `npm/<pkg>` template → `target/npm/<pkg>/`.
3. Stamp that version into every staged `package.json`.
4. Stamp `optionalDependencies` (exact pin) into the main `package.json`.
5. Copy binaries from artifact paths → `target/npm/restui-*/bin/restui`, `chmod 0o755`.
6. Validate: every binary exists, is non-empty (> 1 MB), and magic bytes match the
   expected format (Mach-O for darwin legs, ELF for linux) — cheap guard against
   staging a broken artifact.
7. **Host-only mode** (flag, e.g. `--only-host`): stage only the current machine's
   platform package + a main package whose `optionalDependencies` lists just that one —
   used by local verification (§7). Exact flag name is an implementation detail.

## 6. Versioning

- Bump `Cargo.toml` → commit → dispatch. That is the entire release ceremony.
- `Cargo.toml` remains the only place a human edits a version. Staged `package.json`
  versions are always derived.
- The workflow fails loudly if the dispatched ref's version is already fully published
  (all four skipped ⇒ error with "bump Cargo.toml first"), rather than silently no-op'ing.

## 7. Local verification (Makefile additions)

Publishing is CI-only; local targets verify packaging without touching the registry:

- `make npm-stage` — build host-target release binary → `stage.mjs --only-host` →
  staged host platform package + main in `target/npm/`.
- `make npm-pack` — `npm-stage` + `npm pack --dry-run` on staged packages → inspect
  tarball contents/sizes, zero risk.
- `make npm-test-local` — the pre-publish safety net: stage host-only, `npm pack` real
  tarballs, create a temp project, install main package from tarball (with the platform
  tarball satisfying its single optional dep), run `restui --help` through the shim,
  assert exit 0 and clap's help output, clean up. Verifies the full
  install→resolve→spawn chain on this machine.

## 8. Error handling

| Failure | Behavior |
|---|---|
| Unsupported platform (win32, linux-arm64) | Shim exits 1 with named platform + fallback links |
| musl Linux | Shim detects, exits 1 pointing to `cargo install` |
| Platform package missing (`--omit=optional`) | Shim exits 1 explaining why + fix |
| Binary spawn failure | Shim exits 1 with the child error |
| Child exit/signal | Propagated verbatim to parent |
| `NPM_TOKEN` invalid/expired | `npm publish` fails; skip-checks mean a re-run after fixing the secret resumes cleanly |
| Workflow double-dispatch | `concurrency` serializes; second run finds versions published and skips/errors per §6 |
| Stale half-published release | Re-run publishes only missing packages |
| Custom TLS CA / musl / Windows users | README troubleshooting section: system-store trust works as today; `--no-verify` escape hatch; `cargo install` fallback |

## 9. Testing

- **Existing suite**: `cargo test` must pass unchanged after the reqwest feature swap
  (wiremock tests are HTTP, backend-agnostic).
- **`make npm-test-local`** in CI? No — it's a local, pre-publish human check (the
  workflow's own staging validation + dry-run-free publish covers CI side).
- **Manual verification checklist (implementation plan must include)**:
  1. HTTPS request to a public endpoint (rustls path).
  2. HTTPS request to a server whose cert chains to a locally-trusted custom CA
     (e.g. `openssl s_server` or mitmproxy + CA in Keychain) — must succeed *without*
     `--no-verify`, proving native-roots behavior.
  3. `--no-verify` still disables verification.
- **Post-publish smoke (first release only)**: fresh `npx restui@<version> --help` on
  macOS arm64; `docker run --rm -it node:bookworm-slim npx -y restui@<version> --help`
  for linux-gnu.

## 10. One-time setup checklist

1. npm account; `npm whoami` locally to confirm.
2. Granular access token: **Read and write**, restricted to packages `restui`,
   `restui-darwin-arm64`, `restui-darwin-x64`, `restui-linux-x64-gnu` (create after
   first publish if package-scoped restriction is awkward on day zero — temporarily
   all-packages, then narrow). Store as repo secret `NPM_TOKEN`.
3. First release: `0.1.0` (current `Cargo.toml`) or an immediate bump — owner's choice.

## 11. Out of scope

- Windows / linux-arm64 / linux-musl packages (shim errors clearly; architecture
  extends naturally later).
- Automated tag-driven releases, changesets, release-please.
- Homebrew / crates.io publication changes (crates.io publish stays manual as today).
- `napi-rs` library bindings (restui is a TUI app; the binary is the product).

## 12. Risks

| Risk | Mitigation |
|---|---|
| `restui-*` names squatted before first publish | Publish soon after merge; names verified free 2026-08-27 |
| rustls behavioral edge (e.g. cert type it won't parse) | Manual checklist §9.2 before release; `--no-verify` escape hatch; cargo fallback documented |
| glibc 2.17 floor insufficiently/overly broad | 2.17 covers CentOS 7-era to modern distros; musl users get a clear error, can request musl package later |
| macos-15 runner defaults target newer macOS than users run | `MACOSX_DEPLOYMENT_TARGET=10.12` env (rustls's own floor) on both darwin build legs |
| Provenance unsupported (npm < 9.5 / registry quirk) | Workflow pins Node LTS (npm ≥ 9.5); `--provenance` failure is a hard error, not silent |
