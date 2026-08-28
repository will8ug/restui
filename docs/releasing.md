# Releasing

Publishing restui to npm is a manual, two-step process. `Cargo.toml` is the single
source of truth for the version — all npm packages are stamped from it at build time.

## One-time setup

1. An npm account (`npm whoami` works locally).
2. A granular access token (npmjs.com → Access Tokens → **Granular Access Token**) with
   **Read and write** permission, scoped to the packages `restui`,
   `restui-darwin-arm64`, `restui-darwin-x64`, `restui-linux-x64-gnu`. If package
   scoping is unavailable before the first publish, temporarily allow all packages and
   narrow the token afterwards.
3. Store the token as the `NPM_TOKEN` secret in the repository's
   Settings → Secrets and variables → Actions.

## Publishing a release

1. Bump `version` in `Cargo.toml` and commit to `main`.
2. GitHub → Actions → **npm-publish** → **Run workflow** (on `main`).
3. Wait for the three build jobs and the publish job. Platform packages publish first,
   the main `restui` package last. Packages already at the target version are skipped,
   so re-running a partially-failed release is safe.

The workflow attaches npm provenance (a sigstore attestation linking the packages to
this repository's CI), which shows as a verified badge on the npm package pages.

## Post-release smoke test

```bash
npx -y restui@<version> --help
docker run --rm -it node:bookworm-slim npx -y restui@<version> --help  # linux-gnu
```
