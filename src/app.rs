use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use ratatui::layout::Margin;

use crate::content;
use crate::http::AppResponse;
use crate::layout;
use crate::message::{Command, Message};
use crate::parser::{self, ParsedFile, ParsedRequest, Variable};
use crate::vars;

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Idle,
    Sending(Instant),
    Reloaded(Instant),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    RequestList,
    RequestDetail,
    ResponsePane,
}

pub struct App {
    pub file_path: PathBuf,
    pub requests: Vec<ParsedRequest>,
    pub variables: Vec<Variable>,
    pub selected_index: usize,
    pub response: Option<AppResponse>,
    pub status: AppStatus,
    pub focus: Focus,
    pub scroll_offset: usize,
    pub size: (u16, u16),
    pub last_sent_index: Option<usize>,
    pub show_help: bool,
    pub show_request_detail: bool,
    pub detail_scroll_offset: usize,
    pub list_scroll_offset_x: usize,
    pub detail_scroll_offset_x: usize,
    pub scroll_offset_x: usize,
}

impl App {
    pub fn new(file_path: PathBuf, parsed_file: ParsedFile) -> Self {
        Self {
            file_path,
            requests: parsed_file.requests,
            variables: parsed_file.variables,
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
            list_scroll_offset_x: 0,
            detail_scroll_offset_x: 0,
            scroll_offset_x: 0,
        }
    }

    pub fn reload(&mut self) {
        match fs::read_to_string(&self.file_path) {
            Ok(contents) => match parser::parse(&contents) {
                Ok(parsed_file) => {
                    self.requests = parsed_file.requests;
                    self.variables = parsed_file.variables;
                    self.selected_index = match self.requests.len() {
                        0 => 0,
                        len => self.selected_index.min(len.saturating_sub(1)),
                    };
                    self.last_sent_index = self
                        .last_sent_index
                        .filter(|index| *index < self.requests.len());
                    self.status = AppStatus::Reloaded(Instant::now());
                }
                Err(error) => {
                    self.set_error(error.to_string());
                }
            },
            Err(error) => {
                self.set_error(format!(
                    "Failed to read {}: {error}",
                    self.file_path.display()
                ));
            }
        }
    }

    pub fn update(&mut self, msg: Message) -> Command {
        self.handle(msg)
    }

    fn set_error(&mut self, message: String) {
        self.status = AppStatus::Error(message);
        self.scroll_offset = 0;
        self.scroll_offset_x = 0;
    }

    pub fn response_max_scroll(&self) -> usize {
        let areas = layout::pane_areas(self.size, self.show_request_detail);
        let inner = areas.response_pane.inner(Margin::new(1, 1));
        let line_count = match &self.status {
            AppStatus::Error(message) => {
                content::wrapped_line_count(message, usize::from(inner.width))
            }
            _ => match &self.response {
                Some(response) => content::format_response(response).lines().count(),
                None => 0,
            },
        };
        line_count.saturating_sub(usize::from(inner.height))
    }

    pub fn response_max_scroll_x(&self) -> usize {
        let areas = layout::pane_areas(self.size, self.show_request_detail);
        let inner = areas.response_pane.inner(Margin::new(1, 1));
        match &self.status {
            // Error text renders wrapped, so it never overflows horizontally.
            AppStatus::Error(_) => 0,
            _ => match &self.response {
                Some(response) => content::max_line_width(&content::format_response(response))
                    .saturating_sub(usize::from(inner.width)),
                None => 0,
            },
        }
    }

    pub fn detail_max_scroll(&self) -> usize {
        let areas = layout::pane_areas(self.size, self.show_request_detail);
        match areas.request_detail {
            Some(detail_area) if !self.requests.is_empty() => {
                let inner = detail_area.inner(Margin::new(1, 1));
                content::format_request_detail(&self.variables, &self.requests[self.selected_index])
                    .lines()
                    .count()
                    .saturating_sub(usize::from(inner.height))
            }
            _ => 0,
        }
    }

