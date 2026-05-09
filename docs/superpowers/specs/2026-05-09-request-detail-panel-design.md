# Request Detail Panel Design

A togglable panel that displays the full details of the currently selected HTTP request, positioned in the top-right of the interface.

## Overview

When toggled on with `d`, the right 70% area splits horizontally: a 30% request detail pane on top and a 70% response pane below. The panel shows the selected request's method, URL (with unresolved `{{variables}}`), headers, and body. It is scrollable and participates in the existing Tab-based focus cycling.

## Layout

### Panel Closed (default, unchanged)

```
┌──────────────────────────────────────────────────────────────┐
│ restui - file.http                                            │
├──────────────────┬───────────────────────────────────────────┤
│    Requests      │              Response                      │
│  (30% width)     │            (70% width)                    │
├──────────────────┴───────────────────────────────────────────┤
│ status bar                                                    │
└──────────────────────────────────────────────────────────────┘
```

### Panel Open (after pressing `d`)

```
┌──────────────────────────────────────────────────────────────┐
│ restui - file.http                                            │
├──────────────────┬───────────────────────────────────────────┤
│    Requests      │        Request Detail (30% height)         │
│  (30% width)     ├───────────────────────────────────────────┤
│                  │        Response (70% height)               │
├──────────────────┴───────────────────────────────────────────┤
│ status bar                                                    │
└──────────────────────────────────────────────────────────────┘
```

The left request list column remains unchanged at 30% width. The right 70% column splits vertically into 30% detail / 70% response.

## State Changes

### `App` struct (`src/app.rs`)

Add two fields:

- `show_request_detail: bool` — controls panel visibility. Default: `false`.
- `detail_scroll_offset: usize` — scroll position within the detail panel. Default: `0`.

### `Focus` enum (`src/app.rs`)

Add a variant:

```rust
pub enum Focus {
    RequestList,
    RequestDetail,
    ResponsePane,
}
```

### Focus cycling via Tab

When `show_request_detail` is `true`, Tab cycles: `RequestList → RequestDetail → ResponsePane → RequestList`.

When `show_request_detail` is `false`, Tab cycles as today: `RequestList → ResponsePane → RequestList`.

### Scroll reset

When `selected_index` changes (via `SelectNext` or `SelectPrev` while `focus == Focus::RequestList`), reset `detail_scroll_offset` to `0`. This reset is added at the end of the existing `SelectNext` and `SelectPrev` match arms (after updating `selected_index`).

### Panel toggle while focused

If `show_request_detail` is toggled off while `focus == Focus::RequestDetail`, focus falls back to `Focus::RequestList`.

## Messages

### New variant in `Message` enum (`src/message.rs`)

```rust
Message::ToggleRequestDetail
```

No new `Command` variant needed — this is a UI-only state change.

### Handler in `App::update` (`src/app.rs`)

```rust
Message::ToggleRequestDetail => {
    self.show_request_detail = !self.show_request_detail;
    if !self.show_request_detail && self.focus == Focus::RequestDetail {
        self.focus = Focus::RequestList;
    }
    Command::None
}
```

## Scroll Handling

`ScrollUp` and `ScrollDown` messages apply to the detail panel when `focus == Focus::RequestDetail`:

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

These arms are inserted before the existing `Focus::ResponsePane` scroll arms.

## New Component: `src/ui/request_detail.rs`

### Public API

```rust
pub fn render(app: &App, frame: &mut Frame, area: Rect)
```

### Rendering

1. Border color: cyan when `app.focus == Focus::RequestDetail`, dark gray otherwise (same pattern as request list and response pane).
2. Block title: `"Request Detail"`.
3. Content is a single `Paragraph` widget with `scroll((app.detail_scroll_offset as u16, 0))`.

### Content Format

Given the selected `ParsedRequest`:

```
METHOD URL

Header-Name: Header-Value
Header-Name: Header-Value

body text (raw)
```

- Line 1: `"{method} {url}"` — method displayed as uppercase (GET, POST, etc.).
- Line 2: blank separator.
- Lines 3+: one header per line in `"Name: Value"` format.
- If body is present: blank separator, then the raw body text.
- If no headers and no body: just the method + URL line.

### Empty State

When `app.requests.is_empty()`, display: `"No request selected"` (centered, same pattern as response pane empty state).

## Layout Integration (`src/ui/mod.rs`)

When `app.show_request_detail` is `true`, the right content area splits:

```rust
let right_areas = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
    .split(content_areas[1]);

request_detail::render(app, frame, right_areas[0]);
response_pane::render(app, frame, right_areas[1]);
```

When `false`, render response pane at `content_areas[1]` as today.

## Key Bindings (`src/main.rs`)

Add to the key-to-message mapping, guarded by the existing `!app.show_help` condition (same as other non-help keys):

- `KeyCode::Char('d')` → `Message::ToggleRequestDetail`

When `app.show_help` is `true`, `d` is swallowed (no action), consistent with existing key handling.

## Status Bar (`src/ui/status_bar.rs`)

Add `d detail` to the keybinding hints displayed in the status bar.

## Focus Cycling Update

The `ToggleFocus` handler changes from a 2-state toggle to conditional cycling:

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

## Edge Cases

1. **No requests loaded**: Detail panel shows "No request selected" centered.
2. **Panel toggled off while focused**: Focus falls back to `RequestList`.
3. **Request has no headers or body**: Only `"METHOD URL"` displayed — no extra blank lines.
4. **Request has headers but no body**: Method+URL, blank, headers. No trailing blank line.
5. **Scroll beyond content**: `saturating_add` means scroll stops at content end (ratatui handles this naturally with `Paragraph::scroll`).

## Testing

Each component should have unit tests following existing patterns:

- `app.rs`: Test `ToggleRequestDetail` message toggles state, test focus fallback on close, test focus cycling with 3 panes, test `detail_scroll_offset` reset on selection change.
- `ui/request_detail.rs`: Test renders method+url, headers, body. Test empty state. Test border color matches focus.
- `ui/mod.rs`: Test layout splits correctly when panel open vs closed.

## Files Modified

- `src/app.rs` — Add fields, extend Focus enum, add message handler, update ToggleFocus
- `src/message.rs` — Add `ToggleRequestDetail` variant
- `src/ui/mod.rs` — Conditional layout split, register new module
- `src/ui/request_detail.rs` — New file
- `src/ui/status_bar.rs` — Add `d detail` hint
- `src/main.rs` — Add `d` key binding
