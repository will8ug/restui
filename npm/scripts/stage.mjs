#!/usr/bin/env node
// Stage restui npm packages: copy templates from npm/, stamp the version from
// Cargo.toml, place binaries, validate. Never publishes.
//
// Usage: node npm/scripts/stage.mjs [--only-host] [--binaries-dir <dir>] [--out <dir>] [--templates-dir <dir>]
//   --binaries-dir   directory containing <triple>/restui binaries (default: ./artifacts)
//   --out            staging directory (default: ./target/npm)
//   --only-host      stage only the current machine's platform package + main
//   --templates-dir  package templates root (default: ./npm)

import { exit } from 'node:process';
import { access, chmod, copyFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { constants } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const PACKAGES = [
  { name: '@will8ug/restui-darwin-arm64', dir: 'restui-darwin-arm64', triple: 'aarch64-apple-darwin', format: 'macho' },
  { name: '@will8ug/restui-darwin-x64', dir: 'restui-darwin-x64', triple: 'x86_64-apple-darwin', format: 'macho' },
  { name: '@will8ug/restui-linux-x64-gnu', dir: 'restui-linux-x64-gnu', triple: 'x86_64-unknown-linux-gnu', format: 'elf' },
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

const GITHUB_BLOB_BASE = 'https://github.com/will8ug/restui/blob/main/';
const GITHUB_RAW_BASE = 'https://raw.githubusercontent.com/will8ug/restui/main/';

function parseArgs(argv) {
  const args = {
    onlyHost: false,
    binariesDir: path.join(REPO, 'artifacts'),
    out: path.join(REPO, 'target/npm'),
    templatesDir: path.join(REPO, 'npm'),
    readme: path.join(REPO, 'README.md'),
  };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--only-host') args.onlyHost = true;
    else if (argv[i] === '--binaries-dir') args.binariesDir = argv[++i];
    else if (argv[i] === '--out') args.out = argv[++i];
    else if (argv[i] === '--templates-dir') args.templatesDir = argv[++i];
    else if (argv[i] === '--readme') args.readme = argv[++i];
    else {
      console.error(`stage: unknown argument ${argv[i]}`);
      exit(2);
    }
  }
  return args;
}

function absolutizeReadmeLinks(markdown) {
  return markdown.replace(/(!?)\[([^\]]*)\]\(([^)\s]+)\)/g, (match, bang, text, target) => {
    if (/^(https?:|mailto:|#|\/)/.test(target)) return match;
    const base = bang ? GITHUB_RAW_BASE : GITHUB_BLOB_BASE;
    return `${bang}[${text}](${base}${target})`;
  });
}

function die(message) {
  console.error(`stage: ${message}`);
  exit(1);
}

async function cargoVersion() {
  const toml = await readFile(path.join(REPO, 'Cargo.toml'), 'utf8');
  // Scoped to the [package] table: [^\[]*? keeps the match inside the table, so a
  // `version` key in any other table (dependencies, workspace, ...) can't win.
  const match = toml.match(/^\[package\][^\[]*?^version\s*=\s*"([^"]+)"/ms);
  if (!match) die('could not read version from the [package] table of Cargo.toml');
  return match[1];
}

function validateTemplate(pkgName, template) {
  if (!template?.repository?.url) {
    die(
      `${pkgName}/package.json is missing repository.url — it is required for npm provenance ` +
        '(the registry rejects packages without it with E422)'
    );
  }
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

async function stagePackage(pkg, version, outDir, templatesDir) {
  const src = path.join(templatesDir, pkg.dir);
  const dest = path.join(outDir, pkg.dir);
  const template = JSON.parse(await readFile(path.join(src, 'package.json'), 'utf8'));
  if (template.name !== pkg.name) die(`template name mismatch in ${pkg.dir}: expected ${pkg.name}, found ${template.name}`);
  validateTemplate(template.name, template);
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

  // Provenance guard: every template (platform packages and main) must carry
  // repository.url before any staging work begins.
  for (const pkg of selected) {
    const template = JSON.parse(await readFile(path.join(args.templatesDir, pkg.dir, 'package.json'), 'utf8'));
    validateTemplate(template.name, template);
  }
  const mainTemplate = JSON.parse(await readFile(path.join(args.templatesDir, 'restui', 'package.json'), 'utf8'));
  if (mainTemplate.name !== '@will8ug/restui') die(`main template name is ${mainTemplate.name}, expected @will8ug/restui`);
  validateTemplate(mainTemplate.name, mainTemplate);

  await rm(args.out, { recursive: true, force: true });

  for (const pkg of selected) {
    const binPath = await validateBinary(pkg, args.binariesDir);
    const dest = await stagePackage(pkg, version, args.out, args.templatesDir);
    const destBin = path.join(dest, 'bin', 'restui');
    await copyFile(binPath, destBin);
    await chmod(destBin, 0o755);
    console.log(`staged ${pkg.name}@${version}`);
  }

  const mainSrc = path.join(args.templatesDir, 'restui');
  const mainDest = path.join(args.out, 'restui');
  mainTemplate.version = version;
  mainTemplate.optionalDependencies = Object.fromEntries(
    selected.map((p) => [p.name, version])
  );
  await mkdir(path.join(mainDest, 'bin'), { recursive: true });
  await writeFile(path.join(mainDest, 'package.json'), `${JSON.stringify(mainTemplate, null, 2)}\n`);
  const readme = absolutizeReadmeLinks(await readFile(args.readme, 'utf8'));
  await writeFile(path.join(mainDest, 'README.md'), readme);
  await copyFile(path.join(mainSrc, 'LICENSE'), path.join(mainDest, 'LICENSE'));
  await copyFile(path.join(mainSrc, 'bin/restui.js'), path.join(mainDest, 'bin/restui.js'));
  console.log(`staged restui@${version} (main, optionals: ${selected.map((p) => p.name).join(', ')})`);
}

main().catch((err) => die(err.stack ?? String(err)));
