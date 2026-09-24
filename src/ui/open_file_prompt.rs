use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::content;

pub fn render(frame: &mut Frame, input: &str, error: Option<&str>) {
    let area = frame.area();
    let overlay = centered_rect(area);

    frame.render_widget(Clear, overlay);
    let block = Block::default().title(" Open file ").borders(Borders::ALL);
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);

    let [input_area, error_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .areas(inner);

    frame.render_widget(Paragraph::new(input), input_area);
    if let Some(message) = error {
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(Color::Red)),
            error_area,
        );
    }

    let input_width =
        content::max_line_width(input).min(usize::from(inner.width.saturating_sub(1))) as u16;
    frame.set_cursor_position(Position::new(inner.x + input_width, inner.y));
}

fn centered_rect(area: Rect) -> Rect {
    let width = (area.width * 60 / 100).max(40).min(area.width);
    let height = 4.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::style::Color;

    fn render_prompt(input: &str, error: Option<&str>) -> TestBackend {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, input, error)).unwrap();
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
    fn test_open_file_prompt_renders_title() {
        let backend = render_prompt("./api.http", None);
        let text = buffer_text(&backend);

        assert!(text.contains("Open file"));
    }

    #[test]
    fn test_open_file_prompt_renders_input_path() {
        let backend = render_prompt("./api.http", None);
        let text = buffer_text(&backend);

        assert!(text.contains("./api.http"));
    }

    #[test]
    fn test_open_file_prompt_renders_error_message() {
        let backend = render_prompt("./missing.http", Some("Failed to read ./missing.http"));
        let text = buffer_text(&backend);

        assert!(text.contains("Failed to read ./missing.http"));
    }

    #[test]
    fn test_open_file_prompt_renders_error_in_red() {
        let backend = render_prompt("./missing.http", Some("boom"));
        let text = buffer_text(&backend);

        assert!(text.contains("boom"));
        let has_red_cell = backend
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol().trim() == "b" && cell.fg == Color::Red);
        assert!(has_red_cell);
    }

    #[test]
    fn test_open_file_prompt_places_cursor_after_input() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, "api.http", None))
            .unwrap();

        terminal
            .backend_mut()
            .assert_cursor_position(Position::new(25, 11));
    }

    #[test]
    fn test_centered_rect_is_small_and_centered() {
        let area = Rect::new(0, 0, 100, 40);
        let rect = centered_rect(area);

        assert_eq!(rect.width, 60); // 60% of 100
        assert_eq!(rect.height, 4);
        assert_eq!(rect.x, 20); // (100 - 60) / 2
        assert_eq!(rect.y, 18); // (40 - 4) / 2
    }

    #[test]
    fn test_centered_rect_clamps_to_area() {
        let area = Rect::new(0, 0, 30, 3);
        let rect = centered_rect(area);

        assert_eq!(rect.width, 30);
        assert_eq!(rect.height, 3);
    }
}
