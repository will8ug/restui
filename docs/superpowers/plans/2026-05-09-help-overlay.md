# Help Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `?`-toggled help overlay that displays all keyboard shortcuts in a centered popup.

**Architecture:** New `show_help: bool` state field toggled via `Message::ToggleHelp`. When visible, a centered overlay renders on top of the main UI and all keys except `?`/`Esc` are swallowed.

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.28

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/message.rs` | Add `ToggleHelp` variant to `Message` enum |
| `src/app.rs` | Add `show_help` field, handle `ToggleHelp` message |
| `src/main.rs` | Wire `?` key, pass `show_help` to event handler, swallow keys when help visible |
| `src/ui/help_overlay.rs` | **New** — render centered overlay with shortcut content |
| `src/ui/mod.rs` | Register module, conditional render |
| `src/ui/status_bar.rs` | Add `? help` to hints |

---

### Task 1: Add `ToggleHelp` message variant

**Files:**
- Modify: `src/message.rs`
- Test: `src/app.rs` (tests added in Task 2)

- [ ] **Step 1: Add the variant**

In `src/message.rs`, add `ToggleHelp` to the `Message` enum:

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
    Quit,
    Resize(u16, u16),
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: Warning about unused variant `ToggleHelp` (not yet handled). No errors.

- [ ] **Step 3: Commit**

```bash
git add src/message.rs
git commit -m "feat: add ToggleHelp message variant"
```

---

### Task 2: Add `show_help` state and handle `ToggleHelp` in App

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests at the end of the `mod tests` block in `src/app.rs`:

```rust
#[test]
fn test_toggle_help_on() {
    let mut app = app_with_requests(vec![request("https://example.com")]);

    app.update(Message::ToggleHelp);

    assert!(app.show_help);
}

#[test]
fn test_toggle_help_off() {
    let mut app = app_with_requests(vec![request("https://example.com")]);
    app.show_help = true;

    app.update(Message::ToggleHelp);

    assert!(!app.show_help);
}

#[test]
fn test_toggle_help_returns_none() {
    let mut app = app_with_requests(vec![request("https://example.com")]);

    let command = app.update(Message::ToggleHelp);

    assert!(matches!(command, Command::None));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::tests::test_toggle_help`
Expected: Compile error — `show_help` field doesn't exist, `ToggleHelp` not handled.

- [ ] **Step 3: Add `show_help` field to `App` struct**

In `src/app.rs`, add the field to the `App` struct:

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
}
```

- [ ] **Step 4: Initialize in `App::new()`**

In the `App::new()` function, add `show_help: false` to the `Self` initialization:

```rust
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
    }
}
```

- [ ] **Step 5: Handle `ToggleHelp` in `update()`**

Add a new match arm in the `update()` method, before the final catch-all arm:

```rust
Message::ToggleHelp => {
    self.show_help = !self.show_help;
    Command::None
}
```

- [ ] **Step 6: Fix test helpers that construct `App` directly**

Multiple test files create `App` structs directly without `App::new()`. Add `show_help: false` to each. These are in:

In `src/app.rs` test helper `app_with_requests`:
```rust
fn app_with_requests(requests: Vec<ParsedRequest>) -> App {
    App::new(
        PathBuf::from("requests.http"),
        parsed_file(requests, vec![]),
    )
}
```
This one uses `App::new()` so it's already fine.

In `src/ui/request_list.rs` test helper `app_with_requests`:
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
    }
}
```

In `src/ui/response_pane.rs` test helper `app_with_response`:
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
    }
}
```

In `src/ui/status_bar.rs` test helper `app`:
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
    }
}
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass including the 3 new ones.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs src/ui/request_list.rs src/ui/response_pane.rs src/ui/status_bar.rs
git commit -m "feat: add show_help state and ToggleHelp handler"
```

---

### Task 3: Wire `?` key and help-mode key swallowing in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update `event_message` signature**

Change the function signature and its call to `key_message`:

```rust
fn event_message(event: Event, focus: Focus, show_help: bool) -> Option<Message> {
    match event {
        Event::Key(key) => key_message(key, focus, show_help),
        Event::Resize(width, height) => Some(Message::Resize(width, height)),
        _ => None,
    }
}
```

- [ ] **Step 2: Update `key_message` with help-mode early return and `?` binding**

```rust
fn key_message(key: KeyEvent, focus: Focus, show_help: bool) -> Option<Message> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    if show_help {
        return match key.code {
            KeyCode::Char('?') | KeyCode::Esc => Some(Message::ToggleHelp),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(match focus {
            Focus::RequestList => Message::SelectPrev,
            Focus::ResponsePane => Message::ScrollUp,
        }),
        KeyCode::Down | KeyCode::Char('j') => Some(match focus {
            Focus::RequestList => Message::SelectNext,
            Focus::ResponsePane => Message::ScrollDown,
        }),
        KeyCode::Enter => Some(Message::SendRequest),
        KeyCode::Tab => Some(Message::ToggleFocus),
        KeyCode::Char('r') => Some(Message::ReloadFile),
        KeyCode::Char('?') => Some(Message::ToggleHelp),
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}
```

- [ ] **Step 3: Update the call site in the event loop**

In the `loop` block, update the `event_message` call to pass `app.show_help`:

```rust
if event::poll(Duration::from_millis(50))?
    && let Some(message) = event_message(event::read()?, app.focus, app.show_help)
{
    pending_messages.push(message);
}
```

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cargo test`
Expected: All tests pass. No compile errors.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire ? key binding and help-mode key swallowing"
```

---

### Task 4: Create help overlay UI component

**Files:**
- Create: `src/ui/help_overlay.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/help_overlay.rs` with render function**

```rust
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

const HELP_TEXT: &str = "\
 Navigation
   ↑ / k     Move up / Scroll up
   ↓ / j     Move down / Scroll down
   Tab       Toggle focus between panes

 Actions
   Enter     Send selected request
   r         Reload file from disk

 Application
   ?         Toggle this help
   q         Quit
   Ctrl+C    Quit";

pub fn render(frame: &mut Frame) {
    let area = frame.area();
    let overlay = centered_rect(area);

    frame.render_widget(Clear, overlay);
    let block = Block::default()
        .title(" Help (? or Esc to close) ")
        .borders(Borders::ALL);
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);
    frame.render_widget(Paragraph::new(HELP_TEXT), inner);
}

