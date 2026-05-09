# Help Overlay Design

## Overview

Add a keyboard-triggered help overlay that displays all available shortcuts in a centered popup. The overlay is toggled with `?` and dismissed with `?` or `Esc`. While visible, all other keys are swallowed (no pass-through).

## Decisions

- **Trigger key**: `?` (standard TUI convention)
- **Dismiss**: `?` (toggle) or `Esc`
- **Key behavior while open**: All other keys ignored (no pass-through)
- **Content**: Unified list of all shortcuts, grouped by category
- **Discoverability**: Status bar gains `? help` hint

## Architecture

### Approach: Centered overlay via `Clear` + `Paragraph`

Render the help box **after** the main layout in `ui::view()`. Drawing last means it appears on top. Use ratatui's `Clear` widget to blank the underlying area before drawing the help content.

## State Changes

### `src/app.rs`

Add field to `App` struct:

```rust
pub show_help: bool,
```

Initialize to `false` in `App::new()`:

```rust
show_help: false,
```

Add match arm in `update()`:

```rust
Message::ToggleHelp => {
    self.show_help = !self.show_help;
    Command::None
}
```

### `src/message.rs`

Add variant to `Message` enum:

```rust
ToggleHelp,
```

## Key Handling Changes

### `src/main.rs`

The `key_message` function gains awareness of `show_help` state. When help is visible, only `?` and `Esc` produce messages (both map to `ToggleHelp`). All other keys return `None`.

When help is not visible, add `?` to the existing key match:

```rust
KeyCode::Char('?') => Some(Message::ToggleHelp),
```

The `event_message` function signature changes to accept `show_help: bool`:

```rust
fn event_message(event: Event, focus: Focus, show_help: bool) -> Option<Message>
```

Which passes through to `key_message`:

```rust
fn key_message(key: KeyEvent, focus: Focus, show_help: bool) -> Option<Message>
```

### Key routing logic when `show_help == true`:

```rust
if show_help {
    return match key.code {
        KeyCode::Char('?') | KeyCode::Esc => Some(Message::ToggleHelp),
        _ => None,
    };
}
```

This early-return goes at the top of `key_message`, before the existing match.

## New UI Component

### `src/ui/help_overlay.rs`

A single public function:

```rust
pub fn render(frame: &mut Frame)
```

#### Layout calculation

1. Get terminal area from `frame.area()`
2. Calculate overlay dimensions: 60% width, 70% height (clamped to minimum 40 columns × 12 rows)
3. Center the rect within the terminal area

#### Rendering steps

1. `frame.render_widget(Clear, overlay_rect)` — blank underlying content
2. Render a `Block` with `Borders::ALL` and title `" Help (? or Esc to close) "`
3. Inside the block's inner area, render a `Paragraph` with the help content

#### Help content

```text
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
   Ctrl+C    Quit
```

## UI Integration

### `src/ui/mod.rs`

Add module declaration:

```rust
pub mod help_overlay;
```

Add conditional render at end of `view()`:

```rust
if app.show_help {
    help_overlay::render(frame);
}
```

## Status Bar Change

### `src/ui/status_bar.rs`

Update the `KEY_HINTS` constant:

```rust
const KEY_HINTS: &str = "↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit";
```

## Files Touched

| File | Change |
|------|--------|
| `src/app.rs` | Add `show_help: bool` field, handle `ToggleHelp` in `update()` |
| `src/message.rs` | Add `Message::ToggleHelp` variant |
| `src/main.rs` | Add `?` keybinding, pass `show_help` to event handler, early-return when help visible |
| `src/ui/help_overlay.rs` | **New file** — centered overlay rendering |
| `src/ui/mod.rs` | Add `pub mod help_overlay;`, conditional render call |
| `src/ui/status_bar.rs` | Add `? help` to `KEY_HINTS` constant |

## Testing

### Unit tests for `app.rs`

- `test_toggle_help_on`: `ToggleHelp` message sets `show_help = true`
- `test_toggle_help_off`: Second `ToggleHelp` sets `show_help = false`
- `test_toggle_help_returns_none`: Returns `Command::None`

### Unit tests for `main.rs` (key handling)

- `test_question_mark_toggles_help`: `?` key produces `Message::ToggleHelp`
- `test_keys_swallowed_when_help_visible`: Other keys return `None` when `show_help == true`
- `test_esc_closes_help`: `Esc` produces `ToggleHelp` when help visible

### Integration test for UI rendering

- `test_help_overlay_renders_when_visible`: When `show_help == true`, rendered output contains "Help" title and shortcut text
- `test_help_overlay_hidden_by_default`: When `show_help == false`, no help content in rendered output

### Status bar test update

- Update existing `test_layout_has_status_bar` to expect `? help` in hints
