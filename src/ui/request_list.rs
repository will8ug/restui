use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::app::{App, Focus};
use crate::parser::ParsedRequest;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border_color = if app.focus == Focus::RequestList {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let items = app
        .requests
        .iter()
        .enumerate()
        .map(|(index, request)| {
            let selected_prefix = if index == app.selected_index {
                ">"
            } else {
                " "
            };
            let sent_prefix = if app.last_sent_index == Some(index) {
                "●"
            } else {
                " "
            };
            let label = request_label(request);
            ListItem::new(format!("{selected_prefix}{sent_prefix} {label}"))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Requests")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let mut state = ListState::default();
    if !app.requests.is_empty() {
        state.select(Some(app.selected_index));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn request_label(request: &ParsedRequest) -> String {
    request
        .name
        .clone()
        .unwrap_or_else(|| format!("{} {}", request.method, url_display(&request.url)))
}

fn url_display(url: &str) -> String {
    if let Some(path) = http_url_path(url) {
        path.to_string()
    } else {
        url.to_string()
    }
}

fn http_url_path(url: &str) -> Option<&str> {
    let scheme_index = url.find("://")?;
    let path_start = url[scheme_index + 3..].find('/')? + scheme_index + 3;
    Some(&url[path_start..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, AppStatus};
    use crate::http::AppResponse;
    use crate::parser::{Method, ParsedRequest};

    fn request(name: Option<&str>, method: Method, url: &str) -> ParsedRequest {
        ParsedRequest {
            name: name.map(str::to_owned),
            method,
            url: url.to_string(),
            headers: vec![],
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
            focus: Focus::RequestList,
            scroll_offset: 0,
            size: (0, 0),
            last_sent_index: None,
        }
    }

    fn render_app(app: &App) -> TestBackend {
        let backend = TestBackend::new(40, 6);
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
    fn test_renders_request_names() {
        let app = app_with_requests(vec![
            request(Some("List users"), Method::Get, "https://example.com/users"),
            request(None, Method::Post, "https://example.com/users"),
        ]);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains("List users"));
        assert!(text.contains("POST /users"));
    }

    #[test]
    fn test_renders_selected_highlight() {
        let mut app = app_with_requests(vec![request(
            Some("List users"),
            Method::Get,
            "https://example.com/users",
        )]);
        app.selected_index = 0;

        let backend = render_app(&app);
        let cell = &backend.buffer()[(1, 1)];

        assert_eq!(cell.symbol(), ">");
        assert_eq!(cell.bg, Color::DarkGray);
        assert_eq!(cell.fg, Color::White);
    }

    #[test]
    fn test_renders_sent_indicator() {
        let mut app = app_with_requests(vec![
            request(Some("List users"), Method::Get, "https://example.com/users"),
            request(
                Some("Create user"),
                Method::Post,
                "https://example.com/users",
            ),
        ]);
        app.selected_index = 0;
        app.last_sent_index = Some(1);

        let backend = render_app(&app);
        let text = buffer_text(&backend);

        assert!(text.contains(" ● Create user"));
    }

    #[test]
    fn test_request_label_prefers_name() {
        let request = request(Some("Named"), Method::Get, "https://example.com/users");

        assert_eq!(request_label(&request), "Named");
    }

    #[test]
    fn test_request_label_falls_back_to_method_and_path() {
        let request = request(None, Method::Get, "https://example.com/users");

        assert_eq!(request_label(&request), "GET /users");
    }
}