    pub fn detail_max_scroll_x(&self) -> usize {
        let areas = layout::pane_areas(self.size, self.show_request_detail);
        match areas.request_detail {
            Some(detail_area) if !self.requests.is_empty() => {
                let inner = detail_area.inner(Margin::new(1, 1));
                content::max_line_width(&content::format_request_detail(
                    &self.variables,
                    &self.requests[self.selected_index],
                ))
                .saturating_sub(usize::from(inner.width))
            }
            _ => 0,
        }
    }

    pub fn list_max_scroll_x(&self) -> usize {
        let areas = layout::pane_areas(self.size, self.show_request_detail);
        let inner = areas.request_list.inner(Margin::new(1, 1));
        self.requests
            .iter()
            .enumerate()
            .map(|(index, request)| {
                let selected_prefix = if index == self.selected_index {
                    ">"
                } else {
                    " "
                };
                let sent_prefix = if self.last_sent_index == Some(index) {
                    "●"
                } else {
                    " "
                };
                let label = content::request_label(request);
                content::max_line_width(&format!("{selected_prefix}{sent_prefix} {label}"))
            })
            .max()
            .unwrap_or(0)
            .saturating_sub(usize::from(inner.width))
    }

    fn handle(&mut self, msg: Message) -> Command {
        match msg {
            Message::SelectNext
                if self.focus == Focus::RequestList && !self.requests.is_empty() =>
            {
                self.selected_index = (self.selected_index + 1) % self.requests.len();
                self.detail_scroll_offset = 0;
                self.list_scroll_offset_x = 0;
                self.detail_scroll_offset_x = 0;
                Command::None
            }
            Message::SelectPrev
                if self.focus == Focus::RequestList && !self.requests.is_empty() =>
            {
                self.selected_index = if self.selected_index == 0 {
                    self.requests.len() - 1
                } else {
                    self.selected_index - 1
                };
                self.detail_scroll_offset = 0;
                self.list_scroll_offset_x = 0;
                self.detail_scroll_offset_x = 0;
                Command::None
            }
            Message::SelectFirst
                if self.focus == Focus::RequestList && !self.requests.is_empty() =>
            {
                self.selected_index = 0;
                self.detail_scroll_offset = 0;
                self.list_scroll_offset_x = 0;
                self.detail_scroll_offset_x = 0;
                Command::None
            }
            Message::SelectLast
                if self.focus == Focus::RequestList && !self.requests.is_empty() =>
            {
                self.selected_index = self.requests.len() - 1;
                self.detail_scroll_offset = 0;
                self.list_scroll_offset_x = 0;
                self.detail_scroll_offset_x = 0;
                Command::None
            }
            Message::ScrollUp if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(1);
                Command::None
            }
            Message::ScrollDown if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset = self.detail_scroll_offset.saturating_add(1);
                Command::None
            }
            Message::ScrollUp if self.focus == Focus::ResponsePane => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                Command::None
            }
            Message::ScrollDown if self.focus == Focus::ResponsePane => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                Command::None
            }
            Message::ScrollTop if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset = 0;
                Command::None
            }
            Message::ScrollTop if self.focus == Focus::ResponsePane => {
                self.scroll_offset = 0;
                Command::None
            }
            Message::ScrollBottom if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset = self.detail_max_scroll();
                Command::None
            }
            Message::ScrollBottom if self.focus == Focus::ResponsePane => {
                self.scroll_offset = self.response_max_scroll();
                Command::None
            }
            Message::ScrollLeft if self.focus == Focus::RequestList => {
                self.list_scroll_offset_x = self.list_scroll_offset_x.saturating_sub(1);
                Command::None
            }
            Message::ScrollRight if self.focus == Focus::RequestList => {
                self.list_scroll_offset_x = self.list_scroll_offset_x.saturating_add(1);
                Command::None
            }
            Message::ScrollLeft if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset_x = self.detail_scroll_offset_x.saturating_sub(1);
                Command::None
            }
            Message::ScrollRight if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset_x = self.detail_scroll_offset_x.saturating_add(1);
                Command::None
            }
            Message::ScrollLeft if self.focus == Focus::ResponsePane => {
                self.scroll_offset_x = self.scroll_offset_x.saturating_sub(1);
                Command::None
            }
            Message::ScrollRight if self.focus == Focus::ResponsePane => {
                self.scroll_offset_x = self.scroll_offset_x.saturating_add(1);
                Command::None
            }
            Message::ScrollStart if self.focus == Focus::RequestList => {
                self.list_scroll_offset_x = 0;
                Command::None
            }
            Message::ScrollStart if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset_x = 0;
                Command::None
            }
            Message::ScrollStart if self.focus == Focus::ResponsePane => {
                self.scroll_offset_x = 0;
                Command::None
            }
            Message::ScrollEnd if self.focus == Focus::RequestList => {
                self.list_scroll_offset_x = self.list_max_scroll_x();
                Command::None
            }
            Message::ScrollEnd if self.focus == Focus::RequestDetail => {
                self.detail_scroll_offset_x = self.detail_max_scroll_x();
                Command::None
            }
            Message::ScrollEnd if self.focus == Focus::ResponsePane => {
                self.scroll_offset_x = self.response_max_scroll_x();
                Command::None
            }
            Message::SendRequest => {
                let Some(request) = self.requests.get(self.selected_index) else {
                    self.set_error("No request selected".to_string());
                    return Command::None;
                };

                match vars::resolve(&self.variables, request) {
                    Ok(resolved) => {
                        self.status = AppStatus::Sending(Instant::now());
                        Command::SendHttp(resolved)
                    }
                    Err(error) => {
                        self.set_error(error.to_string());
                        Command::None
                    }
                }
            }
            Message::ResponseReceived(response) => {
                self.response = Some(response);
                self.status = AppStatus::Idle;
                self.scroll_offset = 0;
                self.scroll_offset_x = 0;
                self.last_sent_index = Some(self.selected_index);
                Command::None
            }
            Message::ResponseError(error) => {
                self.set_error(error);
                Command::None
            }
            Message::ToggleFocus => {
                self.focus = if self.show_request_detail {
                    match self.focus {
                        Focus::RequestList => Focus::RequestDetail,
                        Focus::RequestDetail => Focus::ResponsePane,
                        Focus::ResponsePane => Focus::RequestList,
                    }
                } else {
                    match self.focus {
                        Focus::RequestList => Focus::ResponsePane,
                        Focus::ResponsePane => Focus::RequestList,
                        Focus::RequestDetail => Focus::RequestList,
                    }
                };
                Command::None
            }
            Message::ReloadFile => {
                self.reload();
                Command::None
            }
            Message::ToggleHelp => {
                self.show_help = !self.show_help;
                Command::None
            }
            Message::ToggleRequestDetail => {
                self.show_request_detail = !self.show_request_detail;
                if !self.show_request_detail && self.focus == Focus::RequestDetail {
                    self.focus = Focus::RequestList;
                }
                Command::None
            }
            Message::Quit => Command::Quit,
            Message::Resize(width, height) => {
                self.size = (width, height);
                Command::None
            }
            Message::SelectNext
            | Message::SelectPrev
            | Message::SelectFirst
            | Message::SelectLast
            | Message::ScrollUp
            | Message::ScrollDown
            | Message::ScrollTop
            | Message::ScrollBottom
            | Message::ScrollStart
            | Message::ScrollEnd
            | Message::ScrollLeft
            | Message::ScrollRight => Command::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Command;
    use crate::parser::Method;
    use std::env;
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn variable(name: &str, value: &str) -> Variable {
        Variable {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn request(url: &str) -> ParsedRequest {
        ParsedRequest {
            name: Some("example".to_string()),
            method: Method::Get,
            url: url.to_string(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
            body: None,
            source_line: 1,
        }
    }

    fn parsed_file(requests: Vec<ParsedRequest>, variables: Vec<Variable>) -> ParsedFile {
        ParsedFile {
            requests,
            variables,
        }
    }

    fn sample_response() -> AppResponse {
        AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: "{}".to_string(),
            content_type: Some("application/json".to_string()),
            duration: Duration::from_millis(15),
            size_bytes: 2,
        }
    }

    fn temp_file_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("restui-{name}-{nanos}.http"))
    }

    fn app_with_requests(requests: Vec<ParsedRequest>) -> App {
        App::new(
            PathBuf::from("requests.http"),
            parsed_file(requests, vec![]),
        )
    }

    #[test]
    fn test_select_next() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
        ]);

        let command = app.update(Message::SelectNext);

        assert!(matches!(command, Command::None));
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_select_next_wraps() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
        ]);
        app.selected_index = 1;

        app.update(Message::SelectNext);

        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_select_prev() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
        ]);
        app.selected_index = 1;

        app.update(Message::SelectPrev);

        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_select_prev_wraps() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
        ]);

        app.update(Message::SelectPrev);

        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_scroll_up() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;
        app.scroll_offset = 3;

        app.update(Message::ScrollUp);

        assert_eq!(app.scroll_offset, 2);
    }

    #[test]
    fn test_scroll_down() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;

        app.update(Message::ScrollDown);

        assert_eq!(app.scroll_offset, 1);
    }

    #[test]
    fn test_scroll_up_at_zero() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;

        app.update(Message::ScrollUp);

        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_send_request_success() {
        let file = parsed_file(
            vec![request("{{host}}/users")],
            vec![variable("host", "https://example.com")],
        );
        let mut app = App::new(PathBuf::from("requests.http"), file);

        let command = app.update(Message::SendRequest);

        match command {
            Command::SendHttp(resolved) => {
                assert_eq!(resolved.url, "https://example.com/users");
            }
            other => panic!("expected SendHttp command, got {other:?}"),
        }

        assert!(matches!(app.status, AppStatus::Sending(_)));
    }

    #[test]
    fn test_send_request_undefined_var() {
        let mut app = app_with_requests(vec![request("{{missing}}/users")]);
        app.scroll_offset = 3;
        app.scroll_offset_x = 2;

        let command = app.update(Message::SendRequest);

        assert!(matches!(command, Command::None));
        assert_eq!(
            app.status,
            AppStatus::Error("Undefined variable 'missing' in url".to_string())
        );
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_send_request_no_selection_resets_scroll_offsets() {
        let mut app = app_with_requests(vec![]);
        app.scroll_offset = 5;
        app.scroll_offset_x = 2;

        app.update(Message::SendRequest);

        assert_eq!(
            app.status,
            AppStatus::Error("No request selected".to_string())
        );
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_response_received() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        let response = sample_response();
        app.status = AppStatus::Sending(Instant::now());
        app.scroll_offset = 4;

        app.update(Message::ResponseReceived(response.clone()));

        assert_eq!(app.response, Some(response));
        assert_eq!(app.status, AppStatus::Idle);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.last_sent_index, Some(0));
    }

    #[test]
    fn test_response_error() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::ResponseError("boom".to_string()));

        assert_eq!(app.status, AppStatus::Error("boom".to_string()));
    }

    #[test]
    fn test_response_error_resets_scroll_offsets() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.scroll_offset = 4;
        app.scroll_offset_x = 3;

        app.update(Message::ResponseError("boom".to_string()));

        assert_eq!(app.status, AppStatus::Error("boom".to_string()));
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_toggle_focus() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::ResponsePane);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::RequestList);
    }

    #[test]
    fn test_toggle_request_detail_on() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        let command = app.update(Message::ToggleRequestDetail);

        assert!(app.show_request_detail);
        assert!(matches!(command, Command::None));
    }

    #[test]
    fn test_toggle_request_detail_off() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;

        app.update(Message::ToggleRequestDetail);

        assert!(!app.show_request_detail);
    }

    #[test]
    fn test_toggle_request_detail_off_resets_focus() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;

        app.update(Message::ToggleRequestDetail);

        assert_eq!(app.focus, Focus::RequestList);
    }

    #[test]
    fn test_focus_cycles_three_panes_when_detail_open() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::RequestDetail);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::ResponsePane);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::RequestList);
    }

    #[test]
    fn test_focus_cycles_two_panes_when_detail_closed() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::ResponsePane);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::RequestList);
    }

    #[test]
    fn test_detail_scroll_up() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset = 3;

        app.update(Message::ScrollUp);

        assert_eq!(app.detail_scroll_offset, 2);
    }

    #[test]
    fn test_detail_scroll_down() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;

        app.update(Message::ScrollDown);

        assert_eq!(app.detail_scroll_offset, 1);
    }

    #[test]
    fn test_detail_scroll_reset_on_selection_change() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
        ]);
        app.show_request_detail = true;
        app.detail_scroll_offset = 5;

        app.update(Message::SelectNext);

        assert_eq!(app.detail_scroll_offset, 0);
    }

    #[test]
    fn test_quit_returns_command() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        let command = app.update(Message::Quit);

        assert!(matches!(command, Command::Quit));
    }

    #[test]
    fn test_reload_file_updates_requests_and_variables() {
        let file_path = temp_file_path("reload-success");
        fs::write(
            &file_path,
            "@host = https://reloaded.example.com\n\nGET {{host}}/health",
        )
        .expect("should write temp request file");

        let mut app = App::new(
            file_path.clone(),
            parsed_file(vec![request("https://stale.example.com")], vec![]),
        );

        app.update(Message::ReloadFile);

        assert_eq!(app.requests.len(), 1);
        assert_eq!(app.requests[0].url, "{{host}}/health");
        assert_eq!(app.variables.len(), 1);
        assert_eq!(app.variables[0].name, "host");
        assert!(matches!(app.status, AppStatus::Reloaded(_)));

        fs::remove_file(&file_path).expect("should remove temp request file");
    }

    #[test]
    fn test_reload_file_parse_error_sets_status() {
        let file_path = temp_file_path("reload-error");
        fs::write(&file_path, "TRACE https://example.com")
            .expect("should write invalid temp request file");

        let original_request = request("https://original.example.com");
        let mut app = App::new(
            file_path.clone(),
            parsed_file(vec![original_request.clone()], vec![]),
        );

        app.update(Message::ReloadFile);

        assert_eq!(app.requests, vec![original_request]);
        assert!(
            matches!(app.status, AppStatus::Error(message) if message.contains("Parse error at line 1"))
        );

        fs::remove_file(&file_path).expect("should remove temp request file");
    }

    #[test]
    fn test_reload_file_parse_error_resets_scroll_offsets() {
        let file_path = temp_file_path("reload-parse-scroll");
        fs::write(&file_path, "TRACE https://example.com")
            .expect("should write invalid temp request file");

        let mut app = App::new(
            file_path.clone(),
            parsed_file(vec![request("https://original.example.com")], vec![]),
        );
        app.scroll_offset = 4;
        app.scroll_offset_x = 3;

        app.update(Message::ReloadFile);

        assert!(
            matches!(app.status, AppStatus::Error(message) if message.contains("Parse error at line 1"))
        );
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.scroll_offset_x, 0);

        fs::remove_file(&file_path).expect("should remove temp request file");
    }

    #[test]
    fn test_reload_file_read_error_resets_scroll_offsets() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.file_path = temp_file_path("reload-missing");
        app.scroll_offset = 4;
        app.scroll_offset_x = 3;

        app.update(Message::ReloadFile);

        assert!(matches!(app.status, AppStatus::Error(_)));
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_resize_updates_size() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::Resize(120, 40));

        assert_eq!(app.size, (120, 40));
    }

    #[test]
    fn test_toggle_help_on() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::ToggleHelp);

        assert!(app.show_help);
    }

    #[test]
    fn test_toggle_help_off() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_help = true;

        app.update(Message::ToggleHelp);

        assert!(!app.show_help);
    }

    #[test]
    fn test_toggle_help_returns_none() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        let command = app.update(Message::ToggleHelp);

        assert!(matches!(command, Command::None));
    }

    #[test]
    fn test_scroll_left_response() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;
        app.scroll_offset_x = 3;

        app.update(Message::ScrollLeft);

        assert_eq!(app.scroll_offset_x, 2);
    }

    #[test]
    fn test_scroll_right_response() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;

        app.update(Message::ScrollRight);

        assert_eq!(app.scroll_offset_x, 1);
    }

    #[test]
    fn test_scroll_left_at_zero_response() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;

        app.update(Message::ScrollLeft);

        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_left_detail() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset_x = 3;

        app.update(Message::ScrollLeft);

        assert_eq!(app.detail_scroll_offset_x, 2);
    }

    #[test]
    fn test_scroll_right_detail() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;

        app.update(Message::ScrollRight);

        assert_eq!(app.detail_scroll_offset_x, 1);
    }

    #[test]
    fn test_scroll_left_list() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::RequestList;
        app.list_scroll_offset_x = 3;

        app.update(Message::ScrollLeft);

        assert_eq!(app.list_scroll_offset_x, 2);
    }

    #[test]
    fn test_scroll_right_list() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::RequestList;

        app.update(Message::ScrollRight);

        assert_eq!(app.list_scroll_offset_x, 1);
    }

    #[test]
    fn test_list_horizontal_reset_on_selection_change() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
        ]);
        app.list_scroll_offset_x = 5;
        app.detail_scroll_offset_x = 7;

        app.update(Message::SelectNext);

        assert_eq!(app.list_scroll_offset_x, 0);
        assert_eq!(app.detail_scroll_offset_x, 0);
    }

    #[test]
    fn test_response_horizontal_reset_on_response_received() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.scroll_offset_x = 5;

        app.update(Message::ResponseReceived(sample_response()));

        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_horizontal_persists_across_focus_change() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.list_scroll_offset_x = 2;
        app.detail_scroll_offset_x = 4;
        app.scroll_offset_x = 6;

        // Cycle full circle: List -> Detail -> Response -> List.
        app.update(Message::ToggleFocus);
        app.update(Message::ToggleFocus);
        app.update(Message::ToggleFocus);

        assert_eq!(app.focus, Focus::RequestList);
        assert_eq!(app.list_scroll_offset_x, 2);
        assert_eq!(app.detail_scroll_offset_x, 4);
        assert_eq!(app.scroll_offset_x, 6);
    }

    #[test]
    fn test_select_first_jumps_to_start_and_resets_offsets() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
            request("https://example.com/three"),
        ]);
        app.selected_index = 2;
        app.detail_scroll_offset = 5;
        app.list_scroll_offset_x = 3;
        app.detail_scroll_offset_x = 7;

        let command = app.update(Message::SelectFirst);

        assert!(matches!(command, Command::None));
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.detail_scroll_offset, 0);
        assert_eq!(app.list_scroll_offset_x, 0);
        assert_eq!(app.detail_scroll_offset_x, 0);
    }

    #[test]
    fn test_select_last_jumps_to_end_and_resets_offsets() {
        let mut app = app_with_requests(vec![
            request("https://example.com/one"),
            request("https://example.com/two"),
            request("https://example.com/three"),
        ]);
        app.detail_scroll_offset = 5;
        app.list_scroll_offset_x = 3;
        app.detail_scroll_offset_x = 7;

        let command = app.update(Message::SelectLast);

        assert!(matches!(command, Command::None));
        assert_eq!(app.selected_index, 2);
        assert_eq!(app.detail_scroll_offset, 0);
        assert_eq!(app.list_scroll_offset_x, 0);
        assert_eq!(app.detail_scroll_offset_x, 0);
    }

    #[test]
    fn test_select_first_on_empty_requests_is_noop() {
        let mut app = app_with_requests(vec![]);

        let command = app.update(Message::SelectFirst);

        assert!(matches!(command, Command::None));
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_select_last_on_empty_requests_is_noop() {
        let mut app = app_with_requests(vec![]);

        let command = app.update(Message::SelectLast);

        assert!(matches!(command, Command::None));
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_scroll_top_detail() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset = 8;

        app.update(Message::ScrollTop);

        assert_eq!(app.detail_scroll_offset, 0);
    }

    #[test]
    fn test_scroll_bottom_detail_jumps_to_max() {
        let mut req = request("https://example.com");
        req.body = Some(
            (0..40)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut app = app_with_requests(vec![req]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset = 3;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollBottom);

        assert_eq!(app.detail_scroll_offset, app.detail_max_scroll());
        assert!(app.detail_scroll_offset > 3);
    }

    #[test]
    fn test_scroll_top_response() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;
        app.scroll_offset = 8;

        app.update(Message::ScrollTop);

        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_bottom_response_jumps_to_max() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        let body = (0..40)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.response = Some(AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body,
            content_type: Some("text/plain".to_string()),
            duration: Duration::from_millis(15),
            size_bytes: 0,
        });
        app.focus = Focus::ResponsePane;
        app.scroll_offset = 3;

        app.update(Message::Resize(50, 10));
        app.update(Message::ScrollBottom);

        assert_eq!(app.scroll_offset, app.response_max_scroll());
        assert!(app.scroll_offset > 3);
    }

    #[test]
    fn test_scroll_bottom_ignored_in_request_list() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::RequestList;
        app.scroll_offset = 4;

        let command = app.update(Message::ScrollBottom);

        assert!(matches!(command, Command::None));
        assert_eq!(app.scroll_offset, 4);
    }

    #[test]
    fn test_long_response_computes_max_scroll() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        let body = (0..40)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.response = Some(AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body,
            content_type: Some("text/plain".to_string()),
            duration: Duration::from_millis(15),
            size_bytes: 0,
        });

        app.update(Message::Resize(50, 10));

        assert!(app.response_max_scroll() > 0);
    }

    #[test]
    fn test_short_response_computes_zero_max_scroll() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.response = Some(sample_response());

        app.update(Message::Resize(50, 10));

        assert_eq!(app.response_max_scroll(), 0);
    }

    #[test]
    fn test_long_wrapped_error_computes_max_scroll() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.status = AppStatus::Error("x".repeat(1500));

        app.update(Message::Resize(80, 20));

        assert!(app.response_max_scroll() > 0);
    }

    #[test]
    fn test_long_request_computes_max_scroll() {
        let mut req = request("https://example.com/users");
        req.body = Some(
            (0..40)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut app = app_with_requests(vec![req]);
        app.show_request_detail = true;

        app.update(Message::Resize(80, 20));

        assert!(app.detail_max_scroll() > 0);
    }

    #[test]
    fn test_detail_max_scroll_counts_resolved_lines() {
        let req = ParsedRequest {
            name: Some("Bulk".to_string()),
            method: Method::Get,
            url: "https://example.com".to_string(),
            headers: vec![],
            body: Some("{{payload}}".to_string()),
            source_line: 1,
        };
        let mut app = app_with_requests(vec![req]);
        app.show_request_detail = true;
        app.update(Message::Resize(80, 20));

        assert_eq!(app.detail_max_scroll(), 0);

        app.variables = vec![variable(
            "payload",
            &(0..60)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )];

        assert!(app.detail_max_scroll() > 0);
    }

    #[test]
    fn test_detail_max_scroll_x_uses_resolved_text() {
        let mut app = app_with_requests(vec![request("{{host}}/get")]);
        app.show_request_detail = true;
        app.variables = vec![variable("host", &format!("https://{}", "x".repeat(120)))];

        app.update(Message::Resize(80, 20));

        assert!(app.detail_max_scroll_x() > 0);
    }

    #[test]
    fn test_empty_detail_state_computes_zero_max_scroll() {
        let mut app = app_with_requests(vec![]);
        app.show_request_detail = true;

        app.update(Message::Resize(80, 20));

        assert_eq!(app.detail_max_scroll(), 0);
    }

    #[test]
    fn test_scroll_start_list_zeroes_offset() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::RequestList;
        app.list_scroll_offset_x = 5;

        app.update(Message::ScrollStart);

        assert_eq!(app.list_scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_start_detail_zeroes_offset() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset_x = 5;

        app.update(Message::ScrollStart);

        assert_eq!(app.detail_scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_start_response_zeroes_offset() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::ResponsePane;
        app.scroll_offset_x = 5;

        app.update(Message::ScrollStart);

        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_end_short_list_zeroes_offset() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::RequestList;
        app.list_scroll_offset_x = 5;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollEnd);

        assert_eq!(app.list_scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_end_short_detail_zeroes_offset() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset_x = 5;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollEnd);

        assert_eq!(app.detail_scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_end_short_response_zeroes_offset() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.response = Some(sample_response());
        app.focus = Focus::ResponsePane;
        app.scroll_offset_x = 5;

        app.update(Message::Resize(50, 10));
        app.update(Message::ScrollEnd);

        assert_eq!(app.scroll_offset_x, 0);
    }

    #[test]
    fn test_scroll_end_ignored_in_request_list() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.focus = Focus::RequestList;
        app.scroll_offset_x = 4;

        let command = app.update(Message::ScrollEnd);

        assert!(matches!(command, Command::None));
        assert_eq!(app.scroll_offset_x, 4);
    }

    #[test]
    fn test_scroll_end_list_jumps_to_max() {
        let mut req = request("https://example.com");
        req.name = Some("x".repeat(200));
        let mut app = app_with_requests(vec![req]);
        app.focus = Focus::RequestList;
        app.list_scroll_offset_x = 3;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollEnd);

        assert_eq!(app.list_scroll_offset_x, app.list_max_scroll_x());
        assert!(app.list_scroll_offset_x > 3);
    }

    #[test]
    fn test_scroll_end_detail_jumps_to_max() {
        let url = format!("https://example.com/{}", "x".repeat(200));
        let mut app = app_with_requests(vec![request(&url)]);
        app.show_request_detail = true;
        app.focus = Focus::RequestDetail;
        app.detail_scroll_offset_x = 3;

        app.update(Message::Resize(80, 20));
        app.update(Message::ScrollEnd);

        assert_eq!(app.detail_scroll_offset_x, app.detail_max_scroll_x());
        assert!(app.detail_scroll_offset_x > 3);
    }

    #[test]
    fn test_scroll_end_response_jumps_to_max() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.response = Some(AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: "x".repeat(200),
            content_type: Some("text/plain".to_string()),
            duration: Duration::from_millis(15),
            size_bytes: 200,
        });
        app.focus = Focus::ResponsePane;
        app.scroll_offset_x = 3;

        app.update(Message::Resize(50, 10));
        app.update(Message::ScrollEnd);

        assert_eq!(app.scroll_offset_x, app.response_max_scroll_x());
        assert!(app.scroll_offset_x > 3);
    }

    #[test]
    fn test_error_status_computes_zero_max_scroll_x() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.status = AppStatus::Error("x".repeat(1500));

        app.update(Message::Resize(80, 20));

        assert_eq!(app.response_max_scroll_x(), 0);
    }

    #[test]
    fn test_long_response_computes_max_scroll_x() {
        let mut app = app_with_requests(vec![request("https://example.com")]);
        app.response = Some(AppResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: "x".repeat(200),
            content_type: Some("text/plain".to_string()),
            duration: Duration::from_millis(15),
            size_bytes: 200,
        });

        app.update(Message::Resize(50, 10));

        assert!(app.response_max_scroll_x() > 0);
    }

    #[test]
    fn test_no_response_computes_zero_max_scroll_x() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::Resize(50, 10));

        assert_eq!(app.response_max_scroll_x(), 0);
    }

    #[test]
    fn test_long_request_name_computes_list_max_scroll_x() {
        let mut req = request("https://example.com");
        req.name = Some("x".repeat(200));
        let mut app = app_with_requests(vec![req]);

        app.update(Message::Resize(80, 20));

        assert!(app.list_max_scroll_x() > 0);
    }

    #[test]
    fn test_empty_requests_compute_zero_list_max_scroll_x() {
        let mut app = app_with_requests(vec![]);

        app.update(Message::Resize(80, 20));

        assert_eq!(app.list_max_scroll_x(), 0);
    }
}
