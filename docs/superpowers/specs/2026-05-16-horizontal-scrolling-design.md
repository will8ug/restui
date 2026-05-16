# Horizontal Scrolling for All Three Panels

Add horizontal scrolling to the Request List, Request Detail, and Response panels, mirroring the existing vertical-scroll model.

## Overview

The focused panel responds to `←`/`h` (scroll left) and `→`/`l` (scroll right), moving one column per keypress. Each panel keeps its own horizontal offset. Offsets persist across focus changes and reset to `0` when the panel's content changes (selection change for the List and Detail panels, new response for the Response panel). Scroll is unbounded — the same `saturating_add`/`saturating_sub` behavior as today's vertical scroll, so the user can over-scroll past the longest line into empty space.

No new visible UI elements are added. The status-bar hint and help overlay are updated to document the new keys.

## Design decisions

These were resolved during brainstorming:

1. **Horizontal scrolling, not word-wrap.** A dedicated wrap toggle was considered and deferred; horizontal scroll is the literal feature and mirrors existing behavior.
2. **Keybindings: `←`/`h` and `→`/`l`.** Mirrors the existing `↑`/`k` and `↓`/`j` pairing. Both `h` and `l` are currently unbound (confirmed against `main.rs::key_message`); no conflicts.
3. **Auto-reset on content change.** Matches the existing rule that vertical offsets reset when content changes (see `App::update` `SelectNext`/`SelectPrev` resetting `detail_scroll_offset`, and `ResponseReceived` resetting `scroll_offset`).
4. **Unbounded scroll.** `saturating_sub(1)` / `saturating_add(1)`. Matches vertical scroll exactly. No content-width clamp.
5. **Persist across focus changes.** Tab does not touch any scroll offsets. Matches the existing rule for vertical scroll.
6. **One column per keypress.** Matches vertical scroll exactly. Autorepeat handles long traversals.

## State changes

### `App` struct (`src/app.rs`)

Add three new fields, one per panel:

```rust
pub struct App {
    // ... existing fields ...
    pub scroll_offset: usize,           // existing: response, vertical
    pub detail_scroll_offset: usize,    // existing: detail, vertical
    pub list_scroll_offset_x: usize,    // NEW: request list, horizontal
    pub detail_scroll_offset_x: usize,  // NEW: request detail, horizontal
    pub scroll_offset_x: usize,         // NEW: response, horizontal
}
```

All three default to `0` in `App::new`.

Naming: keep existing vertical fields untouched and suffix new horizontal fields with `_x`. The existing inconsistency (`scroll_offset` for response vs `detail_scroll_offset` for detail) is preserved; renaming is out of scope.

### Reset rules

| Event | Existing reset | New reset added by this change |
|---|---|---|
| `SelectNext` / `SelectPrev` (focus = `RequestList`) | `detail_scroll_offset = 0` | `list_scroll_offset_x = 0`, `detail_scroll_offset_x = 0` |
| `ResponseReceived` | `scroll_offset = 0` | `scroll_offset_x = 0` |
| `ToggleFocus` (Tab) | none | none (offsets persist) |
| `ToggleRequestDetail` off | focus fallback to `RequestList` | none |
| `ReloadFile` | none | none |

Rule summary: **whenever a panel's content changes, its horizontal offset resets to `0`**, parallel to the existing vertical-offset behavior.

## Messages

### `Message` enum (`src/message.rs`)

Add two variants alongside `ScrollUp`/`ScrollDown`:

```rust
pub enum Message {
    // ... existing ...
    ScrollUp,
    ScrollDown,
    ScrollLeft,   // NEW
    ScrollRight,  // NEW
    // ... existing ...
}
```

No new `Command` variants — horizontal scroll is pure state, no I/O.

### `App::update` match arms (`src/app.rs`)

Add six new arms, one per (panel × direction), mirroring the existing vertical arms:

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

Extend the existing exhaustive-fallback arm to include the new variants:

```rust
Message::SelectNext
| Message::SelectPrev
| Message::ScrollUp
| Message::ScrollDown
| Message::ScrollLeft
| Message::ScrollRight => Command::None,
```

