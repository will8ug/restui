# Request Detail Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a togglable request detail panel that displays the selected request's method, URL, headers, and body in the top-right area of the TUI.

**Architecture:** Extend the existing Focus enum to include `RequestDetail`, add a `show_request_detail` bool toggle, and conditionally split the right pane into a 30/70 vertical layout. New `request_detail.rs` module follows the same render pattern as `response_pane.rs`.

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.28

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/message.rs` | Modify | Add `ToggleRequestDetail` message variant |
| `src/app.rs` | Modify | Add fields, extend Focus, add handlers |
| `src/ui/request_detail.rs` | Create | Render request detail panel |
| `src/ui/mod.rs` | Modify | Conditional layout split, register module |
| `src/ui/status_bar.rs` | Modify | Replace `r reload` with `d detail` hint |
| `src/main.rs` | Modify | Add `d` key binding, update focus match arms |

---

### Task 1: Add `ToggleRequestDetail` Message

**Files:**
- Modify: `src/message.rs`

- [ ] **Step 1: Add the message variant**

In `src/message.rs`, add `ToggleRequestDetail` to the `Message` enum:

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
    ReloadFile,
    ToggleHelp,
    ToggleRequestDetail,
    Quit,
    Resize(u16, u16),
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: Success (the variant is not yet used anywhere, which Rust allows for enum variants)

- [ ] **Step 3: Commit**

```bash
git add src/message.rs
git commit -m "Add ToggleRequestDetail message variant"
```

---

### Task 2: Extend App State and Focus Enum

**Files:**
- Modify: `src/app.rs`
- Test: `src/app.rs` (inline tests)

- [ ] **Step 1: Write failing tests for the new state and handlers**

Add these tests to the `#[cfg(test)] mod tests` block in `src/app.rs`:

```rust
#[test]
fn test_toggle_request_detail_on() {
    let mut app = app_with_requests(vec![request("https://example.com")]);

    let command = app.update(Message::ToggleRequestDetail);

    assert!(app.show_request_detail);
    assert!(matches!(command, Command::None));
}

#[test]
fn test_toggle_request_detail_off() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;

    app.update(Message::ToggleRequestDetail);

    assert!(!app.show_request_detail);
}

#[test]
fn test_toggle_request_detail_off_resets_focus() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;
    app.focus = Focus::RequestDetail;

    app.update(Message::ToggleRequestDetail);

    assert_eq!(app.focus, Focus::RequestList);
}

#[test]
fn test_focus_cycles_three_panes_when_detail_open() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;

    app.update(Message::ToggleFocus);
    assert_eq!(app.focus, Focus::RequestDetail);

    app.update(Message::ToggleFocus);
    assert_eq!(app.focus, Focus::ResponsePane);

    app.update(Message::ToggleFocus);
    assert_eq!(app.focus, Focus::RequestList);
}

#[test]
fn test_focus_cycles_two_panes_when_detail_closed() {
    let mut app = app_with_requests(vec![request("https://example.com")]);

    app.update(Message::ToggleFocus);
    assert_eq!(app.focus, Focus::ResponsePane);

    app.update(Message::ToggleFocus);
    assert_eq!(app.focus, Focus::RequestList);
}

#[test]
fn test_detail_scroll_up() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;
    app.focus = Focus::RequestDetail;
    app.detail_scroll_offset = 3;

    app.update(Message::ScrollUp);

    assert_eq!(app.detail_scroll_offset, 2);
}

#[test]
fn test_detail_scroll_down() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_request_detail = true;
    app.focus = Focus::RequestDetail;

    app.update(Message::ScrollDown);

    assert_eq!(app.detail_scroll_offset, 1);
}

#[test]
fn test_detail_scroll_reset_on_selection_change() {
    let mut app = app_with_requests(vec![
        request("https://example.com/one"),
        request("https://example.com/two"),
    ]);
    app.show_request_detail = true;
    app.detail_scroll_offset = 5;

    app.update(Message::SelectNext);

    assert_eq!(app.detail_scroll_offset, 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib`
