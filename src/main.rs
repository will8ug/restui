use std::error::Error;
use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    self, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use restui::app::{App, Focus};
use restui::message::{Command, Message};
use restui::{http, parser, tls, ui};

#[derive(Parser)]
#[command(name = "restui", about = "TUI REST Client", version)]
struct Cli {
    /// Path to .http or .rest file
    file: std::path::PathBuf,
    /// Request timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,
    /// Disable SSL certificate verification
    #[arg(long)]
    no_verify: bool,
}

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    if !cli.file.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found: {}", cli.file.display()),
        )
        .into());
    }

    let contents = fs::read_to_string(&cli.file)?;
    let parsed_file = parser::parse(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cli.timeout))
        .use_preconfigured_tls(tls::client_config(cli.no_verify)?)
        .build()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _cleanup = TerminalCleanup;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cli.file, parsed_file);
    let (width, height) = terminal::size()?;
    app.update(Message::Resize(width, height));

    let (tx, rx) = mpsc::channel::<Message>();

    loop {
        let mut pending_messages = Vec::new();

        if event::poll(Duration::from_millis(50))?
            && let Some(message) = event_message(event::read()?, app.focus, app.show_help)
        {
            pending_messages.push(message);
        }

        while let Ok(message) = rx.try_recv() {
            pending_messages.push(message);
        }

        let mut should_quit = false;

        for message in pending_messages {
            match app.update(message) {
                Command::SendHttp(request) => {
                    let tx = tx.clone();
                    let client = client.clone();
                    thread::spawn(move || {
                        let message = match http::send_request(&client, &request) {
                            Ok(response) => Message::ResponseReceived(response),
                            Err(error) => Message::ResponseError(error.message),
                        };

                        let _ = tx.send(message);
                    });
                }
                Command::Quit => {
                    should_quit = true;
                    break;
                }
                Command::None => {}
            }
        }

        terminal.draw(|frame| ui::view(&app, frame))?;

        if should_quit {
            break;
        }
    }

    Ok(())
}

fn event_message(event: Event, focus: Focus, show_help: bool) -> Option<Message> {
    match event {
        Event::Key(key) => key_message(key, focus, show_help),
        Event::Resize(width, height) => Some(Message::Resize(width, height)),
        _ => None,
    }
}

