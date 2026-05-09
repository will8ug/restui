use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use serde_json::Value;

use crate::app::{App, Focus};
use crate::http::AppResponse;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border_color = if app.focus == Focus::ResponsePane {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title("Response")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let widget = match &app.response {
        Some(response) => Paragraph::new(format_response(response))
            .block(block)
            .scroll((app.scroll_offset as u16, 0)),
        None => Paragraph::new("No response yet. Select a request and press Enter.")
            .block(block)
            .alignment(Alignment::Center),
    };

    frame.render_widget(widget, area);
}

fn format_response(response: &AppResponse) -> String {
    let mut lines = vec![format!("HTTP {} {}", response.status, response.status_text)];

    for (name, value) in &response.headers {
        lines.push(format!("{name}: {value}"));
    }

    lines.push(String::new());
    lines.push(format_body(response));
    lines.join("\n")
}

fn format_body(response: &AppResponse) -> String {
    let is_json = response
        .content_type
        .as_deref()
        .map(|content_type| content_type.to_ascii_lowercase().contains("json"))
        .unwrap_or(false);

    if !is_json {
        return response.body.clone();
    }

    match serde_json::from_str::<Value>(&response.body)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
    {
        Some(pretty) => pretty,
        None => response.body.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, AppStatus};
    use crate::parser::ParsedRequest;

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

    fn sample_response(body: &str, content_type: Option<&str>) -> AppResponse {
        AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![(
                "content-type".to_string(),
                content_type.unwrap_or("text/plain").to_string(),
            )],
            body: body.to_string(),
            content_type: content_type.map(str::to_owned),
            duration: Duration::from_millis(120),
            size_bytes: body.len(),
        }
    }

    fn render_app(app: &App) -> TestBackend {
        let backend = TestBackend::new(50, 10);
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
    fn test_renders_empty_state() {
        let app = app_with_response(None);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("No response yet."));
        assert!(text.contains("Response"));
    }

    #[test]
    fn test_renders_response() {
        let app = app_with_response(Some(sample_response("hello", Some("text/plain"))));

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("HTTP 200 OK"));
        assert!(text.contains("content-type: text/plain"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn test_renders_json_pretty_printed() {
        let app = app_with_response(Some(sample_response(
            r#"{"user":{"name":"alice"}}"#,
            Some("application/json"),
        )));

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("\"user\": {"));
        assert!(text.contains("\"name\": \"alice\""));
    }
}
