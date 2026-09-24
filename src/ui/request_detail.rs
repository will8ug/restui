use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Focus};
use crate::content::format_request_detail;

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
        let text = format_request_detail(&app.variables, &app.requests[app.selected_index]);
        Paragraph::new(text).block(block).scroll((
            app.detail_scroll_offset as u16,
            app.detail_scroll_offset_x as u16,
        ))
    };

    frame.render_widget(widget, area);
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
            open_file_prompt: None,
            open_file_error: None,
            fullscreen: false,
            show_request_detail: true,
            detail_scroll_offset: 0,
            list_scroll_offset_x: 0,
            detail_scroll_offset_x: 0,
            scroll_offset_x: 0,
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
    fn test_renders_resolved_url_and_headers() {
        let mut app = app_with_requests(vec![request(Some("Get"), Method::Get, "{{host}}/get")]);
        app.requests[0].headers = vec![("Accept".to_string(), "{{content_type}}".to_string())];
        app.variables = vec![
            crate::parser::Variable {
                name: "host".to_string(),
                value: "https://httpbin.org".to_string(),
            },
            crate::parser::Variable {
                name: "content_type".to_string(),
                value: "application/json".to_string(),
            },
        ];

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("GET https://httpbin.org/get"));
        assert!(text.contains("Accept: application/json"));
        assert!(!text.contains("{{host}}"));
        assert!(!text.contains("{{content_type}}"));
    }

    #[test]
    fn test_renders_resolved_body() {
        let mut app = app_with_requests(vec![request(Some("Post"), Method::Post, "{{host}}/post")]);
        app.requests[0].body = Some("{\"name\": \"{{username}}\"}".to_string());
        app.variables = vec![
            crate::parser::Variable {
                name: "host".to_string(),
                value: "https://httpbin.org".to_string(),
            },
            crate::parser::Variable {
                name: "username".to_string(),
                value: "restui".to_string(),
            },
        ];

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("{\"name\": \"restui\"}"));
    }

    #[test]
    fn test_renders_raw_template_when_variable_undefined() {
        let app = app_with_requests(vec![request(Some("Get"), Method::Get, "{{missing}}/get")]);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("GET {{missing}}/get"));
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
    fn test_renders_with_horizontal_offset() {
        let mut app = app_with_requests(vec![request(
            Some("Get users"),
            Method::Get,
            "https://example.com/users",
        )]);
        app.detail_scroll_offset_x = 5;

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        // With offset 5, the leading "GET h" is clipped; the panel shows from "ttps://..." onward.
        assert!(!text.contains("GET https://example.com/users"));
        assert!(text.contains("ttps://example.com/users"));
    }
}
