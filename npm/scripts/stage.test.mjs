import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cp, mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const STAGE = path.join(REPO, 'npm/scripts/stage.mjs');
const PACKAGES = [
  { name: '@will8ug/restui-darwin-arm64', dir: 'restui-darwin-arm64', triple: 'aarch64-apple-darwin', magic: [0xcf, 0xfa, 0xed, 0xfe] },
  { name: '@will8ug/restui-darwin-x64', dir: 'restui-darwin-x64', triple: 'x86_64-apple-darwin', magic: [0xcf, 0xfa, 0xed, 0xfe] },
  { name: '@will8ug/restui-linux-x64-gnu', dir: 'restui-linux-x64-gnu', triple: 'x86_64-unknown-linux-gnu', magic: [0x7f, 0x45, 0x4c, 0x46] },
];
const HOST_TRIPLE = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
}[`${process.platform}-${process.arch}`];

async function cargoVersion() {
  const toml = await readFile(path.join(REPO, 'Cargo.toml'), 'utf8');
  return toml.match(/^\[package\][^\[]*?^version\s*=\s*"([^"]+)"/ms)[1];
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
      const json = JSON.parse(await readFile(path.join(out, pkg.dir, 'package.json'), 'utf8'));
      assert.equal(json.version, version);
      assert.ok(existsSync(path.join(out, pkg.dir, 'bin', 'restui')), `${pkg.name} binary staged`);
    }
    const main = JSON.parse(await readFile(path.join(out, 'restui', 'package.json'), 'utf8'));
    assert.equal(main.version, version);
    assert.deepEqual(main.optionalDependencies, {
      '@will8ug/restui-darwin-arm64': version,
      '@will8ug/restui-darwin-x64': version,
      '@will8ug/restui-linux-x64-gnu': version,
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
    assert.ok(existsSync(path.join(out, hostPkg.dir, 'bin', 'restui')), 'host binary staged');
    for (const other of PACKAGES.filter((p) => p.triple !== HOST_TRIPLE)) {
      assert.ok(!existsSync(path.join(out, other.dir)), `${other.name} not staged`);
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

test('fails when a binary is smaller than 1MB', async (t) => {
  if (!HOST_TRIPLE) return t.skip('host platform not in package set');
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    const hostPkg = PACKAGES.find((p) => p.triple === HOST_TRIPLE);
    await mkdir(path.join(binaries, hostPkg.triple), { recursive: true });
    const buf = Buffer.alloc(1024 * 600, 0); // 600KB — correct magic, too small
    Buffer.from(hostPkg.magic).copy(buf, 0);
    await writeFile(path.join(binaries, hostPkg.triple, 'restui'), buf);
    const res = runStage(['--only-host', '--binaries-dir', binaries, '--out', out]);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /bytes/i);
  });
});

test('clears stale content from the out directory before staging', async (t) => {
  if (!HOST_TRIPLE) return t.skip('host platform not in package set');
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries, { skip: PACKAGES.filter((p) => p.triple !== HOST_TRIPLE).map((p) => p.triple) });
    await mkdir(path.join(out, 'stale-pkg', 'bin'), { recursive: true });
    await writeFile(path.join(out, 'stale-pkg', 'package.json'), '{"stale": true}');
    const res = runStage(['--only-host', '--binaries-dir', binaries, '--out', out]);
    assert.equal(res.status, 0, `stderr: ${res.stderr}`);
    assert.ok(!existsSync(path.join(out, 'stale-pkg')), 'stale package removed');
  });
});

test('fails when a template lacks repository.url (provenance guard)', async (t) => {
  if (!HOST_TRIPLE) return t.skip('host platform not in package set');
  await withTemp(async (dir) => {
    // Copy the real template tree, strip repository from the host platform package.
    const templates = path.join(dir, 'npm');
    await cp(path.join(REPO, 'npm'), templates, { recursive: true });
    const hostPkg = PACKAGES.find((p) => p.triple === HOST_TRIPLE);
    const tmplPath = path.join(templates, hostPkg.dir, 'package.json');
    const tmpl = JSON.parse(await readFile(tmplPath, 'utf8'));
    delete tmpl.repository;
    await writeFile(tmplPath, `${JSON.stringify(tmpl, null, 2)}\n`);
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries, { skip: PACKAGES.filter((p) => p.triple !== HOST_TRIPLE).map((p) => p.triple) });
    const res = runStage(['--only-host', '--binaries-dir', binaries, '--out', out, '--templates-dir', templates]);
    assert.notEqual(res.status, 0);
    assert.match(res.stderr, /repository/i);
    assert.match(res.stderr, new RegExp(hostPkg.name));
  });
});
