# restui - TUI REST Client Design Spec

## Overview

**restui** is a terminal-based REST client written in Rust. It serves as a TUI alternative to the VS Code REST Client extension (`Huachao/vscode-restclient`). The tool loads `.http`/`.rest` files, displays parsed requests in an interactive list, and shows HTTP responses in a split-pane layout.

## Scope (MVP)

**In scope:**
- Parse `.http`/`.rest` files with `###` delimiters
- Send HTTP requests (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)
- Display responses with pretty-printed JSON
- File-level variables (`@name = value`) with `{{name}}` substitution in URL, headers, and body
- Split-pane TUI: request list (left) + response viewer (right)
- Manual file reload via keybinding

**Out of scope for MVP:**
- External config files / environment switching
- Authentication helpers (users set auth headers manually)
- GraphQL support
- cURL import/export
- Request history persistence
- Code snippet generation
- File watching (auto-reload)
- `.env` file support
- Cookie jar

## Tech Stack

| Component | Choice | Rationale |
|---|---|---|
| Language | Rust | User requirement |
| TUI framework | Ratatui | Active (20k+ stars), modular, TEA pattern support |
| Terminal backend | Crossterm | Cross-platform, ratatui's recommended backend |
| HTTP client | Reqwest (blocking) | Ergonomic API, widely used, blocking avoids Tokio for MVP |
| JSON formatting | serde_json | Pretty-print response bodies |
| CLI parsing | clap | Standard Rust CLI argument parser |
| Test mocking | wiremock (dev) | Local HTTP mock server |
| Coverage | cargo-tarpaulin or cargo-llvm-cov | Enforce 80%+ UT coverage |

## Architecture

### Pattern: The Elm Architecture (TEA)

Single `App` struct holds all state. Events map to `Message` enum variants. `update()` mutates state and optionally returns a `Command`. `view()` renders the current state to a Ratatui frame.

### Module Structure

```
restui/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point: parse CLI, load file, run event loop
│   ├── app.rs               # App struct, update(), AppStatus enum
│   ├── message.rs           # Message enum + Command enum
│   ├── parser.rs            # .http file parser
│   ├── vars.rs              # Variable resolution engine
│   ├── http.rs              # Reqwest wrapper (send request, return response)
│   └── ui/
│       ├── mod.rs           # view() dispatcher
│       ├── request_list.rs  # Left pane rendering
│       ├── response_pane.rs # Right pane rendering
│       └── status_bar.rs    # Bottom bar rendering
├── tests/
│   └── integration.rs       # End-to-end: parse → resolve → send → verify
└── docs/
```

### Module Dependency Flow

```
main.rs → app.rs → parser.rs, vars.rs, http.rs, ui/
```

No module depends on `ui/`. Parser, vars, and http are pure/isolated — they take data in and return data out.

## Data Structures

### Message Enum

```rust
pub enum Message {
    // Navigation
    SelectNext,
    SelectPrev,
    SendRequest,

    // Response lifecycle
    ResponseReceived(AppResponse),
    ResponseError(String),

    // UI
    ToggleFocus,
    ScrollUp,
    ScrollDown,
    ReloadFile,
    Quit,
    Resize(u16, u16),
}
```

### App State

```rust
pub struct App {
    pub file_path: PathBuf,
    pub requests: Vec<ParsedRequest>,
    pub variables: Vec<Variable>,
    pub selected_index: usize,
    pub response: Option<AppResponse>,
    pub status: AppStatus,
    pub focus: Focus,
    pub scroll_offset: usize,
    pub size: (u16, u16),
}

pub enum AppStatus {
    Idle,
    Sending(Instant),
    Error(String),
}

pub enum Focus {
    RequestList,
    ResponsePane,
}
```

### Parser Types

