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
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let content_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(areas[1]);

    let filename = app
        .file_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| app.file_path.display().to_string());

    frame.render_widget(Paragraph::new(format!("restui - {filename}")), areas[0]);
    request_list::render(app, frame, content_areas[0]);

    if app.show_request_detail {
        let right_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(content_areas[1]);

        request_detail::render(app, frame, right_areas[0]);
        response_pane::render(app, frame, right_areas[1]);
    } else {
        response_pane::render(app, frame, content_areas[1]);
    }
    status_bar::render(app, frame, areas[2]);

    if app.show_help {
        help_overlay::render(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::view;
    use std::path::PathBuf;

    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::App;
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
}
