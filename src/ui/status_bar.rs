use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;

use crate::app::{App, AppStatus};

const KEY_HINTS: &str = "↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit";

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let right_text = status_text(app);
    let right_width = right_text.chars().count().min(area.width as usize) as u16;

    if right_width == 0 {
        frame.render_widget(Paragraph::new(KEY_HINTS), area);
        return;
    }

    let areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(area);

    frame.render_widget(Paragraph::new(KEY_HINTS), areas[0]);
    frame.render_widget(
        Paragraph::new(right_text).style(status_style(app)),
        areas[1],
    );
}

fn status_text(app: &App) -> String {
    match &app.status {
        AppStatus::Idle => app
            .response
            .as_ref()
            .map(|response| {
                format!(
                    "{}ms {}",
                    response.duration.as_millis(),
                    human_size(response.size_bytes)
                )
            })
            .unwrap_or_default(),
        AppStatus::Sending(started_at) => {
            format!("Sending... {}ms", started_at.elapsed().as_millis())
        }
        AppStatus::Error(message) => message.clone(),
    }
}

fn status_style(app: &App) -> Style {
    match app.status {
        AppStatus::Error(_) => Style::default().fg(Color::Red),
        _ => Style::default(),
    }
}

fn human_size(size_bytes: usize) -> String {
    if size_bytes < 1024 {
        format!("{size_bytes}B")
    } else {
        format!("{:.1}KB", size_bytes as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, Focus};
    use crate::http::AppResponse;
    use crate::parser::ParsedRequest;

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
        }
    }

    fn render_app(app: &App) -> TestBackend {
        let backend = TestBackend::new(80, 1);
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

    fn sample_response(size_bytes: usize, duration_ms: u64) -> AppResponse {
        AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![],
            body: "x".repeat(size_bytes),
            content_type: Some("text/plain".to_string()),
            duration: Duration::from_millis(duration_ms),
            size_bytes,
        }
    }

    #[test]
    fn test_renders_keybinding_hints() {
        let backend = render_app(&app());
        let text = buffer_text(&backend);

        assert!(text.contains("↑↓ navigate │ Enter send │ Tab focus │ r reload │ ? help │ q quit"));
    }

    #[test]
    fn test_renders_duration_on_idle() {
        let mut app = app();
        app.response = Some(sample_response(1229, 120));

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("120ms 1.2KB"));
    }

    #[test]
    fn test_renders_sending_state() {
        let mut app = app();
        app.status = AppStatus::Sending(Instant::now());

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("Sending..."));
    }

    #[test]
    fn test_renders_error_state() {
        let mut app = app();
        app.status = AppStatus::Error("boom".to_string());

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("boom"));
        let has_red_cell = backend
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol().trim() == "b" && cell.fg == Color::Red);
        assert!(has_red_cell);
    }
}
