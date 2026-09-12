pub mod help_overlay;
pub mod request_detail;
pub mod request_list;
pub mod response_pane;
pub mod status_bar;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::Paragraph;

use crate::app::App;

pub fn view(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let panes = crate::layout::pane_areas((area.width, area.height), app.show_request_detail);

    let chrome = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let filename = app
        .file_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| app.file_path.display().to_string());

    frame.render_widget(Paragraph::new(format!("restui - {filename}")), chrome[0]);
    request_list::render(app, frame, panes.request_list);

    if let Some(detail) = panes.request_detail {
        request_detail::render(app, frame, detail);
    }
    response_pane::render(app, frame, panes.response_pane);
    status_bar::render(app, frame, chrome[2]);

    if app.show_help {
        help_overlay::render(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::view;
    use std::path::PathBuf;
    use std::time::Duration;

    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::{App, AppStatus, Focus};
    use crate::http::AppResponse;
    use crate::message::Message;
    use crate::parser::{Method, ParsedFile, ParsedRequest};

    fn app() -> App {
        App::new(
            PathBuf::from("requests.http"),
            ParsedFile {
                variables: vec![],
                requests: vec![ParsedRequest {
                    name: Some("List users".to_string()),
                    method: Method::Get,
                    url: "https://example.com/users".to_string(),
                    headers: vec![("Accept".to_string(), "application/json".to_string())],
                    body: None,
                    source_line: 1,
                }],
            },
        )
    }

    fn render_text(app: &App) -> String {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| view(app, frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let area = buffer.area();

        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_layout_has_title_bar() {
        let text = render_text(&app());

        assert!(text.contains("restui - requests.http"));
    }

    #[test]
    fn test_layout_has_request_list() {
        let text = render_text(&app());

        assert!(text.contains("Requests"));
    }

    #[test]
    fn test_layout_has_response_pane() {
        let text = render_text(&app());

        assert!(text.contains("Response"));
    }

    #[test]
    fn test_layout_splits_when_detail_panel_open() {
        let mut app = app();
        app.show_request_detail = true;

        let text = render_text(&app);

        assert!(text.contains("Request Detail"));
        assert!(text.contains("Response"));
    }

    #[test]
    fn test_layout_no_detail_panel_when_closed() {
        let app = app();

        let text = render_text(&app);

        assert!(!text.contains("Request Detail"));
        assert!(text.contains("Response"));
    }

    #[test]
    fn test_layout_has_status_bar() {
        let text = render_text(&app());

        assert!(text.contains("[Enter] Send"));
        assert!(text.contains("[Tab] Focus"));
        assert!(text.contains("[?] Help"));
        assert!(text.contains("[q] Quit"));
    }

    #[test]
    fn test_help_overlay_renders_when_visible() {
        let mut app = app();
        app.show_help = true;

        let text = render_text(&app);

        assert!(text.contains("Help (? or Esc to close)"));
        assert!(text.contains("Navigation"));
    }

    #[test]
    fn test_help_overlay_hidden_by_default() {
        let app = app();

        let text = render_text(&app);

        assert!(!text.contains("Help (? or Esc to close)"));
    }

    #[test]
    fn test_scroll_end_reveals_tail_of_wide_list_name() {
        let mut app = app();
        app.requests[0].name = Some(format!("{}Z端点", "日本語".repeat(12)));
        app.focus = Focus::RequestList;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollEnd);

        let text = render_text(&app);

        // Wide chars occupy two cells; buffer_text interleaves a space for each
        // continuation cell, so compare against the space-normalized text.
        assert!(text.replace(' ', "").contains("Z端点"));
        assert_eq!(app.list_scroll_offset_x, app.list_max_scroll_x());
        assert!(app.list_max_scroll_x() > 0);
    }

    #[test]
    fn test_scroll_bottom_reveals_end_of_wrapped_error() {
        let mut app = app();
        let marker = "OSStatus-67901";
        let message = format!("{}{marker}", "x".repeat(54 * 27));
        app.status = AppStatus::Error(message);
        app.focus = Focus::ResponsePane;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollBottom);

        let text = render_text(&app);

        assert!(text.contains(marker));
        assert_eq!(app.scroll_offset, app.response_max_scroll());
        assert!(app.response_max_scroll() > 0);
    }

    #[test]
    fn test_scroll_end_reveals_tail_of_long_response_line() {
        let mut app = app();
        let marker = "TAIL-MARKER-67901";
        app.response = Some(AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: format!("{}{marker}", "x".repeat(300)),
            content_type: Some("text/plain".to_string()),
            duration: Duration::from_millis(15),
            size_bytes: 317,
        });
        app.focus = Focus::ResponsePane;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollEnd);

        let text = render_text(&app);

        assert!(text.contains(marker));
        assert_eq!(app.scroll_offset_x, app.response_max_scroll_x());
        assert!(app.response_max_scroll_x() > 0);
    }
}