Expected: Multiple compilation errors (fields/variant don't exist yet)

- [ ] **Step 3: Extend the Focus enum**

In `src/app.rs`, change:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    RequestList,
    RequestDetail,
    ResponsePane,
}
```

- [ ] **Step 4: Add fields to App struct**

Add after `show_help: bool`:

```rust
pub show_request_detail: bool,
pub detail_scroll_offset: usize,
```

- [ ] **Step 5: Initialize new fields in App::new**

In `App::new`, add to the `Self { ... }` initializer after `show_help: false,`:

```rust
show_request_detail: false,
detail_scroll_offset: 0,
```

- [ ] **Step 6: Add ToggleRequestDetail handler**

In `App::update`, add before the `Message::Quit` arm:

```rust
Message::ToggleRequestDetail => {
    self.show_request_detail = !self.show_request_detail;
    if !self.show_request_detail && self.focus == Focus::RequestDetail {
        self.focus = Focus::RequestList;
    }
    Command::None
}
```

- [ ] **Step 7: Add scroll arms for RequestDetail**

In `App::update`, add before the existing `Message::ScrollUp if self.focus == Focus::ResponsePane` arm:

```rust
Message::ScrollUp if self.focus == Focus::RequestDetail => {
    self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(1);
    Command::None
}
Message::ScrollDown if self.focus == Focus::RequestDetail => {
    self.detail_scroll_offset = self.detail_scroll_offset.saturating_add(1);
    Command::None
}
```

- [ ] **Step 8: Update ToggleFocus handler**

Replace the existing `Message::ToggleFocus` arm with:

```rust
Message::ToggleFocus => {
    self.focus = if self.show_request_detail {
        match self.focus {
            Focus::RequestList => Focus::RequestDetail,
            Focus::RequestDetail => Focus::ResponsePane,
            Focus::ResponsePane => Focus::RequestList,
        }
    } else {
        match self.focus {
            Focus::RequestList => Focus::ResponsePane,
            Focus::ResponsePane => Focus::RequestList,
            Focus::RequestDetail => Focus::RequestList,
        }
    };
    Command::None
}
```

- [ ] **Step 9: Reset detail_scroll_offset on selection change**

In the `Message::SelectNext` arm (the one guarded by `self.focus == Focus::RequestList`), add at the end before `Command::None`:

```rust
self.detail_scroll_offset = 0;
```

Do the same in the `Message::SelectPrev` arm (guarded by `self.focus == Focus::RequestList`).

- [ ] **Step 10: Fix compilation errors in existing tests**

The `app_with_requests` helper in `src/app.rs` tests uses `App::new` which now handles the new fields. However, the test helper in `src/ui/request_list.rs` and `src/ui/status_bar.rs` construct `App` directly — they need the new fields. We'll fix those in later tasks. For now, fix the `src/main.rs` `key_message` function to handle the new `Focus::RequestDetail` variant in the match arms at lines 154-161:

```rust
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
```

- [ ] **Step 11: Fix direct App struct construction in test helpers**

In `src/ui/request_list.rs` test helper `app_with_requests` (around line 96), add the new fields:

```rust
show_request_detail: false,
detail_scroll_offset: 0,
```

In `src/ui/status_bar.rs` test helper `app` (around line 78), add the same fields:

```rust
show_request_detail: false,
detail_scroll_offset: 0,
```

- [ ] **Step 12: Run tests to verify they pass**

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 13: Commit**

```bash
git add src/app.rs src/main.rs src/ui/request_list.rs src/ui/status_bar.rs
git commit -m "feat: add request detail state, focus cycling, and scroll handling"
```

---

### Task 3: Create Request Detail Renderer

**Files:**
- Create: `src/ui/request_detail.rs`
- Modify: `src/ui/mod.rs` (register module)

- [ ] **Step 1: Create the module file with render function and tests**

Create `src/ui/request_detail.rs`:

```rust
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Focus};
use crate::parser::ParsedRequest;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border_color = if app.focus == Focus::RequestDetail {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title("Request Detail")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let widget = if app.requests.is_empty() {
        Paragraph::new("No request selected")
            .block(block)
            .alignment(Alignment::Center)
    } else {
        let request = &app.requests[app.selected_index];
        Paragraph::new(format_request(request))
            .block(block)
            .scroll((app.detail_scroll_offset as u16, 0))
    };

    frame.render_widget(widget, area);
}

