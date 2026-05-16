# Horizontal Scrolling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add horizontal scrolling to all three TUI panels (Request List, Request Detail, Response) via `←`/`h` and `→`/`l`, mirroring the existing vertical-scroll model.

**Architecture:** Add three new `usize` fields to `App` (one horizontal offset per panel). Add `ScrollLeft`/`ScrollRight` message variants. Two panels (Detail, Response) use `ratatui::Paragraph::scroll((y, x))` natively; the List panel uses a custom `chars().skip(offset).collect()` slice on each item's display string. Offsets persist across focus changes and reset to `0` on content change (selection change or new response), matching the existing vertical-scroll rules. Scroll is unbounded via `saturating_add`/`saturating_sub`.

**Tech Stack:** Rust 2024 edition, ratatui 0.29, crossterm 0.28.

**Spec:** `docs/superpowers/specs/2026-05-16-horizontal-scrolling-design.md` (commit `341f6cb`).

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/message.rs` | Modify | Add `ScrollLeft` / `ScrollRight` message variants |
| `src/app.rs` | Modify | Add 3 horizontal offset fields, 6 handler arms, content-change resets, extend fallback arm, add tests |
| `src/main.rs` | Modify | Map `←`/`h` → `ScrollLeft`, `→`/`l` → `ScrollRight` in `key_message` |
| `src/ui/request_list.rs` | Modify | Add `horizontal_slice` helper; apply offset to item strings; update test helper; add tests |
| `src/ui/request_detail.rs` | Modify | Pass `detail_scroll_offset_x` to `Paragraph::scroll`; update test helper; add render test |
| `src/ui/response_pane.rs` | Modify | Pass `scroll_offset_x` to `Paragraph::scroll`; update test helper; add render test |
| `src/ui/status_bar.rs` | Modify | Update `KEY_HINTS` to `[↑↓←→] Nav`; update test helper; update test |
| `src/ui/help_overlay.rs` | Modify | Add `← / h` and `→ / l` rows to `HELP_TEXT`; extend test assertions |
| `README.md` | Modify | Add `← / h` and `→ / l` rows to keybindings table |

---

## Task ordering rationale

State changes flow outward from the data model to the UI:

1. **Message variants first** (free addition, no consumers yet).
2. **`App` state + handlers + resets** (must come before any UI code can reference the fields).
3. **Key handler** (binds keys to the new messages).
4. **Rendering changes per panel** (each panel reads its new field).
5. **Status bar + help + README** (user-facing surfaces last).

The implementation is TDD-driven: each behavioral change has a test written first.

---

### Task 1: Add `ScrollLeft` and `ScrollRight` message variants

**Files:**
- Modify: `src/message.rs`

- [ ] **Step 1: Add the variants**

Edit `src/message.rs` to add `ScrollLeft` and `ScrollRight` after `ScrollDown`:

```rust
#[derive(Debug)]
pub enum Message {
    SelectNext,
    SelectPrev,
    SendRequest,
    ResponseReceived(AppResponse),
    ResponseError(String),
    ToggleFocus,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ReloadFile,
    ToggleHelp,
    ToggleRequestDetail,
    Quit,
    Resize(u16, u16),
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: Success. The match in `App::update` becomes non-exhaustive at this point; that's fine — `cargo check` reports it as a warning (or error if the match is `#[deny]`-ed) and we fix it in Task 2. If `cargo check` errors with "non-exhaustive match", that's the expected next signal — proceed to Task 2.

- [ ] **Step 3: Commit**

```bash
git add src/message.rs
git commit -m "Add ScrollLeft and ScrollRight message variants"
```

---

### Task 2: Add horizontal offset fields and handlers to `App`

**Files:**
- Modify: `src/app.rs`
- Test: `src/app.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing tests**

Append these tests to the `#[cfg(test)] mod tests` block at the bottom of `src/app.rs`:

```rust
#[test]
fn test_scroll_left_response() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.focus = Focus::ResponsePane;
    app.scroll_offset_x = 3;

    app.update(Message::ScrollLeft);

    assert_eq!(app.scroll_offset_x, 2);
}

#[test]
fn test_scroll_right_response() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.focus = Focus::ResponsePane;

    app.update(Message::ScrollRight);

    assert_eq!(app.scroll_offset_x, 1);
}

#[test]
fn test_scroll_left_at_zero_response() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.focus = Focus::ResponsePane;

    app.update(Message::ScrollLeft);

    assert_eq!(app.scroll_offset_x, 0);
}

#[test]
fn test_scroll_left_detail() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;
    app.focus = Focus::RequestDetail;
    app.detail_scroll_offset_x = 3;

    app.update(Message::ScrollLeft);

    assert_eq!(app.detail_scroll_offset_x, 2);
}

#[test]
fn test_scroll_right_detail() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;
    app.focus = Focus::RequestDetail;

    app.update(Message::ScrollRight);

    assert_eq!(app.detail_scroll_offset_x, 1);
}

#[test]
fn test_scroll_left_list() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.focus = Focus::RequestList;
    app.list_scroll_offset_x = 3;

    app.update(Message::ScrollLeft);

    assert_eq!(app.list_scroll_offset_x, 2);
}

#[test]
fn test_scroll_right_list() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.focus = Focus::RequestList;

    app.update(Message::ScrollRight);

    assert_eq!(app.list_scroll_offset_x, 1);
}

#[test]
fn test_list_horizontal_reset_on_selection_change() {
    let mut app = app_with_requests(vec![
        request("https://example.com/one"),
        request("https://example.com/two"),
    ]);
    app.list_scroll_offset_x = 5;
    app.detail_scroll_offset_x = 7;

    app.update(Message::SelectNext);

    assert_eq!(app.list_scroll_offset_x, 0);
    assert_eq!(app.detail_scroll_offset_x, 0);
}

#[test]
fn test_response_horizontal_reset_on_response_received() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.scroll_offset_x = 5;

    app.update(Message::ResponseReceived(sample_response()));

    assert_eq!(app.scroll_offset_x, 0);
}

#[test]
fn test_scroll_horizontal_persists_across_focus_change() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;
    app.list_scroll_offset_x = 2;
    app.detail_scroll_offset_x = 4;
    app.scroll_offset_x = 6;

    // Cycle full circle: List -> Detail -> Response -> List.
    app.update(Message::ToggleFocus);
    app.update(Message::ToggleFocus);
    app.update(Message::ToggleFocus);

    assert_eq!(app.focus, Focus::RequestList);
    assert_eq!(app.list_scroll_offset_x, 2);
    assert_eq!(app.detail_scroll_offset_x, 4);
    assert_eq!(app.scroll_offset_x, 6);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::tests`
Expected: All 10 new tests fail to compile because the fields `list_scroll_offset_x`, `detail_scroll_offset_x`, `scroll_offset_x` and the messages `ScrollLeft`/`ScrollRight` are not yet handled. The compiler error will be either "no field" or "non-exhaustive patterns". This is the expected red phase.

- [ ] **Step 3: Add the three new fields to the `App` struct**

In `src/app.rs`, modify the `App` struct (around line 24) to add three new fields after the existing scroll-related ones:

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
    pub last_sent_index: Option<usize>,
    pub show_help: bool,
    pub show_request_detail: bool,
    pub detail_scroll_offset: usize,
    pub list_scroll_offset_x: usize,
    pub detail_scroll_offset_x: usize,
    pub scroll_offset_x: usize,
}
```

- [ ] **Step 4: Initialize the new fields in `App::new`**

In `App::new` (around line 41), set all three new fields to `0`:

```rust
impl App {
    pub fn new(file_path: PathBuf, parsed_file: ParsedFile) -> Self {
        Self {
            file_path,
            requests: parsed_file.requests,
            variables: parsed_file.variables,
            selected_index: 0,
            response: None,
            status: AppStatus::Idle,
            focus: Focus::RequestList,
            scroll_offset: 0,
            size: (0, 0),
            last_sent_index: None,
            show_help: false,
            show_request_detail: false,
            detail_scroll_offset: 0,
            list_scroll_offset_x: 0,
            detail_scroll_offset_x: 0,
            scroll_offset_x: 0,
        }
    }
    // ...
}
```

- [ ] **Step 5: Add horizontal-scroll handler arms to `App::update`**

In `App::update` in `src/app.rs`, add six new arms after the existing `Message::ScrollDown if self.focus == Focus::ResponsePane` arm (around line 122). Keep the placement next to vertical scroll handlers for symmetry:

```rust
Message::ScrollLeft if self.focus == Focus::RequestList => {
    self.list_scroll_offset_x = self.list_scroll_offset_x.saturating_sub(1);
    Command::None
}
Message::ScrollRight if self.focus == Focus::RequestList => {
    self.list_scroll_offset_x = self.list_scroll_offset_x.saturating_add(1);
    Command::None
}
Message::ScrollLeft if self.focus == Focus::RequestDetail => {
    self.detail_scroll_offset_x = self.detail_scroll_offset_x.saturating_sub(1);
    Command::None
}
Message::ScrollRight if self.focus == Focus::RequestDetail => {
    self.detail_scroll_offset_x = self.detail_scroll_offset_x.saturating_add(1);
    Command::None
}
Message::ScrollLeft if self.focus == Focus::ResponsePane => {
    self.scroll_offset_x = self.scroll_offset_x.saturating_sub(1);
    Command::None
}
Message::ScrollRight if self.focus == Focus::ResponsePane => {
    self.scroll_offset_x = self.scroll_offset_x.saturating_add(1);
    Command::None
}
```

- [ ] **Step 6: Extend selection-change resets**

In the same `App::update`, find the existing `Message::SelectNext` arm (around line 89) and the `Message::SelectPrev` arm (around line 96). After the existing `self.detail_scroll_offset = 0;` lines in each arm, add the two new horizontal resets:

```rust
Message::SelectNext
    if self.focus == Focus::RequestList && !self.requests.is_empty() =>
{
    self.selected_index = (self.selected_index + 1) % self.requests.len();
    self.detail_scroll_offset = 0;
    self.list_scroll_offset_x = 0;
    self.detail_scroll_offset_x = 0;
    Command::None
}
Message::SelectPrev
    if self.focus == Focus::RequestList && !self.requests.is_empty() =>
{
    self.selected_index = if self.selected_index == 0 {
        self.requests.len() - 1
    } else {
        self.selected_index - 1
    };
    self.detail_scroll_offset = 0;
    self.list_scroll_offset_x = 0;
    self.detail_scroll_offset_x = 0;
    Command::None
}
```

- [ ] **Step 7: Extend `ResponseReceived` reset**

In the same `App::update`, find the `Message::ResponseReceived(response)` arm (around line 140) and add `self.scroll_offset_x = 0;` next to the existing `self.scroll_offset = 0;`:

```rust
Message::ResponseReceived(response) => {
    self.response = Some(response);
    self.status = AppStatus::Idle;
    self.scroll_offset = 0;
    self.scroll_offset_x = 0;
    self.last_sent_index = Some(self.selected_index);
    Command::None
}
```

- [ ] **Step 8: Extend the exhaustive-fallback arm**

At the bottom of the `match msg` block in `App::update` (around line 187), extend the existing fallback to include the new variants:

```rust
Message::SelectNext
| Message::SelectPrev
| Message::ScrollUp
| Message::ScrollDown
| Message::ScrollLeft
| Message::ScrollRight => Command::None,
```

- [ ] **Step 9: Run tests to verify they pass**

Run: `cargo test --lib app::tests`
Expected: All previously-failing tests now pass. All pre-existing tests still pass.

- [ ] **Step 10: Verify clippy is clean**

Run: `cargo clippy -- -D warnings`
Expected: Success.

- [ ] **Step 11: Commit**

```bash
git add src/app.rs
git commit -m "Add horizontal scroll state and handlers to App"
```

---

### Task 3: Wire `←`/`h` and `→`/`l` keys

**Files:**
- Modify: `src/main.rs`

This task has no unit-test coverage because `key_message` is a free function inside `main.rs` and the project does not unit-test it today (the help-mode swallow and arrow-key dispatch are validated via integration / manual testing only). We follow the existing pattern.

- [ ] **Step 1: Add the new key arms to `key_message`**

In `src/main.rs`, find the `key_message` function (around line 141). After the existing `KeyCode::Down | KeyCode::Char('j')` arm, add:

```rust
KeyCode::Left | KeyCode::Char('h') => Some(Message::ScrollLeft),
KeyCode::Right | KeyCode::Char('l') => Some(Message::ScrollRight),
```

The final block should look like:

```rust
match key.code {
    KeyCode::Up | KeyCode::Char('k') => Some(match focus {
        Focus::RequestList => Message::SelectPrev,
        Focus::RequestDetail => Message::ScrollUp,
        Focus::ResponsePane => Message::ScrollUp,
    }),
    KeyCode::Down | KeyCode::Char('j') => Some(match focus {
        Focus::RequestList => Message::SelectNext,
        Focus::RequestDetail => Message::ScrollDown,
        Focus::ResponsePane => Message::ScrollDown,
    }),
    KeyCode::Left | KeyCode::Char('h') => Some(Message::ScrollLeft),
    KeyCode::Right | KeyCode::Char('l') => Some(Message::ScrollRight),
    KeyCode::Enter => Some(Message::SendRequest),
    KeyCode::Tab => Some(Message::ToggleFocus),
    KeyCode::Char('r') => Some(Message::ReloadFile),
    KeyCode::Char('d') => Some(Message::ToggleRequestDetail),
    KeyCode::Char('?') => Some(Message::ToggleHelp),
    KeyCode::Char('q') => Some(Message::Quit),
    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
    _ => None,
}
```

- [ ] **Step 2: Verify compilation and tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Verify clippy is clean**

Run: `cargo clippy -- -D warnings`
Expected: Success.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "Wire arrow keys and h/l to ScrollLeft/ScrollRight"
```

