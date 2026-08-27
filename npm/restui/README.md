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

Requires Node 14+.

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