fn centered_rect(area: Rect) -> Rect {
    let width = (area.width * 60 / 100).max(40).min(area.width);
    let height = (area.height * 70 / 100).max(12).min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_help_overlay_renders_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .flat_map(|y| {
                (0..buffer.area().width)
                    .map(move |x| buffer[(x, y)].symbol().to_string())
            })
            .collect();

        assert!(text.contains("Help (? or Esc to close)"));
    }

    #[test]
    fn test_help_overlay_renders_shortcuts() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .map(|y| {
                (0..buffer.area().width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Navigation"));
        assert!(text.contains("Enter"));
        assert!(text.contains("Quit"));
    }

    #[test]
    fn test_centered_rect_respects_minimum() {
        let area = Rect::new(0, 0, 30, 8);
        let rect = centered_rect(area);

        // Width clamped to area.width (30) since min(40) > area width
        assert_eq!(rect.width, 30);
        // Height clamped to area.height (8) since min(12) > area height
        assert_eq!(rect.height, 8);
    }

    #[test]
    fn test_centered_rect_centers_properly() {
        let area = Rect::new(0, 0, 100, 40);
        let rect = centered_rect(area);

        assert_eq!(rect.width, 60); // 60% of 100
        assert_eq!(rect.height, 28); // 70% of 40
        assert_eq!(rect.x, 20); // (100 - 60) / 2
        assert_eq!(rect.y, 6); // (40 - 28) / 2
    }
}
```

- [ ] **Step 2: Register module in `src/ui/mod.rs`**

Add `pub mod help_overlay;` after the existing module declarations:

```rust
pub mod help_overlay;
pub mod request_list;
pub mod response_pane;
pub mod status_bar;
```

- [ ] **Step 3: Add conditional render in `view()` function**

At the end of `view()` in `src/ui/mod.rs`, before the closing brace, add:

```rust
if app.show_help {
    help_overlay::render(frame);
}
```

The full `view()` function becomes:

```rust
pub fn view(app: &App, frame: &mut Frame) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let content_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(areas[1]);

    let filename = app
        .file_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| app.file_path.display().to_string());

    frame.render_widget(Paragraph::new(format!("restui - {filename}")), areas[0]);
    request_list::render(app, frame, content_areas[0]);
    response_pane::render(app, frame, content_areas[1]);
    status_bar::render(app, frame, areas[2]);

    if app.show_help {
        help_overlay::render(frame);
    }
}
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test`
Expected: All tests pass including the new `help_overlay` tests.

- [ ] **Step 5: Commit**

```bash
git add src/ui/help_overlay.rs src/ui/mod.rs
git commit -m "feat: add help overlay UI component with centered rendering"
```

---

### Task 5: Add integration test for help overlay visibility

**Files:**
- Modify: `src/ui/mod.rs` (test section)

- [ ] **Step 1: Write the integration tests**

Add these tests to the `mod tests` block in `src/ui/mod.rs`:

```rust
#[test]
fn test_help_overlay_renders_when_visible() {
    let mut app = app();
    app.show_help = true;

    let text = render_text(&app);

    assert!(text.contains("Help (? or Esc to close)"));
    assert!(text.contains("Navigation"));
}

#[test]
fn test_help_overlay_hidden_by_default() {
    let app = app();

    let text = render_text(&app);

    assert!(!text.contains("Help (? or Esc to close)"));
}
```

- [ ] **Step 2: Fix the `app()` helper in `src/ui/mod.rs` tests**

The existing `app()` function uses `App::new()` which already initializes `show_help: false`, so no change needed here.

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test ui::tests`
Expected: All tests pass including the 2 new ones.

- [ ] **Step 4: Commit**

```bash
git add src/ui/mod.rs
git commit -m "test: add integration tests for help overlay visibility"
```

---

### Task 6: Update status bar with `? help` hint

**Files:**
- Modify: `src/ui/status_bar.rs`

- [ ] **Step 1: Update the existing test expectation**

In `src/ui/status_bar.rs`, update the `test_renders_keybinding_hints` test:

```rust
#[test]
fn test_renders_keybinding_hints() {
    let backend = render_app(&app());
    let text = buffer_text(&backend);

    assert!(text.contains("↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test status_bar::tests::test_renders_keybinding_hints`
Expected: FAIL — current hints don't contain `? help`.

- [ ] **Step 3: Update `KEY_HINTS` constant**

```rust
const KEY_HINTS: &str = "↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit";
```

- [ ] **Step 4: Update the integration test in `src/ui/mod.rs`**

In `src/ui/mod.rs`, update `test_layout_has_status_bar`:

```rust
#[test]
fn test_layout_has_status_bar() {
    let text = render_text(&app());

    assert!(text.contains("Enter send"));
    assert!(text.contains("Tab focus"));
    assert!(text.contains("? help"));
    assert!(text.contains("q quit"));
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/status_bar.rs src/ui/mod.rs
git commit -m "feat: add '? help' hint to status bar"
```

---

### Task 7: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings or errors.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt -- --check`
Expected: No formatting issues.

- [ ] **Step 4: Build release**

Run: `cargo build --release`
Expected: Successful build with no errors.