This arm is reachable only when no focus-guarded arm matches (defensive — all three `Focus` variants are covered today).

## Key handling

### `main.rs::key_message`

Add two focus-independent arms after the existing `Up`/`Down` arms:

```rust
KeyCode::Left | KeyCode::Char('h') => Some(Message::ScrollLeft),
KeyCode::Right | KeyCode::Char('l') => Some(Message::ScrollRight),
```

Unlike `Up`/`Down` (which fork based on focus to either `Select*` or `Scroll*`), horizontal keys map to the same `Scroll*` message for every focus state — there is no "select left" semantic, only horizontal scroll.

### Help-mode behavior

Unchanged. The early return in `key_message` for `show_help` only allows `?` and `Esc`, so horizontal scroll keys are correctly ignored while the help overlay is up — same as the existing vertical scroll keys.

## Rendering

### Request Detail (`src/ui/request_detail.rs`)

`Paragraph::scroll` natively accepts an `(y, x)` tuple. Change the existing `.scroll((app.detail_scroll_offset as u16, 0))` to:

```rust
.scroll((
    app.detail_scroll_offset as u16,
    app.detail_scroll_offset_x as u16,
))
```

One-line change. No other rendering logic affected.

### Response Pane (`src/ui/response_pane.rs`)

Same pattern as Request Detail:

```rust
.scroll((
    app.scroll_offset as u16,
    app.scroll_offset_x as u16,
))
```

One-line change.

### Request List (`src/ui/request_list.rs`)

`ratatui::List` has no native horizontal scroll. We slice each item's display string by the offset before wrapping it in `ListItem`.

Update the item-construction block to apply the offset to the composed line:

```rust
let items = app
    .requests
    .iter()
    .enumerate()
    .map(|(index, request)| {
        let selected_prefix = if index == app.selected_index { ">" } else { " " };
        let sent_prefix = if app.last_sent_index == Some(index) { "●" } else { " " };
        let label = request_label(request);
        let full = format!("{selected_prefix}{sent_prefix} {label}");
        let visible = horizontal_slice(&full, app.list_scroll_offset_x);
        ListItem::new(visible)
    })
    .collect::<Vec<_>>();
```

Add a private helper in the same file:

```rust
fn horizontal_slice(line: &str, offset: usize) -> String {
    line.chars().skip(offset).collect()
}
```

Behavior characteristics:

- **Per-character offset.** Matches `Paragraph::scroll`'s character-based behavior used by the other two panels. Wide characters (CJK) shift by their column width — same caveat as existing vertical scroll; not a regression.
- **`>` and `●` prefixes scroll off with the rest of the text.** Expected behavior for horizontal scroll.
- **`ListState` selection still works correctly.** Selection is by index, not visual content. The full-row background highlight from `highlight_style` continues to span the panel width regardless of how much of the label is visible.
- **Over-scrolled items render blank.** `chars().skip(offset).collect()` returns `""` when offset exceeds line length; `ListItem::new("")` renders as a blank row. Matches the "unbounded scroll into empty space" decision.

### Status bar (`src/ui/status_bar.rs`)

Update the existing `KEY_HINTS` constant to include horizontal navigation in the existing `Nav` slot:

```rust
const KEY_HINTS: &str =
    "[↑↓←→] Nav │ [Enter] Send │ [Tab] Focus │ [d] Detail │ [?] Help │ [q] Quit";
```

Two extra characters; fits within a typical 80-column terminal alongside response metadata on the right.

### Help overlay (`src/ui/help_overlay.rs`)

Update `HELP_TEXT` to document the new keys under `Navigation`:

```text
 Navigation
   ↑ / k     Move up / Scroll up
   ↓ / j     Move down / Scroll down
   ← / h     Scroll left
   → / l     Scroll right
   Tab       Toggle focus between panes
```

### README (`README.md`)

Extend the keybindings table:

```markdown
| ← / h | Scroll left |
| → / l | Scroll right |
```

## Test plan