fn format_request(request: &ParsedRequest) -> String {
    let mut lines = vec![format!("{} {}", request.method, request.url)];

    if !request.headers.is_empty() {
        lines.push(String::new());
        for (name, value) in &request.headers {
            lines.push(format!("{name}: {value}"));
        }
    }

    if let Some(body) = &request.body {
        lines.push(String::new());
        lines.push(body.clone());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, AppStatus};
    use crate::http::AppResponse;
    use crate::parser::{Method, ParsedFile, ParsedRequest};

    fn request(name: Option<&str>, method: Method, url: &str) -> ParsedRequest {
        ParsedRequest {
            name: name.map(str::to_owned),
            method,
            url: url.to_string(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
            body: None,
            source_line: 1,
        }
    }

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
        }
    }

    fn render_app(app: &App) -> TestBackend {
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(app, frame, frame.area()))
            .unwrap();
        terminal.backend().clone()
    }

    fn buffer_text(backend: &TestBackend) -> String {
        let area = backend.buffer().area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| backend.buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_renders_method_and_url() {
        let app = app_with_requests(vec![request(
            Some("Get users"),
            Method::Get,
            "https://example.com/users",
        )]);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("GET https://example.com/users"));
    }

    #[test]
    fn test_renders_headers() {
        let app = app_with_requests(vec![request(
            Some("Get users"),
            Method::Get,
            "https://example.com/users",
        )]);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("Accept: application/json"));
    }

    #[test]
    fn test_renders_body() {
        let mut req = request(Some("Post"), Method::Post, "https://example.com/users");
        req.body = Some("{\"name\": \"test\"}".to_string());
        let app = app_with_requests(vec![req]);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("{\"name\": \"test\"}"));
    }

    #[test]
    fn test_renders_empty_state() {
        let app = app_with_requests(vec![]);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("No request selected"));
    }

    #[test]
    fn test_border_cyan_when_focused() {
        let app = app_with_requests(vec![request(
            Some("Get"),
            Method::Get,
            "https://example.com",
        )]);

        let backend = render_app(&app);
        let cell = &backend.buffer()[(0, 0)];

        assert_eq!(cell.fg, Color::Cyan);
    }

    #[test]
    fn test_border_dark_gray_when_unfocused() {
        let mut app = app_with_requests(vec![request(
            Some("Get"),
            Method::Get,
            "https://example.com",
        )]);
        app.focus = Focus::RequestList;

        let backend = render_app(&app);
        let cell = &backend.buffer()[(0, 0)];

        assert_eq!(cell.fg, Color::DarkGray);
    }

    #[test]
    fn test_no_extra_blank_lines_without_headers_or_body() {
        let req = ParsedRequest {
            name: Some("Bare".to_string()),
            method: Method::Get,
            url: "https://example.com".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };
        let app = app_with_requests(vec![req]);

        let text = format_request(&app.requests[0]);

        assert_eq!(text, "GET https://example.com");
    }
}
```

- [ ] **Step 2: Register the module in `src/ui/mod.rs`**

Add at the top of `src/ui/mod.rs`:

```rust
pub mod request_detail;
```

(After `pub mod help_overlay;`)

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib ui::request_detail`
Expected: All 7 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/ui/request_detail.rs src/ui/mod.rs
git commit -m "feat: add request detail renderer with tests"
```

---

### Task 4: Integrate Layout Split in View

**Files:**
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Write a test for the split layout**

Add to the `#[cfg(test)] mod tests` block in `src/ui/mod.rs`:

```rust
#[test]
fn test_layout_splits_when_detail_panel_open() {
    let mut app = app();
    app.show_request_detail = true;

    let text = render_text(&app);

    assert!(text.contains("Request Detail"));
    assert!(text.contains("Response"));
}

#[test]
fn test_layout_no_detail_panel_when_closed() {
    let app = app();

    let text = render_text(&app);

    assert!(!text.contains("Request Detail"));
    assert!(text.contains("Response"));
}
```

