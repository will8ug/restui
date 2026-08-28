#!/usr/bin/env node
// Local pre-publish verification: build the host binary, stage host-only npm
// packages, pack real tarballs and verify their contents, install into a
// throwaway project, and run the installed `restui --help` through the bin
// shim. Never touches the registry.
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
const USAGE = 'Usage: node npm/scripts/test-local.mjs [--stage-only|--pack-only] [--skip-build]';

const HOST = {
  'darwin-arm64': 'restui-darwin-arm64',
  'darwin-x64': 'restui-darwin-x64',
  'linux-x64': 'restui-linux-x64-gnu',
}[`${process.platform}-${process.arch}`];

if (!HOST) {
  console.error(`test-local: unsupported host ${process.platform}-${process.arch}`);
  exit(1);
}

function parseArgs(argv) {
  const flags = { stageOnly: false, packOnly: false, skipBuild: false };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--stage-only') flags.stageOnly = true;
    else if (argv[i] === '--pack-only') flags.packOnly = true;
    else if (argv[i] === '--skip-build') flags.skipBuild = true;
    else {
      console.error(`test-local: unknown argument ${argv[i]}\n${USAGE}`);
      exit(2);
    }
  }
  if (flags.stageOnly && flags.packOnly) {
    console.error(`test-local: --stage-only and --pack-only are mutually exclusive\n${USAGE}`);
    exit(2);
  }
  return {
    mode: flags.stageOnly ? 'stage' : flags.packOnly ? 'pack' : 'full',
    skipBuild: flags.skipBuild,
  };
}

class CommandError extends Error {
  constructor(cmd, args, cwd, status) {
    super(`command failed (exit ${status ?? '?'}): ${cmd} ${args.join(' ')} in ${cwd ?? process.cwd()}`);
    this.name = 'CommandError';
    this.status = status;
  }
}

function run(cmd, args, opts = {}) {
  // stdio defaults to 'inherit' so users see cargo/npm output; callers that
  // need captured output pass { stdio: 'pipe', encoding: 'utf8' }.
  const res = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  if (res.status !== 0) throw new CommandError(cmd, args, opts.cwd, res.status);
  return res;
}

function verifyTarballContents(tarball, entryPattern) {
  const listing = run('tar', ['-tzf', tarball], { stdio: 'pipe', encoding: 'utf8' }).stdout ?? '';
  const entries = listing.split('\n').map((line) => line.trim()).filter(Boolean);
  if (!entries.some((entry) => entryPattern.test(entry))) {
    throw new Error(
      `${path.basename(tarball)} lists no entry matching ${entryPattern}; actual contents:\n${entries.join('\n')}`
    );
  }
}

async function main() {
  const { mode, skipBuild } = parseArgs(process.argv);
  if (!skipBuild) {
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
    const version = JSON.parse(
      await readFile(path.join(outDir, 'restui/package.json'), 'utf8')
    ).version;
    if (mode === 'stage') {
      console.log(`\nstaged to ${outDir} (host-only)`);
      return;
    }

    const tarballs = path.join(tmp, 'tarballs');
    await mkdir(tarballs, { recursive: true });
    const packed = ['restui', HOST].map((pkg) => path.join(tarballs, `${pkg}-${version}.tgz`));
    for (const pkg of ['restui', HOST]) {
      run('npm', ['pack', '--pack-destination', tarballs], { cwd: path.join(outDir, pkg) });
    }
    if (mode === 'pack') {
      // Self-inspect instead of leaving the tarballs around: `finally` below
      // deletes the tmp dir, so any advertised path would be dead on arrival.
      verifyTarballContents(packed[0], /^package\/bin\/restui\.js$/);
      verifyTarballContents(packed[1], /^package\/bin\/restui$/);
      console.log(
        `\npacked and verified contents: ${packed.map((p) => path.basename(p)).join(', ')} (host-only)`
      );
      return;
    }

    const project = path.join(tmp, 'project');
    await mkdir(project, { recursive: true });
    await writeFile(
      path.join(project, 'package.json'),
      `${JSON.stringify(
        {
          name: 'restui-smoke',
          private: true,
          dependencies: {
            restui: `file:${packed[0]}`,
            [HOST]: `file:${packed[1]}`,
          },
        },
        null,
        2
      )}\n`
    );
    run('npm', ['install', '--no-audit', '--no-fund'], { cwd: project });

    const help = spawnSync(path.join(project, 'node_modules/.bin/restui'), ['--help'], {
      encoding: 'utf8',
      timeout: 30_000,
    });
    // keep in sync with about = "TUI REST Client" in src/main.rs
    if (help.status !== 0 || !/TUI REST Client/.test(help.stdout)) {
      console.error(`test-local: --help through shim failed (exit ${help.status})\n${help.stdout}\n${help.stderr}`);
      throw new Error('installed restui --help did not produce the expected output');
    }
    console.log(`\nPASS: installed restui@${version} runs --help through the shim (${HOST} tarball)`);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

main().catch((err) => {
  if (err instanceof CommandError) {
    console.error(`test-local: ${err.message}`);
    process.exitCode = err.status ?? 1;
  } else {
    console.error(`test-local: ${err.stack ?? String(err)}`);
    process.exitCode = 1;
  }
});
