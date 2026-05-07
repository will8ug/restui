# restui - Implementation Plan

## Reference

Design spec: `docs/superpowers/specs/2026-05-07-restui-design.md`

## Phases

The implementation is broken into 5 sequential phases. Within each phase, tasks are independent and can be parallelized.

---

## Phase 1: Project Scaffolding

**Goal**: Compilable Rust project with dependencies, module stubs, and CI config.

### Task 1.1: Initialize Cargo project

- `cargo init` in the `restui/` directory
- Set up `Cargo.toml` with:
  - `[package]` metadata (name, version, edition 2021, authors, description)
  - Dependencies: `ratatui`, `crossterm`, `reqwest` (features: ["blocking", "json"]), `serde_json`, `clap` (features: ["derive"])
  - Dev dependencies: `wiremock`, `tokio` (for wiremock runtime)
- Create module files as stubs:
  - `src/main.rs` — `fn main() {}` placeholder
  - `src/app.rs` — empty structs
  - `src/message.rs` — empty enums
  - `src/parser.rs` — empty module
  - `src/vars.rs` — empty module
  - `src/http.rs` — empty module
  - `src/ui/mod.rs`, `src/ui/request_list.rs`, `src/ui/response_pane.rs`, `src/ui/status_bar.rs`
- `cargo check` must pass

### Task 1.2: CI / Coverage setup

- Create `.github/workflows/ci.yml` (or equivalent) with:
  - `cargo check`
  - `cargo test`
  - `cargo clippy -- -D warnings`
  - `cargo fmt --check`
  - Coverage step using `cargo-tarpaulin` with `--fail-under 80`
- Add `.gitignore` for Rust (target/, Cargo.lock for binaries is committed)

**Exit criteria**: `cargo check` passes, all module files exist, CI config present.

---

## Phase 2: Core Logic (No UI)

**Goal**: Parser, variable resolver, and HTTP client — fully tested, no TUI dependency.

These 3 tasks are **independent** and can be done in parallel.

### Task 2.1: HTTP File Parser (`src/parser.rs`)

Implement:
- `pub fn parse(input: &str) -> Result<ParsedFile, ParseError>`
- Data types: `ParsedFile`, `ParsedRequest`, `Variable`, `Method`, `ParseError`
- Handle all 10 syntax rules from the spec
- Edge cases: empty file, file with only variables, file with only comments, request without name, request without explicit method, multi-line query params

Tests (target 90%+):
- Valid: simple GET, POST with body, multiple requests with `###`, variables, comments, query params
- Edge: empty input, no delimiter (single request), no method defaults to GET, HTTP version ignored
- Errors: invalid method string

### Task 2.2: Variable Resolution (`src/vars.rs`)

Implement:
- `pub fn resolve(variables: &[Variable], request: &ParsedRequest) -> Result<ResolvedRequest, VarError>`
- Data types: `ResolvedRequest`, `VarError`
- Single-pass substitution of `{{name}}` in url, headers, body
- Case-sensitive matching
- Error on undefined variable with context (which variable, which field)

Tests (target 90%+):
- Simple substitution in URL
- Multiple variables in one string
- Variable in header value
- Variable in body
- Undefined variable error with field context
- No variables (passthrough)
- Variable value containing `{{other}}` — not resolved (single pass)

### Task 2.3: HTTP Client Wrapper (`src/http.rs`)

Implement:
- `pub fn send_request(client: &reqwest::blocking::Client, request: &ResolvedRequest) -> Result<AppResponse, HttpError>`
- Data types: `AppResponse`, `HttpError`
- Maps `Method` enum to reqwest methods
- Sets headers, body, sends request
- Captures: status, status_text, response headers, body, content_type, duration, size_bytes

Tests (target 80%+, using wiremock):
- GET 200 response
- POST with JSON body
- Headers forwarded correctly
- Response headers captured
- Duration measured (non-zero)
- Connection error handling
- Timeout handling (mock server with delay)
- Non-JSON response body

**Exit criteria**: `cargo test` passes for all 3 modules, each with coverage targets met. No UI code yet.

---

## Phase 3: App State & Event Loop

**Goal**: `App` struct with `update()` logic and the main event loop wired together. TUI renders but can be tested without a real terminal.

### Task 3.1: Message & Command types (`src/message.rs`)

Implement:
- `Message` enum (all variants from spec)
- `Command` enum: `SendHttp(ResolvedRequest)`, `Quit`, `None`

### Task 3.2: App struct and update logic (`src/app.rs`)

Implement:
- `App::new(file_path, parsed_file)` constructor
- `App::update(&mut self, msg: Message) -> Command`
  - `SelectNext` / `SelectPrev`: wraps around
  - `SendRequest`: resolves variables, returns `Command::SendHttp`, sets status to Sending
  - `ResponseReceived`: stores response, sets status to Idle
  - `ResponseError`: sets status to Error
  - `ToggleFocus`: toggles between RequestList and ResponsePane
  - `ScrollUp` / `ScrollDown`: adjusts scroll_offset (only in ResponsePane focus)
  - `ReloadFile`: re-parses file from disk, updates requests/variables
  - `Quit`: returns `Command::Quit`
  - `Resize`: updates size

