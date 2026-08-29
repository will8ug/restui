# Releasing

Publishing restui to npm uses [trusted publishing](https://docs.npmjs.com/trusted-publishers/)
(OIDC): no npm tokens, short-lived per-job credentials, automatic provenance. `Cargo.toml`
is the single source of truth for the version — all npm packages are stamped from it at
build time.

## One-time bootstrap (already done? skip to "Publishing a release")

npm only allows configuring a trusted publisher on a package that already exists, so the
first publish of each package is manual and interactive (2FA). You need `npm whoami` to
work locally and about 10 minutes:

1. **Build and stage in CI**: GitHub → Actions → **npm-publish** → Run workflow with
   **publish** unchecked. When it finishes, download the `staged-packages` artifact from
   the run page and unzip it (it contains `restui/`, `restui-darwin-arm64/`,
   `restui-darwin-x64/`, `restui-linux-x64-gnu/`).
2. **Publish 0.1.0 interactively** — platforms first, main last; each prompts for 2FA:

   ```bash
   cd <unzipped>/restui-darwin-arm64  && npm publish --access public
   cd <unzipped>/restui-darwin-x64    && npm publish --access public
   cd <unzipped>/restui-linux-x64-gnu && npm publish --access public
   cd <unzipped>/restui               && npm publish --access public
   ```

3. **Configure the trusted publisher** on each of the four packages (also 2FA):

   ```bash
   for pkg in restui restui-darwin-arm64 restui-darwin-x64 restui-linux-x64-gnu; do
     npm trust github "$pkg" --file npm-publish.yml --repository will8ug/restui --allow-publish
   done
   ```

   (Equivalent web UI: npmjs.com → package → Settings → Trusted publishing → GitHub
   Actions → org `will8ug`, repo `restui`, workflow `npm-publish.yml`, allow publish.)
4. **Harden (recommended by npm)**: on each package's Settings → Publishing access,
   select "Require two-factor authentication and disallow tokens". Trusted publishing
   keeps working — only token-based access is disabled.
5. If an `NPM_TOKEN` secret exists in the repo settings, delete it — nothing uses it now.

The 0.1.0 versions published this way have no provenance attestation (provenance is
generated only for CI publishes); every later release does.

## Publishing a release

1. Bump `version` in `Cargo.toml` and commit to `main`.
2. GitHub → Actions → **npm-publish** → **Run workflow** (on `main`, publish checked —
   the default).
3. Wait for the three build jobs and the publish job. Platform packages publish first,
   the main `restui` package last, authenticated by OIDC with no stored tokens. Packages
   already at the target version are skipped, so re-running a partially-failed release is
   safe.

## Post-release smoke test

```bash
npx -y restui@<version> --help
docker run --rm -it node:bookworm-slim npx -y restui@<version> --help  # linux-gnu
```

Confirm the provenance badge on <https://www.npmjs.com/package/restui>.
