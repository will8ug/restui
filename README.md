# restui

[![CI](https://github.com/will8ug/restui/actions/workflows/ci.yml/badge.svg)](https://github.com/will8ug/restui/actions/workflows/ci.yml)

A terminal UI REST client for `.http` request files.

## Features

- Parse REST Client-style `.http` files with multiple named requests
- Resolve `{{variables}}` in URLs, headers, and bodies
- Send requests from a keyboard-driven terminal interface
- Inspect formatted responses, headers, timing, and size metadata
- Reload request files without restarting the app

## Installation

```bash
cargo install --path .
```

## Usage

```bash
restui <file.http> [--timeout <secs>] [--no-verify]
```

## Keybindings

| Key | Action |
| --- | --- |
| ↑ / k | Move selection up |
| ↓ / j | Move selection down |
| Enter | Send selected request |
| Tab | Toggle focus between panes |
| r | Reload file from disk |
| q / Ctrl+C | Quit |

## Example `.http` file

```http
@host = https://httpbin.org
@content_type = application/json

### Get request
GET {{host}}/get
Accept: {{content_type}}

### Post with JSON body
POST {{host}}/post
Content-Type: {{content_type}}

{
  "name": "restui",
  "version": "0.1.0"
}
```

## License

MIT
