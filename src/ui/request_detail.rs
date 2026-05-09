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

    use crate::app::{App, AppStatus, Focus};
    use crate::http::AppResponse;
    use crate::parser::{Method, ParsedRequest};

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

        assert_eq!(cell.fg, ratatui::style::Color::Cyan);
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

        assert_eq!(cell.fg, ratatui::style::Color::DarkGray);
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
