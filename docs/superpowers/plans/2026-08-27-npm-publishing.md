# npm Publishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make restui installable from the npm public registry (`npm i -g restui` / `npx restui`) via prebuilt platform packages published by a manually-triggered GitHub Actions workflow.

**Architecture:** Main `restui` npm package contains a zero-dependency Node shim that resolves and spawns the binary from one of three exact-pinned `optionalDependencies` platform packages (`restui-darwin-arm64`, `restui-darwin-x64`, `restui-linux-x64-gnu`). A `workflow_dispatch` workflow builds the three binaries (native/cross on macos-14, zigbuild on ubuntu), stages the four packages with the version stamped from `Cargo.toml` (single source of truth), and publishes platforms-first with skip-if-exists resume logic and npm provenance.

**Tech Stack:** Rust (reqwest → rustls swap), Node ≥18 stdlib scripts (no npm deps), `cargo-zigbuild` + `pip install ziglang` for glibc 2.17, GitHub Actions (`workflow_dispatch`, OIDC provenance).

**Spec:** `docs/superpowers/specs/2026-08-27-npm-publishing-design.md`

**Conventions:** Commit messages match repo style (imperative, no conventional-commit prefixes — see `git log --oneline`). Never commit anything under `target/` or `artifacts/`. Repo root = `/Users/willyuan/workspaces/will8ug/restui`.

**File map (created/modified across tasks):**

```
Cargo.toml                                  # modify: reqwest features
.gitignore                                  # modify: !npm/**/package.json
npm/restui/package.json                     # create: main package template
npm/restui/bin/restui.js                    # create: bin shim
npm/restui/README.md                        # create: user-facing readme
npm/restui/LICENSE                          # create: copy of repo LICENSE
npm/restui-darwin-arm64/package.json        # create
npm/restui-darwin-arm64/README.md           # create
npm/restui-darwin-x64/package.json          # create
npm/restui-darwin-x64/README.md             # create
npm/restui-linux-x64-gnu/package.json       # create
npm/restui-linux-x64-gnu/README.md          # create
npm/scripts/stage.mjs                       # create: version stamping + staging
npm/scripts/stage.test.mjs                  # create: node:test suite
npm/scripts/test-local.mjs                  # create: local e2e verification
.github/workflows/npm-publish.yml           # create: manual publish workflow
Makefile                                    # modify: npm-* targets
README.md                                   # modify: npm install + Documentation links
docs/tls.md                                 # create: TLS troubleshooting (progressive disclosure)
docs/releasing.md                           # create: maintainer npm release guide
```

---

### Task 1: Switch reqwest to rustls (native roots + webpki fallback)

**Files:**
- Modify: `Cargo.toml:12`

- [ ] **Step 1: Change the reqwest dependency**

Replace line 12 of `Cargo.toml`:

```toml
# before
reqwest = { version = "0.12", features = ["blocking", "json"] }
# after
reqwest = { version = "0.12", default-features = false, features = [
    "blocking",
    "json",
    "rustls-tls-native-roots",
    "rustls-tls-webpki-roots",
] }
```

No source changes needed: `reqwest::blocking::Client::builder()` (`src/main.rs:67`) is
backend-agnostic. `Cargo.lock` will pick up `rustls`, `rustls-native-certs`, `webpki-roots`
and drop `native-tls`/`openssl-sys` on Linux targets.

- [ ] **Step 2: Verify the lockfile resolved as expected**

Run: `cargo tree -e features -i openssl-sys 2>&1; cargo tree -i rustls-native-certs`
Expected: `openssl-sys` → "package ID specification ... did not match any packages"
(not a dependency anywhere); `rustls-native-certs` resolves to reqwest.

- [ ] **Step 3: Run the full test suite and lints**

Run: `cargo test && cargo fmt --check && cargo clippy -- -D warnings`
Expected: all pass (wiremock tests are plain HTTP, backend-agnostic).

- [ ] **Step 4: Manual smoke — public HTTPS still works**

```bash
printf '### Get\nGET https://httpbin.org/get\n' > /tmp/restui-https.http
cargo run -- /tmp/restui-https.http
```

