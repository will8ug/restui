use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, AppStatus, Focus};
use crate::content::format_response;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border_color = if app.focus == Focus::ResponsePane {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let (title, widget) = match &app.status {
        AppStatus::Error(message) => {
            let text = Paragraph::new(message.clone())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false })
                .scroll((app.scroll_offset as u16, 0));
            ("Error", text)
        }
        _ => match &app.response {
            Some(response) => {
                let text = format_response(response);
                let widget = Paragraph::new(text)
                    .scroll((app.scroll_offset as u16, app.scroll_offset_x as u16));
                ("Response", widget)
            }
            None => (
                "Response",
                Paragraph::new("No response yet. Select a request and press Enter.")
                    .alignment(Alignment::Center),
            ),
        },
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    frame.render_widget(widget.block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, AppStatus};
    use crate::http::AppResponse;
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
            fullscreen: false,
            show_request_detail: false,
            detail_scroll_offset: 0,
            list_scroll_offset_x: 0,
            detail_scroll_offset_x: 0,
            scroll_offset_x: 0,
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
    fn test_renders_with_horizontal_offset() {
        let mut app = app_with_response(Some(sample_response("hello-world", Some("text/plain"))));
        app.scroll_offset_x = 4;

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        // The "HTTP 200 OK" status line is clipped by 4 chars; "200 OK" survives, "HTTP" does not.
        assert!(!text.contains("HTTP 200 OK"));
        assert!(text.contains("200 OK"));
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

    #[test]
    fn test_error_state_renders_full_message_wrapped() {
        let message = concat!(
            "request failed: error sending request for url (https://internal.example.com): ",
            "client error (Connect): invalid peer certificate: ",
            "Other(OtherError(\"internal.example.com certificate is not standards ",
            "compliant\")): -67901"
        );
        let mut app = app_with_response(None);
        app.status = AppStatus::Error(message.to_string());

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("Error"));
        assert!(text.contains("-67901"));
        assert!(text.contains("not standards compliant"));
    }

    #[test]
    fn test_error_state_overrides_stale_response() {
        let mut app = app_with_response(Some(sample_response("hello", Some("text/plain"))));
        app.status = AppStatus::Error("request failed: connection reset".to_string());

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("request failed: connection reset"));
        assert!(!text.contains("HTTP 200 OK"));
        assert!(!text.contains("hello"));
    }

    #[test]
    fn test_error_state_scrolls_to_reveal_later_lines() {
        let message = format!(
            "line one {}\nline two tail-marker-{}",
            "x".repeat(200),
            "OSStatus-67901"
        );
        let mut app = app_with_response(None);
        app.status = AppStatus::Error(message);
        app.scroll_offset = 5;

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("OSStatus-67901"));
    }
}