```rust
pub struct ParsedFile {
    pub variables: Vec<Variable>,
    pub requests: Vec<ParsedRequest>,
}

pub struct Variable {
    pub name: String,
    pub value: String,
}

pub struct ParsedRequest {
    pub name: Option<String>,
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub source_line: usize,
}

pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}
```

### HTTP Types

```rust
pub struct ResolvedRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

pub struct AppResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub content_type: Option<String>,
    pub duration: Duration,
    pub size_bytes: usize,
}
```

## .http File Format

### Syntax Rules

1. `###` (3+ `#` chars on a line) is the request delimiter
2. Text immediately after `###` on the same line is the request name (e.g., `### Login`)
3. `@name = value` defines a file-level variable
4. Lines starting with `#` (fewer than 3) or `//` are comments
5. First non-blank, non-comment line of a request block is the request line: `METHOD URL [HTTP/VERSION]`
6. If method is omitted, defaults to `GET`
7. Headers follow the request line until a blank line, format: `Name: Value`
8. Lines starting with `?` or `&` (after optional leading whitespace) following the request line are continuation query parameters, appended to the URL
9. Everything after the blank line (header-body separator) is the request body
10. `{{name}}` references a variable — resolved at send time

### Example File

```http
@host = https://api.example.com
@token = Bearer abc123

### Login
POST {{host}}/auth/login HTTP/1.1
Content-Type: application/json

{
  "username": "admin",
  "password": "secret"
}

### List Users
GET {{host}}/users
  ?role=admin
  &limit=10
Authorization: {{token}}

### Create User
POST {{host}}/users
Content-Type: application/json
Authorization: {{token}}

{
  "name": "Charlie",
  "email": "charlie@example.com"
}
```

## Variable Resolution

### Rules

- File-level variables (`@name = value`) are collected during parsing
- At send time, `{{name}}` is substituted in URL, all header values, and body
- Substitution is a single pass (no recursive resolution)
- Variables are case-sensitive
- Undefined variable → error reported to user (not silently empty)
- Variable values may themselves contain `{{other}}` — these are NOT resolved (single-pass only)

### Interface

```rust
pub fn resolve(
    variables: &[Variable],
    request: &ParsedRequest,
) -> Result<ResolvedRequest, VarError>;
```

## HTTP Execution

### Behavior

- Uses `reqwest::blocking::Client`
- HTTP call runs in a `std::thread::spawn` to keep TUI responsive
- Result sent back via `std::sync::mpsc::channel` as a `Message`
- Default timeout: 30 seconds
- Follows redirects: yes (up to 10 hops, reqwest default)
- SSL verification: on by default (overridable via `--no-verify` flag)
- No cookie persistence for MVP

### Thread Integration

```
[TUI Event Loop] ──spawn──→ [HTTP Thread] ──mpsc::send──→ [Event Loop polls channel]
       │                                                           │
       └── crossterm poll (~50ms tick) ← checks mpsc::Receiver ────┘
```

The event loop:
1. Polls crossterm for terminal events (keys, resize) with a 50ms timeout
2. After each poll (whether event received or timeout), checks the mpsc receiver
3. If a message is available, calls `update()` with it
4. Calls `view()` to render the current state

## UI Layout

```
┌─────────────────────────────────────────────────────────────┐
│ restui - filename.http                                       │  Title bar
├──────────────────────┬──────────────────────────────────────┤
│  Request List        │  Response Viewer                      │
│                      │                                       │
│  > ### Login     ●   │  HTTP/1.1 200 OK                      │
│    ### List Users    │  Content-Type: application/json       │
│    ### Create User   │                                       │
│    ### Delete User   │  {                                    │
│                      │    "users": [                         │
│                      │      { "id": 1, "name": "Alice" }    │
│                      │    ]                                  │
│                      │  }                                    │
│                      │                                       │
├──────────────────────┴──────────────────────────────────────┤
│ ↑↓ navigate │ Enter send │ Tab focus │ r reload │ q quit     │  Status bar
└─────────────────────────────────────────────────────────────┘
```