Press Enter on the request, confirm a 200 response renders, then quit with `q`.
If proxied networks block httpbin, any public HTTPS URL you trust is fine.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Switch reqwest to rustls with native and webpki roots"
```

---

### Task 2: gitignore fix + platform package templates

**Files:**
- Modify: `.gitignore`
- Create: `npm/restui-darwin-arm64/package.json`, `npm/restui-darwin-arm64/README.md`
- Create: `npm/restui-darwin-x64/package.json`, `npm/restui-darwin-x64/README.md`
- Create: `npm/restui-linux-x64-gnu/package.json`, `npm/restui-linux-x64-gnu/README.md`

- [ ] **Step 1: Add the gitignore negation**

Immediately after the `package.json` line (line 59) in `.gitignore`, add:

```gitignore
!npm/**/package.json
```

Required because the bare `package.json` pattern ignores the npm templates at any depth.

- [ ] **Step 2: Create the three platform package.json templates**

`npm/restui-darwin-arm64/package.json`:

```json
{
  "name": "restui-darwin-arm64",
  "version": "0.0.0",
  "description": "restui binary for macOS arm64",
  "license": "MIT",
  "os": ["darwin"],
  "cpu": ["arm64"],
  "files": ["bin"]
}
```

`npm/restui-darwin-x64/package.json`:

```json
{
  "name": "restui-darwin-x64",
  "version": "0.0.0",
  "description": "restui binary for macOS x64",
  "license": "MIT",
  "os": ["darwin"],
  "cpu": ["x64"],
  "files": ["bin"]
}
```

`npm/restui-linux-x64-gnu/package.json`:

```json
{
  "name": "restui-linux-x64-gnu",
  "version": "0.0.0",
  "description": "restui binary for Linux x64 (glibc)",
  "license": "MIT",
  "os": ["linux"],
  "cpu": ["x64"],
  "files": ["bin"]
}
```

Notes: `version` is a placeholder — `stage.mjs` stamps the real one. No `exports` field
(the shim deep-resolves `bin/restui`). `0.0.0` is never published because staging always
stamps it first.

- [ ] **Step 3: Create the three one-liner READMEs**

`npm/restui-darwin-arm64/README.md`:

```markdown
# restui-darwin-arm64

Prebuilt `restui` binary for macOS arm64. Installed automatically by the
[`restui`](https://www.npmjs.com/package/restui) package — do not depend on this directly.
```

`npm/restui-darwin-x64/README.md`: same with "macOS x64".
`npm/restui-linux-x64-gnu/README.md`: same with "Linux x64 (glibc)".

- [ ] **Step 4: Verify git tracks the templates**

Run: `git status --short`
Expected: the six `npm/` files listed as untracked (proves the negation works). If any
`npm/**/package.json` is missing from the list, the negation is wrong — fix before committing.

- [ ] **Step 5: Commit**

```bash
git add .gitignore npm/
git commit -m "Add npm platform package templates and gitignore negation"
```

---

### Task 3: Main package template + bin shim

**Files:**
- Create: `npm/restui/package.json`
- Create: `npm/restui/bin/restui.js`
- Create: `npm/restui/README.md`
- Create: `npm/restui/LICENSE` (copy of repo `LICENSE`)

- [ ] **Step 1: Create the main package template**

`npm/restui/package.json`:

```json
{
  "name": "restui",
  "version": "0.0.0",
  "description": "A TUI REST client for .http files",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/will8ug/restui.git"
  },
  "bin": {
    "restui": "bin/restui.js"
  },
  "files": [
    "bin"
  ],
  "engines": {
    "node": ">=14"
  },
  "keywords": [
    "rest",
    "http",
    "tui",
    "client",
    "rest-client",
    "http-file"
  ],
  "optionalDependencies": {}
}
```

`optionalDependencies` is empty `{}` here — `stage.mjs` overwrites it with the exact-pinned
set for the packages actually staged. npm always includes README/LICENSE regardless of `files`.

- [ ] **Step 2: Create the shim**

`npm/restui/bin/restui.js` (executable bit not needed; npm sets it on `bin` links):

```js
#!/usr/bin/env node
'use strict';

// restui bin shim: resolve the platform package for this machine and spawn its
// binary with the terminal attached (the TUI needs the real TTY).

const { spawn } = require('child_process');

const PACKAGES = {
  'darwin-arm64': 'restui-darwin-arm64',
  'darwin-x64': 'restui-darwin-x64',
  'linux-x64': 'restui-linux-x64-gnu',
};

const CARGO_FALLBACK =
  'Install from source instead: cargo install --git https://github.com/will8ug/restui';

function fail(message) {
  console.error(`restui: ${message}`);
  process.exit(1);
}

function platformKey() {
  return `${process.platform}-${process.arch}`;
}

function isMuslLinux() {
  if (process.platform !== 'linux') return false;
  const report = typeof process.report?.getReport === 'function' && process.report.getReport();
  return !report?.header?.glibcVersionRuntime;
}

function resolveBinary() {
  const pkg = PACKAGES[platformKey()];
  if (!pkg) {
    fail(
      `unsupported platform "${platformKey()}". ` +
        'Prebuilt binaries exist for macOS (arm64, x64) and Linux x64 (glibc).\n' +
        CARGO_FALLBACK
    );
  }
  if (isMuslLinux()) {
    fail(
      'musl-based Linux is not supported by the npm prebuilt binaries (glibc only).\n' +
        CARGO_FALLBACK
    );
  }
  try {
    return require.resolve(`${pkg}/bin/restui`);
  } catch {
    fail(
      `platform package "${pkg}" is not installed. ` +
        'This usually happens when installation skipped optional dependencies ' +
        '(e.g. --omit=optional or --no-optional). Reinstall with optional ' +
        `dependencies enabled, or use ${CARGO_FALLBACK}`
    );
  }
}

