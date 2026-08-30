#!/usr/bin/env node
'use strict';

// restui bin shim: resolve the platform package for this machine and spawn its
// binary with the terminal attached (the TUI needs the real TTY).

const { spawn } = require('child_process');

const PACKAGES = {
  'darwin-arm64': '@will8ug/restui-darwin-arm64',
  'darwin-x64': '@will8ug/restui-darwin-x64',
  'linux-x64': '@will8ug/restui-linux-x64-gnu',
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
