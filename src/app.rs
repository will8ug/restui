use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use crate::http::AppResponse;
use crate::message::{Command, Message};
use crate::parser::{self, ParsedFile, ParsedRequest, Variable};
use crate::vars;

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Idle,
    Sending(Instant),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    RequestList,
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
                    self.status = AppStatus::Idle;
                }
                Err(error) => {
                    self.status = AppStatus::Error(error.to_string());
                }
            },
            Err(error) => {
                self.status = AppStatus::Error(format!(
                    "Failed to read {}: {error}",
                    self.file_path.display()
                ));
            }
        }
    }

    pub fn update(&mut self, msg: Message) -> Command {
        match msg {
            Message::SelectNext
                if self.focus == Focus::RequestList && !self.requests.is_empty() =>
            {
                self.selected_index = (self.selected_index + 1) % self.requests.len();
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
            Message::SendRequest => {
                let Some(request) = self.requests.get(self.selected_index) else {
                    self.status = AppStatus::Error("No request selected".to_string());
                    return Command::None;
                };

                match vars::resolve(&self.variables, request) {
                    Ok(resolved) => {
                        self.status = AppStatus::Sending(Instant::now());
                        Command::SendHttp(resolved)
                    }
                    Err(error) => {
                        self.status = AppStatus::Error(error.to_string());
                        Command::None
                    }
                }
            }
            Message::ResponseReceived(response) => {
                self.response = Some(response);
                self.status = AppStatus::Idle;
                self.scroll_offset = 0;
                self.last_sent_index = Some(self.selected_index);
                Command::None
            }
            Message::ResponseError(error) => {
                self.status = AppStatus::Error(error);
                Command::None
            }
            Message::ToggleFocus => {
                self.focus = match self.focus {
                    Focus::RequestList => Focus::ResponsePane,
                    Focus::ResponsePane => Focus::RequestList,
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
            Message::Quit => Command::Quit,
            Message::Resize(width, height) => {
                self.size = (width, height);
                Command::None
            }
            Message::SelectNext | Message::SelectPrev | Message::ScrollUp | Message::ScrollDown => {
                Command::None
            }
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

        let command = app.update(Message::SendRequest);

        assert!(matches!(command, Command::None));
        assert_eq!(
            app.status,
            AppStatus::Error("Undefined variable 'missing' in url".to_string())
        );
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
    fn test_toggle_focus() {
        let mut app = app_with_requests(vec![request("https://example.com")]);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::ResponsePane);

        app.update(Message::ToggleFocus);
        assert_eq!(app.focus, Focus::RequestList);
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
        assert_eq!(app.status, AppStatus::Idle);

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
}
