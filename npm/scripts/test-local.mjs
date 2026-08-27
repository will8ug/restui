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