fn key_message(key: KeyEvent, focus: Focus, show_help: bool) -> Option<Message> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    if show_help {
        return match key.code {
            KeyCode::Char('?') | KeyCode::Esc => Some(Message::ToggleHelp),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(match focus {
            Focus::RequestList => Message::SelectPrev,
            Focus::RequestDetail => Message::ScrollUp,
            Focus::ResponsePane => Message::ScrollUp,
        }),
        KeyCode::Down | KeyCode::Char('j') => Some(match focus {
            Focus::RequestList => Message::SelectNext,
            Focus::RequestDetail => Message::ScrollDown,
            Focus::ResponsePane => Message::ScrollDown,
        }),
        KeyCode::Char('g') => Some(match focus {
            Focus::RequestList => Message::SelectFirst,
            Focus::RequestDetail => Message::ScrollTop,
            Focus::ResponsePane => Message::ScrollTop,
        }),
        KeyCode::Char('G') => Some(match focus {
            Focus::RequestList => Message::SelectLast,
            Focus::RequestDetail => Message::ScrollBottom,
            Focus::ResponsePane => Message::ScrollBottom,
        }),
        KeyCode::Left | KeyCode::Char('h') => Some(Message::ScrollLeft),
        KeyCode::Right | KeyCode::Char('l') => Some(Message::ScrollRight),
        KeyCode::Home | KeyCode::Char('0') => Some(Message::ScrollStart),
        KeyCode::End | KeyCode::Char('$') => Some(Message::ScrollEnd),
        KeyCode::Enter => Some(Message::SendRequest),
        KeyCode::Tab => Some(Message::ToggleFocus),
        KeyCode::Char('R') => Some(Message::ReloadFile),
        KeyCode::Char('d') => Some(Message::ToggleRequestDetail),
        KeyCode::Char('?') => Some(Message::ToggleHelp),
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use super::key_message;
    use clap::Parser;
    use clap::error::ErrorKind;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use restui::app::Focus;
    use restui::message::Message;

    fn parse_version_error(args: &[&str]) -> clap::Error {
        match Cli::try_parse_from(args) {
            Ok(_) => panic!("version flag should short-circuit parsing"),
            Err(error) => error,
        }
    }

    #[test]
    fn test_version_long_flag() {
        let error = parse_version_error(&["restui", "--version"]);

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert!(error.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_version_short_flag() {
        let error = parse_version_error(&["restui", "-V"]);

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert!(error.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_shift_r_reloads_file() {
        let event = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT);

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::ReloadFile)
        ));
    }

    #[test]
    fn test_shift_r_without_reported_modifier_reloads_file() {
        // Terminals with CapsLock on deliver Char('R') without SHIFT set;
        // the binding matches the character only, so reload still fires.
        let event = KeyEvent::from(KeyCode::Char('R'));

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::ReloadFile)
        ));
    }

    #[test]
    fn test_lowercase_r_is_unbound() {
        let event = KeyEvent::from(KeyCode::Char('r'));

        assert!(key_message(event, Focus::RequestList, false).is_none());
    }

    #[test]
    fn test_g_selects_first_request_in_list() {
        let event = KeyEvent::from(KeyCode::Char('g'));

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::SelectFirst)
        ));
    }

    #[test]
    fn test_g_scrolls_top_in_request_detail() {
        let event = KeyEvent::from(KeyCode::Char('g'));

        assert!(matches!(
            key_message(event, Focus::RequestDetail, false),
            Some(Message::ScrollTop)
        ));
    }

    #[test]
    fn test_g_scrolls_top_in_response_pane() {
        let event = KeyEvent::from(KeyCode::Char('g'));

        assert!(matches!(
            key_message(event, Focus::ResponsePane, false),
            Some(Message::ScrollTop)
        ));
    }

    #[test]
    fn test_shift_g_selects_last_request_in_list() {
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::SelectLast)
        ));
    }

    #[test]
    fn test_shift_g_scrolls_bottom_in_request_detail() {
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);

        assert!(matches!(
            key_message(event, Focus::RequestDetail, false),
            Some(Message::ScrollBottom)
        ));
    }

    #[test]
    fn test_shift_g_scrolls_bottom_in_response_pane() {
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);

        assert!(matches!(
            key_message(event, Focus::ResponsePane, false),
            Some(Message::ScrollBottom)
        ));
    }

    #[test]
    fn test_uppercase_g_without_reported_modifier_selects_last() {
        let event = KeyEvent::from(KeyCode::Char('G'));

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::SelectLast)
        ));
    }

    #[test]
    fn test_jump_keys_ignored_when_help_visible() {
        let g = KeyEvent::from(KeyCode::Char('g'));
        let uppercase_g = KeyEvent::from(KeyCode::Char('G'));

        assert!(key_message(g, Focus::RequestList, true).is_none());
        assert!(key_message(uppercase_g, Focus::ResponsePane, true).is_none());
    }

    #[test]
    fn test_home_scrolls_start() {
        let event = KeyEvent::from(KeyCode::Home);

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::ScrollStart)
        ));
    }

    #[test]
    fn test_zero_scrolls_start() {
        let event = KeyEvent::from(KeyCode::Char('0'));

        assert!(matches!(
            key_message(event, Focus::ResponsePane, false),
            Some(Message::ScrollStart)
        ));
    }

    #[test]
    fn test_end_scrolls_end() {
        let event = KeyEvent::from(KeyCode::End);

        assert!(matches!(
            key_message(event, Focus::RequestList, false),
            Some(Message::ScrollEnd)
        ));
    }

    #[test]
    fn test_dollar_scrolls_end() {
        let event = KeyEvent::from(KeyCode::Char('$'));

        assert!(matches!(
            key_message(event, Focus::ResponsePane, false),
            Some(Message::ScrollEnd)
        ));
    }

    #[test]
    fn test_horizontal_jump_keys_ignored_when_help_visible() {
        let zero = KeyEvent::from(KeyCode::Char('0'));
        let dollar = KeyEvent::from(KeyCode::Char('$'));
        let home = KeyEvent::from(KeyCode::Home);
        let end = KeyEvent::from(KeyCode::End);

        assert!(key_message(zero, Focus::RequestList, true).is_none());
        assert!(key_message(dollar, Focus::RequestList, true).is_none());
        assert!(key_message(home, Focus::ResponsePane, true).is_none());
        assert!(key_message(end, Focus::ResponsePane, true).is_none());
    }
}