---

### Task 4: Apply horizontal offset to Request Detail render

**Files:**
- Modify: `src/ui/request_detail.rs`
- Test: `src/ui/request_detail.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Update the test helper to include the new fields**

In `src/ui/request_detail.rs`, find `app_with_requests` (around line 74) and add the three new fields:

```rust
fn app_with_requests(requests: Vec<ParsedRequest>) -> App {
    App {
        file_path: "requests.http".into(),
        requests,
        variables: vec![],
        selected_index: 0,
        response: None::<AppResponse>,
        status: AppStatus::Idle,
        focus: Focus::RequestDetail,
        scroll_offset: 0,
        size: (0, 0),
        last_sent_index: None,
        show_help: false,
        show_request_detail: true,
        detail_scroll_offset: 0,
        list_scroll_offset_x: 0,
        detail_scroll_offset_x: 0,
        scroll_offset_x: 0,
    }
}
```

- [ ] **Step 2: Write the failing render test**

In the same `tests` module, add:

```rust
#[test]
fn test_renders_with_horizontal_offset() {
    let mut app = app_with_requests(vec![request(
        Some("Get users"),
        Method::Get,
        "https://example.com/users",
    )]);
    app.detail_scroll_offset_x = 5;

    let backend = render_app(&app);
    let text = buffer_text(&backend);

    // With offset 5, the leading "GET h" is clipped; the panel shows from "ttps://..." onward.
    assert!(!text.contains("GET https://example.com/users"));
    assert!(text.contains("ttps://example.com/users"));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib ui::request_detail::tests::test_renders_with_horizontal_offset`
Expected: FAIL. The render still ignores the offset because the `Paragraph::scroll` call passes `0` for the x coordinate.

- [ ] **Step 4: Update `render` to pass the new offset**

In `src/ui/request_detail.rs`, find the `Paragraph::new(format_request(request))` call (around line 27) and change the `.scroll((app.detail_scroll_offset as u16, 0))` line to:

```rust
Paragraph::new(format_request(request))
    .block(block)
    .scroll((
        app.detail_scroll_offset as u16,
        app.detail_scroll_offset_x as u16,
    ))
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib ui::request_detail::tests`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/request_detail.rs
git commit -m "Apply horizontal offset to request detail render"
```

---

### Task 5: Apply horizontal offset to Response Pane render

**Files:**
- Modify: `src/ui/response_pane.rs`
- Test: `src/ui/response_pane.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Update the test helper to include the new fields**

In `src/ui/response_pane.rs`, find `app_with_response` (around line 77) and add the three new fields:

```rust
fn app_with_response(response: Option<AppResponse>) -> App {
    App {
        file_path: "requests.http".into(),
        requests: Vec::<ParsedRequest>::new(),
        variables: vec![],
        selected_index: 0,
        response,
        status: AppStatus::Idle,
        focus: Focus::ResponsePane,
        scroll_offset: 0,
        size: (0, 0),
        last_sent_index: None,
        show_help: false,
        show_request_detail: false,
        detail_scroll_offset: 0,
        list_scroll_offset_x: 0,
        detail_scroll_offset_x: 0,
        scroll_offset_x: 0,
    }
}
```

- [ ] **Step 2: Write the failing render test**

In the same `tests` module, add:

```rust
#[test]
fn test_renders_with_horizontal_offset() {
    let mut app = app_with_response(Some(sample_response("hello-world", Some("text/plain"))));
    app.scroll_offset_x = 4;

    let backend = render_app(&app);
    let text = buffer_text(&backend);

    // The "HTTP 200 OK" status line is clipped by 4 chars; "200 OK" survives, "HTTP" does not.
    assert!(!text.contains("HTTP 200 OK"));
    assert!(text.contains("200 OK"));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib ui::response_pane::tests::test_renders_with_horizontal_offset`
Expected: FAIL. The render passes `0` for the x coordinate today.

- [ ] **Step 4: Update `render` to pass the new offset**

In `src/ui/response_pane.rs`, find the `Some(response) => Paragraph::new(format_response(response))` branch (around line 23) and change the `.scroll((app.scroll_offset as u16, 0))` line to:

```rust
Some(response) => Paragraph::new(format_response(response))
    .block(block)
    .scroll((
        app.scroll_offset as u16,
        app.scroll_offset_x as u16,
    )),
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib ui::response_pane::tests`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/response_pane.rs
git commit -m "Apply horizontal offset to response pane render"
```

---

### Task 6: Apply horizontal slicing to Request List render

**Files:**
- Modify: `src/ui/request_list.rs`
- Test: `src/ui/request_list.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Update the test helper to include the new fields**

In `src/ui/request_list.rs`, find `app_with_requests` (around line 95) and add the three new fields:

```rust
fn app_with_requests(requests: Vec<ParsedRequest>) -> App {
    App {
        file_path: "requests.http".into(),
        requests,
        variables: vec![],
        selected_index: 0,
        response: None::<AppResponse>,
        status: AppStatus::Idle,
        focus: Focus::RequestList,
        scroll_offset: 0,
        size: (0, 0),
        last_sent_index: None,
        show_help: false,
        show_request_detail: false,
        detail_scroll_offset: 0,
        list_scroll_offset_x: 0,
        detail_scroll_offset_x: 0,
        scroll_offset_x: 0,
    }
}
```

- [ ] **Step 2: Write the failing tests for `horizontal_slice` and the renderer**

Add these tests to the `tests` module in `src/ui/request_list.rs`:

```rust
#[test]
fn test_horizontal_slice_basic() {
    assert_eq!(horizontal_slice(">  List users", 3), "List users");
}

#[test]
fn test_horizontal_slice_past_end() {
    assert_eq!(horizontal_slice("short", 100), "");
}

#[test]
fn test_horizontal_slice_zero_offset() {
    assert_eq!(horizontal_slice(">  List users", 0), ">  List users");
}

#[test]
fn test_renders_list_with_horizontal_offset() {
    let mut app = app_with_requests(vec![request(
        Some("List users"),
        Method::Get,
        "https://example.com/users",
    )]);
    app.list_scroll_offset_x = 4;

    let backend = render_app(&app);
    let text = buffer_text(&backend);

    // Full composed line is ">  List users". With offset 4, the leading ">  L" is clipped, leaving "ist users".
    assert!(!text.contains(">  List users"));
    assert!(!text.contains("List users"));
    assert!(text.contains("ist users"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib ui::request_list::tests`
Expected: The four new tests fail to compile (`horizontal_slice` doesn't exist) or assert (renderer ignores offset).

- [ ] **Step 4: Add the `horizontal_slice` helper**

In `src/ui/request_list.rs`, add a private helper after the existing `http_url_path` function (around line 72) and before the `#[cfg(test)]` block:

```rust
fn horizontal_slice(line: &str, offset: usize) -> String {
    line.chars().skip(offset).collect()
}
```

- [ ] **Step 5: Apply the slice in `render`**

In `src/ui/request_list.rs`, modify the `items` construction block in `render` (around lines 16-34) to apply `horizontal_slice`:

```rust
let items = app
    .requests
    .iter()
    .enumerate()
    .map(|(index, request)| {
        let selected_prefix = if index == app.selected_index {
            ">"
        } else {
            " "
        };
        let sent_prefix = if app.last_sent_index == Some(index) {
            "●"
        } else {
            " "
        };
        let label = request_label(request);
        let full = format!("{selected_prefix}{sent_prefix} {label}");
        let visible = horizontal_slice(&full, app.list_scroll_offset_x);
        ListItem::new(visible)
    })
    .collect::<Vec<_>>();
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib ui::request_list::tests`
Expected: All tests pass, including the pre-existing `test_renders_request_names`, `test_renders_selected_highlight`, `test_renders_sent_indicator`, `test_request_label_prefers_name`, and `test_request_label_falls_back_to_method_and_path`.

Note on the pre-existing `test_renders_selected_highlight` test: it asserts `cell.symbol() == ">"` at position `(1, 1)`. With `list_scroll_offset_x = 0` (the default), the `>` prefix still appears at column 1 of the inner area, so this test passes unchanged.

- [ ] **Step 7: Verify clippy is clean**

Run: `cargo clippy -- -D warnings`
Expected: Success.

- [ ] **Step 8: Commit**

```bash
git add src/ui/request_list.rs
git commit -m "Apply horizontal slicing to request list render"
```

---

### Task 7: Update status bar key hints

**Files:**
- Modify: `src/ui/status_bar.rs`
- Test: `src/ui/status_bar.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Update the test helper to include the new fields**

In `src/ui/status_bar.rs`, find the `app()` helper (around line 78) and add the three new fields:

```rust
fn app() -> App {
    App {
        file_path: "requests.http".into(),
        requests: Vec::<ParsedRequest>::new(),
        variables: vec![],
        selected_index: 0,
        response: None,
        status: AppStatus::Idle,
        focus: Focus::RequestList,
        scroll_offset: 0,
        size: (0, 0),
        last_sent_index: None,
        show_help: false,
        show_request_detail: false,
        detail_scroll_offset: 0,
        list_scroll_offset_x: 0,
        detail_scroll_offset_x: 0,
        scroll_offset_x: 0,
    }
}
```

- [ ] **Step 2: Update the existing keybinding-hint test to expect the new literal**

In the same file, replace the contents of `test_renders_keybinding_hints`:

```rust
#[test]
fn test_renders_keybinding_hints() {
    let backend = render_app(&app());
    let text = buffer_text(&backend);

    assert!(text.contains(
        "[↑↓←→] Nav │ [Enter] Send │ [Tab] Focus │ [d] Detail │ [?] Help │ [q] Quit"
    ));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib ui::status_bar::tests::test_renders_keybinding_hints`
Expected: FAIL — the rendered text still shows `[↑↓] Nav`, not `[↑↓←→] Nav`.

- [ ] **Step 4: Update the `KEY_HINTS` constant**

In `src/ui/status_bar.rs`, change the `KEY_HINTS` constant (line 8):

```rust
const KEY_HINTS: &str =
    "[↑↓←→] Nav │ [Enter] Send │ [Tab] Focus │ [d] Detail │ [?] Help │ [q] Quit";
```

- [ ] **Step 5: Run all status-bar tests**

Run: `cargo test --lib ui::status_bar::tests`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/status_bar.rs
git commit -m "Add horizontal scroll arrows to status bar key hints"
```

---

### Task 8: Update help overlay text

**Files:**
- Modify: `src/ui/help_overlay.rs`
- Test: `src/ui/help_overlay.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Extend `test_help_overlay_renders_shortcuts` to assert the new keys**

In `src/ui/help_overlay.rs`, modify the assertions at the bottom of `test_help_overlay_renders_shortcuts` (around line 81):

```rust
assert!(text.contains("Navigation"));
assert!(text.contains("Enter"));
assert!(text.contains("Quit"));
assert!(text.contains("Scroll left"));
assert!(text.contains("Scroll right"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib ui::help_overlay::tests::test_help_overlay_renders_shortcuts`
Expected: FAIL — `HELP_TEXT` does not contain "Scroll left" / "Scroll right" yet.

- [ ] **Step 3: Update `HELP_TEXT`**

In `src/ui/help_overlay.rs`, replace the `HELP_TEXT` constant (lines 5-19):

```rust
const HELP_TEXT: &str = "\
 Navigation
   ↑ / k     Move up / Scroll up
   ↓ / j     Move down / Scroll down
   ← / h     Scroll left
   → / l     Scroll right
   Tab       Toggle focus between panes

 Actions
   Enter     Send selected request
   d         Toggle request detail
   r         Reload file from disk

 Application
   ?         Toggle this help
   q         Quit
   Ctrl+C    Quit";
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib ui::help_overlay::tests`
Expected: All tests pass.

The overlay's vertical-size constraint (`(area.height * 70 / 100).max(12)`) accommodates two extra lines on any terminal at least 19 rows tall, which is well below typical terminal sizes. No layout change needed.

- [ ] **Step 5: Commit**

```bash
git add src/ui/help_overlay.rs
git commit -m "Document horizontal scroll keys in help overlay"
```

---

### Task 9: Update README keybindings table

**Files:**
- Modify: `README.md`

This task has no test coverage (the README is documentation, not executable).

- [ ] **Step 1: Add the new keybinding rows**

In `README.md`, find the keybindings table (lines 30-39) and add two rows after the `↓ / j` row:

```markdown
| Key | Action |
| --- | --- |
| ↑ / k | Move selection up / Scroll up |
| ↓ / j | Move selection down / Scroll down |
| ← / h | Scroll left |
| → / l | Scroll right |
| Enter | Send selected request |
| Tab | Toggle focus between panes |
| d | Toggle request detail |
| r | Reload file from disk |
| ? | Toggle help |
| q / Ctrl+C | Quit |
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "Document horizontal scroll keys in README"
```

---

### Task 10: Full verification and manual smoke test

**Files:** none modified

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: All tests pass. No regressions.

- [ ] **Step 2: Run clippy with warnings as errors**

Run: `cargo clippy -- -D warnings`
Expected: Success.

- [ ] **Step 3: Check formatting**

Run: `cargo fmt --check`
Expected: Success. If it reports differences, run `cargo fmt` and commit the result as `Run make format`.

- [ ] **Step 4: Manual smoke test**

Pick an `.http` file in `examples/` (e.g. `examples/sample.http`) and run:

```bash
cargo run -- examples/sample.http
```

In the running TUI, verify:

- **Request List panel focused (default):** Press `→` / `l` repeatedly — each request label scrolls left, prefixes (`>`, `●`) scroll off too. Press `←` / `h` to scroll back. Press `j` / `k` to change selection — horizontal offset resets to 0 (you see the full label of the newly-selected item from its start).
- **Tab to Response Pane:** Press Enter to send a request, then `→` / `l` repeatedly — the response content scrolls left. Press `←` / `h` to scroll back.
- **Press `d` to open Request Detail.** Tab to it. Scroll with `→`/`l`. Tab away to Response and back — horizontal offset persists for both panels independently.
- **Selection change resets correctly:** in the Request List, scroll horizontally, then press `j` — the list and detail panel offsets both reset to 0.
- **New response resets correctly:** scroll the response horizontally, then select a different request and press Enter — once the response arrives, the response panel offset resets to 0.
- **Tab cycling preserves offsets:** scroll all three panels horizontally to different positions, Tab through all three back to your starting focus — each panel's offset is preserved.
- **Help overlay:** press `?` — the new `← / h Scroll left` and `→ / l Scroll right` rows appear in the Navigation section. Press `?` to close.
- **Status bar:** verify the leftmost hint reads `[↑↓←→] Nav`.

No assertion script — visual confirmation only. If anything looks wrong, the relevant unit test should be extended to catch it.

- [ ] **Step 5: Final commit (only if any cleanup was needed)**

If `cargo fmt` produced changes in Step 3:

```bash
git add -u
git commit -m "Run make format"
```

Otherwise, this task produces no new commits — the work is complete.
