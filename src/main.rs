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
use restui::{http, parser, ui};

#[derive(Parser)]
#[command(name = "restui", about = "TUI REST Client")]
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
        .danger_accept_invalid_certs(cli.no_verify)
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
        KeyCode::Enter => Some(Message::SendRequest),
        KeyCode::Tab => Some(Message::ToggleFocus),
        KeyCode::Char('r') => Some(Message::ReloadFile),
        KeyCode::Char('?') => Some(Message::ToggleHelp),
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}