- [ ] **Step 2: Run tests to verify the new ones fail**

Run: `cargo test --lib ui::tests`
Expected: `test_layout_splits_when_detail_panel_open` fails (no "Request Detail" rendered yet)

- [ ] **Step 3: Update the view function for conditional split**

Replace the response pane rendering in the `view` function (`src/ui/mod.rs`). Change:

```rust
request_list::render(app, frame, content_areas[0]);
response_pane::render(app, frame, content_areas[1]);
```

To:

```rust
request_list::render(app, frame, content_areas[0]);

if app.show_request_detail {
    let right_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(content_areas[1]);

    request_detail::render(app, frame, right_areas[0]);
    response_pane::render(app, frame, right_areas[1]);
} else {
    response_pane::render(app, frame, content_areas[1]);
}
```

- [ ] **Step 4: Add `show_request_detail` and `detail_scroll_offset` to the test helper**

In the `app()` function in `src/ui/mod.rs` tests, the helper uses `App::new` which already initializes the new fields to defaults. No changes needed if the test helper uses `App::new`. Verify by checking: `App::new` already sets `show_request_detail: false` and `detail_scroll_offset: 0`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ui::tests`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/ui/mod.rs
git commit -m "feat: integrate request detail panel into layout split"
```

---

### Task 5: Update Status Bar

**Files:**
- Modify: `src/ui/status_bar.rs`

- [ ] **Step 1: Update the KEY_HINTS constant**

In `src/ui/status_bar.rs`, change:

```rust
const KEY_HINTS: &str = "↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit";
```

To:

```rust
const KEY_HINTS: &str = "↑↓ navigate │ Enter send │ Tab focus │ d detail │ ? help │ q quit";
```

- [ ] **Step 2: Update the test assertion**

In `test_renders_keybinding_hints`, change the assertion from:

```rust
assert!(text.contains("↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit"));
```

To:

```rust
assert!(text.contains("↑↓ navigate │ Enter send │ Tab focus │ d detail │ ? help │ q quit"));
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib ui::status_bar`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/ui/status_bar.rs
git commit -m "feat: replace 'r reload' with 'd detail' in status bar hints"
```

---

### Task 6: Add Key Binding

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add the `d` key binding**

In `src/main.rs`, in the `key_message` function's main `match key.code` block, add after the `KeyCode::Char('r')` line:

```rust
KeyCode::Char('d') => Some(Message::ToggleRequestDetail),
```

- [ ] **Step 2: Verify it compiles and runs**

Run: `cargo check`
Expected: Success

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: bind 'd' key to toggle request detail panel"
```

---

### Task 7: Update Help Overlay Text

**Files:**
- Modify: `src/ui/help_overlay.rs`

- [ ] **Step 1: Add `d` to the help text**

In `src/ui/help_overlay.rs`, update the `HELP_TEXT` constant. Add `d         Toggle request detail` to the Actions section:

```rust
const HELP_TEXT: &str = "\
 Navigation
   ↑ / k     Move up / Scroll up
   ↓ / j     Move down / Scroll down
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

- [ ] **Step 2: Run tests**

Run: `cargo test --lib ui::help_overlay`
Expected: All tests pass (they check for "Navigation" and "Enter", not the exact full text)

- [ ] **Step 3: Commit**

```bash
git add src/ui/help_overlay.rs
git commit -m "feat: add 'd' shortcut to help overlay"
```

---

### Task 8: Integration Test and Final Verification

**Files:**
- Test: Full integration

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run clippy for lint checks**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Manual smoke test (optional)**

Run: `cargo run -- examples/httpbin.http` (or any available .http file)
- Press `d` → detail panel appears in top-right
- Press `Tab` → focus cycles through 3 panes (border turns cyan)
- Press `↓`/`↑` while detail panel focused → content scrolls
- Press `d` again → panel hides, focus returns to request list
- Press `?` → help overlay shows `d` shortcut

- [ ] **Step 4: Final commit (if any lint fixes needed)**

```bash
git add -A
git commit -m "fix: address clippy warnings"
```