### Left Pane (Request List)

- Shows request names (from `###` comments) or fallback `METHOD /path`
- `>` marker on currently selected item
- `●` indicator on the last-sent request
- Scrollable if requests exceed pane height
- Highlight style on focused/selected item

### Right Pane (Response Viewer)

- Shows: status line, headers, blank line, body
- JSON bodies are pretty-printed via `serde_json`
- Scrollable with j/k or arrow keys (when focused)
- Empty state: "No response yet. Select a request and press Enter."

### Status Bar

- Left side: keybinding hints
- Right side: after response → duration + body size (e.g., `200ms 1.2KB`)
- During request: "Sending..." with elapsed time
- On error: error message

### Keybindings

| Key | Action |
|---|---|
| `↑` / `k` | Select previous request (list) / Scroll up (response) |
| `↓` / `j` | Select next request (list) / Scroll down (response) |
| `Enter` | Send selected request |
| `Tab` | Toggle focus between panes |
| `r` | Reload `.http` file from disk |
| `q` / `Ctrl+C` | Quit |

### Focus Model

- Two focusable areas: RequestList, ResponsePane
- `Tab` toggles focus
- Arrow/j/k behavior depends on which pane is focused
- Visual indicator (border highlight) shows which pane has focus

## CLI Interface

```
restui <file.http> [options]

Arguments:
  <file>            Path to .http or .rest file

Options:
  --timeout <secs>  Request timeout in seconds [default: 30]
  --no-verify       Disable SSL certificate verification
  --version         Print version and exit
  --help            Print help and exit
```

### Error Handling

- File doesn't exist → print error to stderr, exit code 1
- File has parse errors → launch TUI, show error in status bar
- File has zero requests → launch TUI, show "No requests found" in list pane

## Testing Strategy

### Coverage Target: 80%+ overall (aiming for ~85%)

| Module | Target | Approach |
|---|---|---|
| `parser.rs` | 90%+ | Table-driven: valid inputs, edge cases, error cases |
| `vars.rs` | 90%+ | Substitution permutations, error cases |
| `http.rs` | 80%+ | wiremock mock server: methods, headers, body, timeouts, errors |
| `app.rs` | 80%+ | State transition tests on `update()` — no terminal needed |
| `ui/` | 60-70% | Ratatui `TestBackend` snapshot tests |
| `message.rs` | 100% | Covered by app.rs tests exercising all variants |

### Test Categories

**Unit tests** (in each module):
- Parser: valid files, edge cases (empty, no delimiter, no method, comments), error cases
- Vars: simple substitution, multiple vars, var in URL/header/body, undefined var error
- HTTP: GET/POST/PUT/DELETE with mock server, timeout, connection error, redirect following
- App: all message variants → state transitions, wrap-around selection, focus toggle

**Integration tests** (`tests/`):
- Full flow: parse file → resolve variables → send to mock server → verify response struct
- Error propagation: parse error → app state reflects it correctly

**What is NOT tested:**
- Actual terminal rendering (crossterm syscalls) — manual testing
- Real network calls — all HTTP tests use wiremock mocks

### Coverage Enforcement

- CI runs `cargo tarpaulin --fail-under 80` (or `cargo llvm-cov --fail-under-lines 80`)
- Coverage report generated on every PR

## Future Enhancements (post-MVP)

These are explicitly out of scope but the architecture supports them:

1. Environment support via `restui.toml` config + env switching keybinding
2. `.env` file loading and `{{$dotenv name}}` support
3. System dynamic variables: `{{$timestamp}}`, `{{$guid}}`, `{{$randomInt min max}}`
4. File watching (auto-reload on change)
5. Response syntax highlighting (colors for JSON keys/values)
6. Request history persistence
7. Authentication helpers (Basic, Digest, Bearer)
8. cURL import/export
9. GraphQL support
10. Code snippet generation