Tests (target 80%+):
- Each message variant → expected state change
- Selection wrapping (next at end → 0, prev at 0 → end)
- SendRequest with valid vars → Command::SendHttp
- SendRequest with undefined var → AppStatus::Error
- Focus toggle
- Scroll bounds (don't go negative)
- Reload updates request list

### Task 3.3: Main event loop (`src/main.rs`)

Implement:
- CLI parsing with clap (file path, --timeout, --no-verify, --version)
- File loading and initial parse
- Terminal setup (crossterm raw mode, alternate screen)
- Event loop:
  - Poll crossterm events (50ms timeout)
  - Map crossterm events → Message
  - Check mpsc receiver → Message
  - Call `app.update(msg)` → Command
  - If Command::SendHttp → spawn thread, send via mpsc on completion
  - If Command::Quit → break
  - Call `view(&app, &mut frame)`
- Terminal cleanup on exit (restore terminal)

**Exit criteria**: App compiles, event loop runs, key presses navigate the list (even if rendering is basic/placeholder).

---

## Phase 4: UI Rendering

**Goal**: Full TUI rendering with the split-pane layout from the spec.

These 3 tasks are **independent** and can be done in parallel.

### Task 4.1: Request List pane (`src/ui/request_list.rs`)

Implement:
- Render function taking `&App` and a `Rect` area
- List widget showing request names (or `METHOD /path` fallback)
- `>` marker on selected
- `●` on last-sent
- Highlighted border when focused
- Scrollable when items exceed height

Tests (TestBackend):
- Renders correct number of items
- Selection marker on correct item
- Sent indicator on correct item

### Task 4.2: Response Viewer pane (`src/ui/response_pane.rs`)

Implement:
- Render function taking `&App` and a `Rect` area
- Empty state: "No response yet. Select a request and press Enter."
- Response state: status line, headers, blank line, pretty-printed body
- JSON pretty-printing via `serde_json::to_string_pretty`
- Scroll offset applied to content
- Highlighted border when focused

Tests (TestBackend):
- Empty state renders message
- Response renders status + headers + body
- JSON is pretty-printed
- Scroll offset shifts visible content

### Task 4.3: Status Bar & Layout (`src/ui/mod.rs`, `src/ui/status_bar.rs`)

Implement:
- `pub fn view(app: &App, frame: &mut Frame)` — main layout dispatcher
- Layout: vertical split → [title_bar, horizontal_split[list | response], status_bar]
- Title bar: "restui - {filename}"
- Status bar: keybinding hints (left), duration + size or "Sending..." or error (right)
- Proportions: list pane ~30% width, response ~70%

Tests (TestBackend):
- Layout produces expected areas
- Status bar shows correct text for Idle/Sending/Error states

**Exit criteria**: Full TUI renders correctly. Running `restui sample.http` shows the complete UI. All keybindings work.

---

## Phase 5: Integration & Polish

**Goal**: End-to-end testing, coverage enforcement, final polish.

### Task 5.1: Integration tests (`tests/integration.rs`)

- Full flow: load sample `.http` file → parse → resolve → send to wiremock → verify AppResponse
- Error flows: invalid file, undefined variable, connection error
- Multiple requests in one file

### Task 5.2: Coverage audit & gap-filling

- Run `cargo tarpaulin` (or `cargo llvm-cov`)
- Identify modules below target
- Add tests to cover gaps until overall >= 80%

### Task 5.3: Sample files & README

- Create `examples/sample.http` with 3-4 requests demonstrating features
- Create minimal `README.md` with: what it is, install, usage, keybindings

**Exit criteria**: `cargo test` all green, coverage >= 80%, `cargo clippy` clean, sample file works end-to-end.

---

## Dependency Graph

```
Phase 1 (scaffolding)
    │
    ▼
Phase 2 (core logic) ─── Tasks 2.1, 2.2, 2.3 in parallel
    │
    ▼
Phase 3 (app state + event loop) ─── Tasks 3.1 → 3.2 → 3.3 sequential
    │
    ▼
Phase 4 (UI rendering) ─── Tasks 4.1, 4.2, 4.3 in parallel
    │
    ▼
Phase 5 (integration & polish) ─── Tasks 5.1, 5.2, 5.3
```

## Estimated Effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 | Small | Boilerplate setup |
| Phase 2 | Large | Core logic + comprehensive tests (bulk of testable code) |
| Phase 3 | Medium | Wiring + state machine |
| Phase 4 | Medium | Ratatui widgets + layout |
| Phase 5 | Small | Integration tests + polish |