The project has strong testing discipline. Every existing scroll behavior has a unit test. New scroll behavior gets matching tests in the same style.

### `src/app.rs::tests`

Mirror the existing vertical-scroll tests for the new horizontal axis:

- `test_scroll_left_response` — focus = `ResponsePane`, `scroll_offset_x = 3`, send `ScrollLeft`, expect `scroll_offset_x == 2`.
- `test_scroll_right_response` — focus = `ResponsePane`, send `ScrollRight` from `0`, expect `scroll_offset_x == 1`.
- `test_scroll_left_at_zero_response` — saturating sub keeps offset at `0`.
- `test_scroll_left_detail` / `test_scroll_right_detail` — parallel to above for `Focus::RequestDetail` and `detail_scroll_offset_x`.
- `test_scroll_left_list` / `test_scroll_right_list` — parallel to above for `Focus::RequestList` and `list_scroll_offset_x`.
- `test_list_horizontal_reset_on_selection_change` — set `list_scroll_offset_x = 5`, send `SelectNext`, expect `0`.
- `test_detail_horizontal_reset_on_selection_change` — set `detail_scroll_offset_x = 5`, send `SelectNext`, expect `0`.
- `test_response_horizontal_reset_on_response_received` — set `scroll_offset_x = 5`, send `ResponseReceived`, expect `0`.
- `test_scroll_horizontal_persists_across_focus_change` — set `show_request_detail = true` (so Tab cycles through all three focuses), set all three x offsets to nonzero values, cycle focus via repeated `ToggleFocus` back to the starting focus, verify all three x offsets are unchanged.

### `src/ui/request_list.rs::tests`

The slicing helper is custom code, so it gets direct coverage:

- `test_horizontal_slice_basic` — `horizontal_slice(">  List users", 3)` returns `"List users"`.
- `test_horizontal_slice_past_end` — offset larger than length returns `""`.
- `test_horizontal_slice_zero_offset` — returns the original string unchanged.
- `test_renders_list_with_horizontal_offset` — render with `list_scroll_offset_x = 4`, assert the buffer shows the sliced label and the leading prefix characters are not present.

### `src/ui/request_detail.rs::tests`

- `test_renders_with_horizontal_offset` — set `detail_scroll_offset_x = 5`, render, assert the leftmost characters of the URL line in the buffer are not the start of the URL (i.e., the offset is applied).

### `src/ui/response_pane.rs::tests`

- `test_renders_with_horizontal_offset` — same pattern for the response, asserting offset is applied.

### `src/ui/status_bar.rs::tests`

- Update `test_renders_keybinding_hints` to assert the new literal: `"[↑↓←→] Nav │ [Enter] Send │ [Tab] Focus │ [d] Detail │ [?] Help │ [q] Quit"`.

### `src/ui/help_overlay.rs::tests`

- Extend `test_help_overlay_renders_shortcuts` to also assert presence of `Scroll left` and `Scroll right`.

### `src/ui/mod.rs::tests`

- Existing layout tests continue to pass unchanged. No new tests required at this level.

### Test helper updates (mechanical)

Four test modules construct `App` via struct literal and must add the three new fields (each defaulting to `0`):

- `src/ui/request_list.rs::app_with_requests`
- `src/ui/response_pane.rs::app_with_response`
- `src/ui/request_detail.rs::app_with_requests`
- `src/ui/status_bar.rs::app`

Without this update the code will not compile.

### Manual verification

After implementation, smoke-test in a real terminal with an `.http` file containing long URLs (>120 chars) and a request returning a wide JSON response. Verify:

- Each panel scrolls independently with `←`/`→`/`h`/`l` when focused.
- `j`/`k` selection-change in the List resets both `list_scroll_offset_x` and `detail_scroll_offset_x` to `0`.
- Receiving a new response (a successful `ResponseReceived` event) resets `scroll_offset_x` to `0`. A request that errors out does not reset, matching how `scroll_offset` behaves today.
- `Tab` cycling does not alter any offset.
- The help overlay and status bar reflect the new keys.
