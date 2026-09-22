use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

const HELP_TEXT: &str = "\
 Navigation
   ↑ / k     Move up / Scroll up
   ↓ / j     Move down / Scroll down
   ← / h     Scroll left
   → / l     Scroll right
   g / G     Jump to start / end vertically
   0 / $ / Home / End  Jump to start / end horizontally
   Tab       Toggle focus between panes

 Actions
   Enter     Send selected request
   d         Toggle request detail
   f         Toggle fullscreen of focused pane
   Esc       Exit fullscreen
   R         Reload file from disk

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
    let width = (area.width * 60 / 100).max(50).min(area.width);
    let height = (area.height * 70 / 100).max(24).min(area.height);
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
        terminal.draw(render).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .map(|y| {
                (0..buffer.area().width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Help (? or Esc to close)"));
    }

    #[test]
    fn test_help_overlay_renders_shortcuts() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(render).unwrap();
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
        assert!(text.contains("Scroll left"));
        assert!(text.contains("Scroll right"));
    }

    #[test]
    fn test_help_overlay_documents_jump_keys() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(render).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .map(|y| {
                (0..buffer.area().width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("g / G"));
        assert!(text.contains("Jump to start / end vertically"));
    }

    #[test]
    fn test_help_overlay_documents_horizontal_jump_keys() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(render).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .map(|y| {
                (0..buffer.area().width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("0 / $ / Home / End"));
        assert!(text.contains("Jump to start / end horizontally"));
    }

    #[test]
    fn test_help_overlay_documents_fullscreen_keys() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(render).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .map(|y| {
                (0..buffer.area().width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Toggle fullscreen of focused pane"));
        assert!(text.contains("Exit fullscreen"));
    }

    #[test]
    fn test_centered_rect_respects_minimum() {
        let area = Rect::new(0, 0, 30, 8);
        let rect = centered_rect(area);

        // Width clamped to area.width (30) since min(40) > area width
        assert_eq!(rect.width, 30);
        // Height clamped to area.height (8) since min(20) > area height
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