function main() {
  const bin = resolveBinary();
  const child = spawn(bin, process.argv.slice(2), { stdio: 'inherit' });
  child.on('error', (err) => fail(`failed to launch binary: ${err.message}`));
  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code == null ? 1 : code);
    }
  });
}

main();
```

- [ ] **Step 3: Create the main package README**

`npm/restui/README.md` — condense the repo README for npm users (no repo-internal
Makefile targets):

```markdown
# restui

A TUI REST client for `.http` request files. Prebuilt binaries for macOS (arm64, x64)
and Linux x64 (glibc), distributed as platform-specific optional dependencies.

## Install

```bash
npm install -g restui
# or run once:
npx restui example.http
```

Rust users: `cargo install --git https://github.com/will8ug/restui`.

## Usage

```bash
restui <file.http> [--timeout <secs>] [--no-verify]
```

Keyboard-driven UI: ↑/↓ select, Enter sends, Tab switches panes, r reloads the file,
? toggles help, q quits.

## Troubleshooting

- **Custom/internal TLS certificates**: restui uses the operating system trust store
  (plus Mozilla's root list as fallback). A certificate trusted by your machine works.
- **Self-signed certs you do not want to trust**: `--no-verify` disables certificate
  verification for the session.
- **"unsupported platform" / musl Linux / Windows**: no prebuilt binary exists yet —
  install from source: `cargo install --git https://github.com/will8ug/restui`.
- **"platform package ... is not installed"**: your install skipped optional
  dependencies (`--omit=optional`). Reinstall without it.

License: MIT
```

- [ ] **Step 4: Copy the LICENSE**

Run: `cp LICENSE npm/restui/LICENSE`

- [ ] **Step 5: Syntax-check the shim and template**

Run: `node --check npm/restui/bin/restui.js && node -e "JSON.parse(require('fs').readFileSync('npm/restui/package.json'))" && for f in npm/*/package.json; do node -e "JSON.parse(require('fs').readFileSync('$f'))"; done`
Expected: no output, exit 0.

- [ ] **Step 6: Commit**

```bash
git add npm/restui/
git commit -m "Add main npm package template with platform-resolving bin shim"
```

---

### Task 4: Staging script (TDD)

**Files:**
- Test: `npm/scripts/stage.test.mjs`
- Create: `npm/scripts/stage.mjs`

- [ ] **Step 1: Write the failing test suite**

`npm/scripts/stage.test.mjs`:

```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const STAGE = path.join(REPO, 'npm/scripts/stage.mjs');
const PACKAGES = [
  { name: 'restui-darwin-arm64', triple: 'aarch64-apple-darwin', magic: [0xcf, 0xfa, 0xed, 0xfe] },
  { name: 'restui-darwin-x64', triple: 'x86_64-apple-darwin', magic: [0xcf, 0xfa, 0xed, 0xfe] },
  { name: 'restui-linux-x64-gnu', triple: 'x86_64-unknown-linux-gnu', magic: [0x7f, 0x45, 0x4c, 0x46] },
];
const HOST_TRIPLE = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
}[`${process.platform}-${process.arch}`];

async function cargoVersion() {
  const toml = await readFile(path.join(REPO, 'Cargo.toml'), 'utf8');
  return toml.match(/^version\s*=\s*"([^"]+)"/m)[1];
}

async function makeFakeBinaries(dir, { skip = [], corrupt = [] } = {}) {
  for (const pkg of PACKAGES) {
    if (skip.includes(pkg.triple)) continue;
    await mkdir(path.join(dir, pkg.triple), { recursive: true });
    const buf = Buffer.alloc(1024 * 1024 * 2, 0);
    const magic = corrupt.includes(pkg.triple) ? [0xde, 0xad, 0xbe, 0xef] : pkg.magic;
    Buffer.from(magic).copy(buf, 0);
    await writeFile(path.join(dir, pkg.triple, 'restui'), buf);
  }
}

function runStage(args) {
  return spawnSync(process.execPath, [STAGE, ...args], { encoding: 'utf8' });
}

async function withTemp(fn) {
  const dir = await mkdtemp(path.join(tmpdir(), 'restui-stage-'));
  try {
    return await fn(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

test('stages all four packages with stamped version and exact optionalDependencies', async (t) => {
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries);
    const res = runStage(['--binaries-dir', binaries, '--out', out]);
    assert.equal(res.status, 0, `stderr: ${res.stderr}`);
    const version = await cargoVersion();
    for (const pkg of PACKAGES) {
      const json = JSON.parse(await readFile(path.join(out, pkg.name, 'package.json'), 'utf8'));
      assert.equal(json.version, version);
      assert.ok(existsSync(path.join(out, pkg.name, 'bin', 'restui')), `${pkg.name} binary staged`);
    }
    const main = JSON.parse(await readFile(path.join(out, 'restui', 'package.json'), 'utf8'));
    assert.equal(main.version, version);
    assert.deepEqual(main.optionalDependencies, {
      'restui-darwin-arm64': version,
      'restui-darwin-x64': version,
      'restui-linux-x64-gnu': version,
    });
    assert.ok(existsSync(path.join(out, 'restui', 'bin', 'restui.js')), 'shim staged');
  });
});

test('--only-host stages host platform package and main with a single optional dep', async (t) => {
  if (!HOST_TRIPLE) return t.skip('host platform not in package set');
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries, { skip: PACKAGES.filter((p) => p.triple !== HOST_TRIPLE).map((p) => p.triple) });
    const res = runStage(['--only-host', '--binaries-dir', binaries, '--out', out]);
    assert.equal(res.status, 0, `stderr: ${res.stderr}`);
    const version = await cargoVersion();
    const hostPkg = PACKAGES.find((p) => p.triple === HOST_TRIPLE);
    assert.ok(existsSync(path.join(out, hostPkg.name, 'bin', 'restui')), 'host binary staged');
    for (const other of PACKAGES.filter((p) => p.triple !== HOST_TRIPLE)) {
      assert.ok(!existsSync(path.join(out, other.name)), `${other.name} not staged`);
    }
    const main = JSON.parse(await readFile(path.join(out, 'restui', 'package.json'), 'utf8'));
    assert.deepEqual(main.optionalDependencies, { [hostPkg.name]: version });
  });
});

test('fails with a named error when a binary is missing', async () => {
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    const missing = PACKAGES[2].triple;
    await makeFakeBinaries(binaries, { skip: [missing] });
    const res = runStage(['--binaries-dir', binaries, '--out', out]);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, new RegExp(missing));
  });
});

test('fails when a binary has the wrong magic bytes', async () => {
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries, { corrupt: [PACKAGES[0].triple] });
    const res = runStage(['--binaries-dir', binaries, '--out', out]);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /magic/i);
  });
});
```

- [ ] **Step 2: Run the suite to verify it fails**

Run: `node --test npm/scripts/`
Expected: 4 failing (stage.mjs does not exist), `ENOENT` for `npm/scripts/stage.mjs`.

- [ ] **Step 3: Implement stage.mjs**

`npm/scripts/stage.mjs`:

```js
#!/usr/bin/env node
// Stage restui npm packages: copy templates from npm/, stamp the version from
// Cargo.toml, place binaries, validate. Never publishes.
//
// Usage: node npm/scripts/stage.mjs [--only-host] [--binaries-dir <dir>] [--out <dir>]
//   --binaries-dir  directory containing <triple>/restui binaries (default: ./artifacts)
//   --out           staging directory (default: ./target/npm)
//   --only-host     stage only the current machine's platform package + main

import { exit } from 'node:process';
import { access, chmod, copyFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { constants } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const PACKAGES = [
  { name: 'restui-darwin-arm64', triple: 'aarch64-apple-darwin', format: 'macho' },
  { name: 'restui-darwin-x64', triple: 'x86_64-apple-darwin', format: 'macho' },
  { name: 'restui-linux-x64-gnu', triple: 'x86_64-unknown-linux-gnu', format: 'elf' },
];

const HOST_TRIPLE = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
}[`${process.platform}-${process.arch}`];

// First bytes of the file: Mach-O 64-bit LE (MH_MAGIC_64), fat binary, or ELF.
const MAGIC = {
  macho: [
    [0xcf, 0xfa, 0xed, 0xfe],
    [0xca, 0xfe, 0xba, 0xbe],
  ],
  elf: [[0x7f, 0x45, 0x4c, 0x46]],
};

const MIN_BINARY_BYTES = 1024 * 1024;

function parseArgs(argv) {
  const args = {
    onlyHost: false,
    binariesDir: path.join(REPO, 'artifacts'),
    out: path.join(REPO, 'target/npm'),
  };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--only-host') args.onlyHost = true;
    else if (argv[i] === '--binaries-dir') args.binariesDir = argv[++i];
    else if (argv[i] === '--out') args.out = argv[++i];
    else {
      console.error(`stage: unknown argument ${argv[i]}`);
      exit(2);
    }
  }
  return args;
}

function die(message) {
  console.error(`stage: ${message}`);
  exit(1);
}

async function cargoVersion() {
  const toml = await readFile(path.join(REPO, 'Cargo.toml'), 'utf8');
  const match = toml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) die('could not read version from Cargo.toml');
  return match[1];
}

async function readBinaryHead(filePath) {
  const handle = await readFile(filePath).catch(() => null);
  if (!handle || handle.length === 0) return null;
  return handle.subarray(0, 4);
}

async function validateBinary(pkg, binariesDir) {
  const binPath = path.join(binariesDir, pkg.triple, 'restui');
  const head = await readBinaryHead(binPath);
  if (!head) die(`binary missing or empty for ${pkg.triple} (expected at ${binPath})`);
  if (head.length < 4) die(`binary too small for ${pkg.triple}`);
  const stat = await readFile(binPath);
  if (stat.length < MIN_BINARY_BYTES) {
    die(`binary for ${pkg.triple} is only ${stat.length} bytes (expected > ${MIN_BINARY_BYTES})`);
  }
  const ok = MAGIC[pkg.format].some((magic) => Buffer.from(magic).equals(head.subarray(0, 4)));
  if (!ok) {
    die(`binary for ${pkg.triple} has unexpected magic bytes ${head.toString('hex')} — not ${pkg.format}`);
  }
  return binPath;
}

async function stagePackage(pkg, version, outDir) {
  const src = path.join(REPO, 'npm', pkg.name);
  const dest = path.join(outDir, pkg.name);
  const template = JSON.parse(await readFile(path.join(src, 'package.json'), 'utf8'));
  template.version = version;
  await mkdir(path.join(dest, 'bin'), { recursive: true });
  await writeFile(path.join(dest, 'package.json'), `${JSON.stringify(template, null, 2)}\n`);
  for (const extra of ['README.md', 'LICENSE']) {
    const extraPath = path.join(src, extra);
    if (await access(extraPath, constants.F_OK).then(() => true, () => false)) {
      await copyFile(extraPath, path.join(dest, extra));
    }
  }
  return dest;
}

async function main() {
  const args = parseArgs(process.argv);
  const version = await cargoVersion();

  const selected = args.onlyHost ? PACKAGES.filter((p) => p.triple === HOST_TRIPLE) : PACKAGES;
  if (args.onlyHost && selected.length === 0) {
    die(`--only-host: this machine (${process.platform}-${process.arch}) has no matching platform package`);
  }

  await rm(args.out, { recursive: true, force: true });

  for (const pkg of selected) {
    const binPath = await validateBinary(pkg, args.binariesDir);
    const dest = await stagePackage(pkg, version, args.out);
    const destBin = path.join(dest, 'bin', 'restui');
    await copyFile(binPath, destBin);
    await chmod(destBin, 0o755);
    console.log(`staged ${pkg.name}@${version}`);
  }

  const mainSrc = path.join(REPO, 'npm/restui');
  const mainDest = path.join(args.out, 'restui');
  const mainTemplate = JSON.parse(await readFile(path.join(mainSrc, 'package.json'), 'utf8'));
  mainTemplate.version = version;
  mainTemplate.optionalDependencies = Object.fromEntries(
    selected.map((p) => [p.name, version])
  );
  await mkdir(path.join(mainDest, 'bin'), { recursive: true });
  await writeFile(path.join(mainDest, 'package.json'), `${JSON.stringify(mainTemplate, null, 2)}\n`);
  await copyFile(path.join(mainSrc, 'README.md'), path.join(mainDest, 'README.md'));
  await copyFile(path.join(mainSrc, 'LICENSE'), path.join(mainDest, 'LICENSE'));
  await copyFile(path.join(mainSrc, 'bin/restui.js'), path.join(mainDest, 'bin/restui.js'));
  console.log(`staged restui@${version} (main, optionals: ${selected.map((p) => p.name).join(', ')})`);
}

main().catch((err) => die(err.stack ?? String(err)));
```

- [ ] **Step 4: Run the suite to verify it passes**

Run: `node --test npm/scripts/`
Expected: 4 pass (1 may report `skip` if run on an unsupported host — on this repo's
darwin-arm64 dev machine all 4 pass).

- [ ] **Step 5: Commit**

```bash
git add npm/scripts/stage.mjs npm/scripts/stage.test.mjs
git commit -m "Add npm package staging script with validation and tests"
```

---

### Task 5: Local e2e verification script + Makefile targets

**Files:**
- Create: `npm/scripts/test-local.mjs`
- Modify: `Makefile`

- [ ] **Step 1: Implement test-local.mjs**

`npm/scripts/test-local.mjs` (exercises build → stage → pack → install → run, on the
host platform only):

```js
#!/usr/bin/env node
// Local pre-publish verification: build the host binary, stage host-only npm
// packages, pack real tarballs, install into a throwaway project, and run the
// installed `restui --help` through the bin shim. Never touches the registry.
//
// Usage: node npm/scripts/test-local.mjs [--stage-only|--pack-only] [--skip-build]

import { exit } from 'node:process';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const STAGE = path.join(REPO, 'npm/scripts/stage.mjs');

const HOST = {
  'darwin-arm64': 'restui-darwin-arm64',
  'darwin-x64': 'restui-darwin-x64',
  'linux-x64': 'restui-linux-x64-gnu',
}[`${process.platform}-${process.arch}`];

if (!HOST) {
  console.error(`test-local: unsupported host ${process.platform}-${process.arch}`);
  exit(1);
}

const mode = process.argv.includes('--stage-only')
  ? 'stage'
  : process.argv.includes('--pack-only')
    ? 'pack'
    : 'full';

function run(cmd, args, opts = {}) {
  const res = spawnSync(cmd, args, { stdio: opts.stdio ?? 'inherit', cwd: opts.cwd, encoding: 'utf8' });
  if (res.status !== 0) {
    console.error(`test-local: command failed: ${cmd} ${args.join(' ')}`);
    exit(res.status ?? 1);
  }
  return res;
}

async function main() {
  if (!process.argv.includes('--skip-build')) {
    run('cargo', ['build', '--release'], { cwd: REPO });
  }

  const tmp = await mkdtemp(path.join(tmpdir(), 'restui-e2e-'));
  try {
    const TRIPLE = {
      'restui-darwin-arm64': 'aarch64-apple-darwin',
      'restui-darwin-x64': 'x86_64-apple-darwin',
      'restui-linux-x64-gnu': 'x86_64-unknown-linux-gnu',
    }[HOST];
    const binariesDir = path.join(tmp, 'artifacts');
    const tripleDir = path.join(binariesDir, TRIPLE);
    await mkdir(tripleDir, { recursive: true });
    run('cp', [path.join(REPO, 'target/release/restui'), path.join(tripleDir, 'restui')]);

    const outDir = path.join(REPO, 'target/npm');
    run(process.execPath, [STAGE, '--only-host', '--binaries-dir', binariesDir, '--out', outDir]);
    if (mode === 'stage') {
      console.log(`\nstaged to ${outDir} (host-only)`);
      return;
    }

    const tarballs = path.join(tmp, 'tarballs');
    await mkdir(tarballs, { recursive: true });
    for (const pkg of ['restui', HOST]) {
      run('npm', ['pack', '--pack-destination', tarballs], { cwd: path.join(outDir, pkg) });
    }
    if (mode === 'pack') {
      console.log(`\ntarballs in ${tarballs}`);
      return;
    }

    const version = JSON.parse(
      await readFile(path.join(outDir, 'restui/package.json'), 'utf8')
    ).version;
    const project = path.join(tmp, 'project');
    await mkdir(project, { recursive: true });
    await writeFile(
      path.join(project, 'package.json'),
      `${JSON.stringify(
        {
          name: 'restui-smoke',
          private: true,
          dependencies: {
            restui: `file:${path.join(tarballs, `restui-${version}.tgz`)}`,
            [HOST]: `file:${path.join(tarballs, `${HOST}-${version}.tgz`)}`,
          },
        },
        null,
        2
      )}\n`
    );
    run('npm', ['install', '--no-audit', '--no-fund'], { cwd: project });

    const help = spawnSync(path.join(project, 'node_modules/.bin/restui'), ['--help'], {
      encoding: 'utf8',
    });
    if (help.status !== 0 || !/TUI REST Client/.test(help.stdout)) {
      console.error(`test-local: --help through shim failed (exit ${help.status})\n${help.stdout}\n${help.stderr}`);
      exit(1);
    }
    console.log(`\nPASS: installed restui@${version} runs --help through the shim (${HOST} tarball)`);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(`test-local: ${err.stack ?? String(err)}`);
  exit(1);
});
```

Note: clap's `--help` output contains `TUI REST Client` (from `#[command(about)]` in
`src/main.rs:24`) — that's the assertion target.

- [ ] **Step 2: Add Makefile targets**

In `Makefile`, update the `.PHONY` line and append (keeping `## help` style):

```makefile
.PHONY: build test clippy fmt lint check run run-sample install clean coverage coverage-ci help npm-test npm-stage npm-pack npm-test-local
```

```makefile
npm-test: ## Run npm packaging script tests
	node --test npm/scripts/

npm-stage: ## Build host binary and stage host-only npm packages in target/npm
	node npm/scripts/test-local.mjs --stage-only

npm-pack: ## Stage and pack host-only npm tarballs (dry inspection)
	node npm/scripts/test-local.mjs --pack-only

npm-test-local: ## Full local e2e: build, stage, pack, install, run --help via shim
	node npm/scripts/test-local.mjs
```

- [ ] **Step 3: Run the full local e2e**

Run: `make npm-test-local`
Expected: ends with `PASS: installed restui@0.1.0 runs --help through the shim (...)`
(uses whatever `Cargo.toml` version is current).

- [ ] **Step 4: Run the script tests too**

Run: `make npm-test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add npm/scripts/test-local.mjs Makefile
git commit -m "Add local npm packaging verification targets"
```

---

### Task 6: GitHub Actions publish workflow

**Files:**
- Create: `.github/workflows/npm-publish.yml`

- [ ] **Step 1: Write the workflow**

`.github/workflows/npm-publish.yml`:

```yaml
name: npm-publish

on:
  workflow_dispatch:

permissions:
  contents: read
  id-token: write # npm provenance (sigstore attestation)

concurrency:
  group: npm-publish
  cancel-in-progress: false

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - triple: aarch64-apple-darwin
            runs-on: macos-14
          - triple: x86_64-apple-darwin
            runs-on: macos-14
          - triple: x86_64-unknown-linux-gnu
            runs-on: ubuntu-latest
            zigbuild: true
    runs-on: ${{ matrix.runs-on }}
    env:
      MACOSX_DEPLOYMENT_TARGET: "10.12" # rustls floor; no-op on linux
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.triple }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.triple }}
      - name: Install cargo-zigbuild
        if: matrix.zigbuild
        run: |
          pip install ziglang
          cargo install cargo-zigbuild --locked
      - name: Build (zigbuild, glibc 2.17 floor)
        if: matrix.zigbuild
        run: cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
      - name: Build
        if: ${{ !matrix.zigbuild }}
        run: cargo build --release --target ${{ matrix.triple }}
      - name: Verify binary
        run: file target/${{ matrix.triple }}/release/restui
      - uses: actions/upload-artifact@v7
        with:
          name: ${{ matrix.triple }}
          path: target/${{ matrix.triple }}/release/restui
          if-no-files-found: error

  publish:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          registry-url: https://registry.npmjs.org
      - uses: actions/download-artifact@v6
        with:
          name: aarch64-apple-darwin
          path: artifacts/aarch64-apple-darwin
      - uses: actions/download-artifact@v6
        with:
          name: x86_64-apple-darwin
          path: artifacts/x86_64-apple-darwin
      - uses: actions/download-artifact@v6
        with:
          name: x86_64-unknown-linux-gnu
          path: artifacts/x86_64-unknown-linux-gnu
      - name: Stage packages
        run: node npm/scripts/stage.mjs --binaries-dir artifacts
      - name: Publish (platforms first, main last, skip existing)
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: |
          set -euo pipefail
          VERSION=$(node -p "require('./target/npm/restui/package.json').version")
          packages=(restui-darwin-arm64 restui-darwin-x64 restui-linux-x64-gnu restui)
          published=0
          for name in "${packages[@]}"; do
            dir="target/npm/$name"
            if npm view "$name@$VERSION" --loglevel=error >/dev/null 2>&1; then
              echo "::notice::$name@$VERSION already published — skipping"
              continue
            fi
            (cd "$dir" && npm publish --access public --provenance)
            echo "published $name@$VERSION"
            published=$((published + 1))
          done
          if [ "$published" -eq 0 ]; then
            echo "::error::All packages at version $VERSION are already published. Bump version in Cargo.toml and re-run."
            exit 1
          fi
          echo "Done: published $published package(s) at $VERSION"
```

Facts this relies on: `cargo zigbuild --target x86_64-unknown-linux-gnu.2.17` writes to
`target/x86_64-unknown-linux-gnu/release/` (suffix stripped); `npm publish` reads
`NODE_AUTH_TOKEN` when `registry-url` is configured by setup-node; `--provenance`
requires npm ≥ 9.5 (Node 22 bundles npm 10).

- [ ] **Step 2: Sanity-check the YAML parses**

Run: `node -e "const yaml=require('fs').readFileSync('.github/workflows/npm-publish.yml','utf8'); console.log('bytes:', yaml.length)"`
(No YAML parser is available without adding deps; the real validation is the workflow
lint in GitHub's UI after push. Cross-check action versions against `ci.yml`, which uses
checkout@v6 and upload-artifact@v7.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/npm-publish.yml
git commit -m "Add manual npm publish workflow with provenance and resume-safe skips"
```

---

### Task 7: Docs — README updates + progressive-disclosure sub-docs

**Files:**
- Modify: `README.md` (Installation section; new Documentation section)
- Create: `docs/tls.md`
- Create: `docs/releasing.md`

Arrangement follows httptui's docs style: README stays a clean overview; details live
in `docs/*.md`, linked with one-line descriptions. Exception: `npm/restui/README.md`
(Task 3) keeps its troubleshooting inline — it is the npm landing page and cannot rely
on GitHub-relative links.

- [ ] **Step 1: Update the Installation section in README.md**

Replace the current section:

```markdown
## Installation

```bash
cargo install --path .
```
```

with:

```markdown
## Installation

**npm** (prebuilt binaries for macOS arm64/x64 and Linux x64):

```bash
npm install -g restui
# or run once without installing:
npx restui file.http
```

**cargo** (any platform with a Rust toolchain):

```bash
cargo install --git https://github.com/will8ug/restui
```
```

- [ ] **Step 2: Add a Documentation section to README.md**

Append at the end of `README.md` (after the Example `.http` file section):

```markdown
## Documentation

- [TLS Troubleshooting](docs/tls.md) — Custom certificate authorities, `--no-verify`, and platform support.
- [Releasing](docs/releasing.md) — Maintainer guide to publishing a new version to npm.
```

No troubleshooting details inline in the README — that is the point of the arrangement.

- [ ] **Step 3: Create docs/tls.md**

`docs/tls.md`:

```markdown
# TLS Troubleshooting

restui uses rustls and loads the operating system's certificate store by default
(macOS Keychain, Linux system certificate directories), with Mozilla's root list as
a fallback. Certificates your machine already trusts — including corporate or
locally-installed CAs — work without any flags.

## Common fixes

### 1. Trust your custom CA in the OS

Add the CA certificate to your system store and restui picks it up automatically on
the next start:

- **macOS**: `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ca.crt`
- **Debian/Ubuntu**: copy the cert to `/usr/local/share/ca-certificates/` and run `sudo update-ca-certificates`
- **Fedora/RHEL**: copy the cert to `/etc/pki/ca-trust/source/anchors/` and run `sudo update-ca-trust`

### 2. Skip certificate verification (not recommended)

As a last resort, disable TLS verification entirely:

```bash
restui --no-verify api.http
```

**Warning:** this disables all certificate checks, making connections vulnerable to
man-in-the-middle attacks. Use only for local development or trusted networks.

## Platform support

The npm package ships prebuilt binaries for macOS (arm64, x64) and Linux x64 (glibc).
On musl-based Linux, Windows, or other architectures, the `restui` shim exits with a
clear error — install from source instead:

```bash
cargo install --git https://github.com/will8ug/restui
```
```

- [ ] **Step 4: Create docs/releasing.md**

`docs/releasing.md`:

```markdown
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
```

- [ ] **Step 5: Commit**

```bash
git add README.md docs/tls.md docs/releasing.md
git commit -m "Document npm installation, TLS troubleshooting, and release process"
```

---

### Task 8: Full verification + one-time setup + release runbook

**Files:** none created (verification + documented setup)

- [ ] **Step 1: Full local gate**

Run: `make lint && cargo test && make npm-test && make npm-test-local`
Expected: everything green, e2e ends with `PASS: ...`.

- [ ] **Step 2: Manual TLS verification (custom CA, macOS)**

Run a local TLS server with a self-signed CA and confirm restui trusts it via the
system store — no `--no-verify`:

```bash
mkdir -p /tmp/restui-ca && cd /tmp/restui-ca
openssl req -x509 -newkey rsa:2048 -keyout ca.key -out ca.crt -days 2 -nodes -subj "/CN=restui-test-ca"
openssl req -newkey rsa:2048 -keyout srv.key -out srv.csr -nodes -subj "/CN=localhost"
openssl x509 -req -in srv.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out srv.crt -days 2
openssl s_server -accept 8443 -key srv.key -cert srv.crt -www >/dev/null 2>&1 & echo $! > s_server.pid
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ca.crt
printf '### TLS check\nGET https://localhost:8443\n' > /tmp/restui-ca/tls.http
cargo run --manifest-path /Users/willyuan/workspaces/will8ug/restui/Cargo.toml -- /tmp/restui-ca/tls.http
```

In the TUI, press Enter on the request. Expected: a response (s_server returns an HTML
body over TLS) with **no** certificate error. Then clean up:

```bash
sudo security remove-trusted-cert -d ca.crt
kill $(cat s_server.pid)
```

Optional strictness check: `--no-verify` against an *untrusted* cert should also
succeed (verifies the flag still disables verification).

- [ ] **Step 3: One-time npm setup (human steps — full guide in `docs/releasing.md`)**

Verify `docs/releasing.md` (created in Task 7) covers these, then perform them:

1. npm account exists, `npm whoami` works locally.
2. Granular access token created (read/write, scoped to the four restui packages —
   see `docs/releasing.md` for the day-zero fallback).
3. Token stored as the `NPM_TOKEN` secret in repo settings.

- [ ] **Step 4: Post-merge release runbook (first release — documented in `docs/releasing.md`)**

1. Push/merge all commits to `main`.
2. GitHub → Actions → **npm-publish** → Run workflow (on `main`). Version comes from
   `Cargo.toml` (`0.1.0` unless bumped first).
3. Watch the run: 3 build legs, then publish publishes 4 packages (platforms first).
4. Smoke per `docs/releasing.md`: `npx -y restui@0.1.0 --help` locally;
   `docker run --rm -it node:bookworm-slim npx -y restui@0.1.0 --help` for linux-gnu.
5. Verify provenance badge on npmjs.com/package/restui.

---

## Self-review notes

- **Spec coverage:** §1 (Task 2/3), §2 layout + gitignore (Task 2), §3 TLS (Task 1 + Task 8.2),
  §4 workflow (Task 6), §5 staging incl. host-only mode + magic validation (Task 4),
  §6 versioning + all-skipped error (Task 6 publish step), §7 local targets (Task 5),
  §8 error paths (shim Task 3 + workflow Task 6), §9 testing (Tasks 1, 4, 5, 8 + post-merge
  smoke 8.4), §10 setup (Tasks 7 + 8.3, documented in docs/releasing.md), §11/12 no
  action needed. README keeps a clean overview with Documentation links (httptui-style
  progressive disclosure); troubleshooting detail lives in docs/tls.md.
- **Type consistency:** triples (`aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`) and artifact layout `<binaries-dir>/<triple>/restui` are
  identical in `stage.mjs`, `stage.test.mjs`, `test-local.mjs`, and the workflow.
  Package names identical in templates, shim map, and publish loop.
- **Placeholders:** none; every code step contains complete code, every run step has
  expected output.
