use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct CommandDashboard {
    terminal: DefaultTerminal,
    operation: String,
    command: String,
    help: &'static str,
    parser: vt100::Parser,
    status: CommandStatus,
}

#[derive(Clone, Copy)]
enum CommandStatus {
    Running,
    Complete,
    Failed,
}

impl CommandDashboard {
    pub fn new(operation: &str, command: &str) -> Result<Self, String> {
        let terminal = ratatui::try_init().map_err(|error| error.to_string())?;
        Ok(Self {
            terminal,
            operation: operation.to_owned(),
            command: command.to_owned(),
            help: command_help(operation),
            parser: vt100::Parser::new(24, 120, 500),
            status: CommandStatus::Running,
        })
    }

    pub fn run(
        &mut self,
        script: &Path,
        arguments: &[String],
        progress_file: &Path,
    ) -> Result<(bool, Option<i32>), String> {
        let mut command = CommandBuilder::new("bash");
        command.arg(script);
        for argument in arguments {
            command.arg(argument);
        }
        command.env("MISTBORN_EMBEDDED_TERMINAL", "1");
        command.env("MISTBORN_PROGRESS_FILE", progress_file);
        command.env("MISTBORN_PROGRESS_STAGE", "bootstrap");
        command.env(
            "MISTBORN_BOOTSTRAP_VERSION",
            format!("v{}", env!("CARGO_PKG_VERSION")),
        );

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| error.to_string())?;
        drop(pair.slave);
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });

        loop {
            while let Ok(bytes) = receiver.try_recv() {
                self.parser.process(&bytes);
            }
            self.draw()?;

            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                while let Ok(bytes) = receiver.try_recv() {
                    self.parser.process(&bytes);
                }
                self.status = if status.success() {
                    CommandStatus::Complete
                } else {
                    CommandStatus::Failed
                };
                self.draw()?;
                return Ok((status.success(), Some(status.exit_code() as i32)));
            }

            if event::poll(Duration::from_millis(50)).map_err(|error| error.to_string())? {
                match event::read().map_err(|error| error.to_string())? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if let Some(bytes) = encode_key(key) {
                            writer
                                .write_all(&bytes)
                                .map_err(|error| error.to_string())?;
                            writer.flush().map_err(|error| error.to_string())?;
                        }
                    }
                    Event::Resize(width, height) => {
                        pair.master
                            .resize(PtySize {
                                rows: height.saturating_sub(16).max(4),
                                cols: width.saturating_sub(4).max(20),
                                pixel_width: 0,
                                pixel_height: 0,
                            })
                            .map_err(|error| error.to_string())?;
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn wait_for_exit(&mut self) -> Result<(), String> {
        self.draw()?;
        loop {
            if event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
                if let Event::Key(key) = event::read().map_err(|error| error.to_string())? {
                    if key.kind == KeyEventKind::Press {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn draw(&mut self) -> Result<(), String> {
        let output = self.parser.screen().contents();
        let operation = self.operation.clone();
        let command = self.command.clone();
        let help = self.help;
        let status = self.status;
        self.terminal
            .draw(|frame| draw(frame, &operation, &command, help, status, &output))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl Drop for CommandDashboard {
    fn drop(&mut self) {
        let _ = ratatui::try_restore();
    }
}

fn draw(
    frame: &mut Frame,
    operation: &str,
    command: &str,
    help: &str,
    status: CommandStatus,
    terminal_output: &str,
) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(outer[0]);
    let color = match status {
        CommandStatus::Running => Color::Cyan,
        CommandStatus::Complete => Color::Green,
        CommandStatus::Failed => Color::Red,
    };
    let label = match status {
        CommandStatus::Running => "RUNNING",
        CommandStatus::Complete => "COMPLETE",
        CommandStatus::Failed => "FAILED",
    };
    frame.render_widget(
        Paragraph::new(vec![
            ratatui::text::Line::from(format!("Operation  {operation}")),
            ratatui::text::Line::from(format!("State      {label}")),
        ])
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .title(" Mistborn command ")
                .borders(Borders::ALL),
        ),
        top[0],
    );
    frame.render_widget(
        Paragraph::new(help)
            .wrap(Wrap { trim: true })
            .block(Block::default().title(" Help ").borders(Borders::ALL)),
        top[1],
    );
    frame.render_widget(
        Paragraph::new(terminal_output.to_owned())
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title(format!(" {} · press keys to interact ", command))
                    .borders(Borders::ALL),
            ),
        outer[1],
    );
    let (ratio, footer) = match status {
        CommandStatus::Running => (0.0, "Running · Ctrl-C interrupts"),
        CommandStatus::Complete => (1.0, "Complete · press any key to return"),
        CommandStatus::Failed => (1.0, "Failed · press any key to return"),
    };
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title(" Command status ")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(color).bg(Color::Black))
            .ratio(ratio)
            .label(footer),
        outer[2],
    );
}

fn command_help(operation: &str) -> &'static str {
    match operation {
        "status" => {
            "Read-only overview of installed components, service health, and Tailscale connectivity."
        }
        "doctor" => {
            "Audits Docker, Runtipi, Tailscale, rclone, and security tools. Warnings do not change configuration."
        }
        "fix" => {
            "Enables and starts installed Docker and Tailscale services. SSH and firewall settings remain unchanged."
        }
        "security-status" => {
            "Shows SSH daemon policy, firewall rules, fail2ban status, and Tailscale state."
        }
        "tailscale-status" => "Shows current Tailscale connectivity and peer status.",
        "rclone-config" => {
            "Interactive rclone configuration. Use the terminal panel below to answer prompts."
        }
        "upgrade" | "update" => {
            "Checks for the latest stable Mistborn Bootstrap release and refreshes the installed tool."
        }
        "update-runtipi" => {
            "Snapshots installed apps, then updates Runtipi core, app stores, and installed apps."
        }
        "update-apps" => {
            "Updates selected apps or all installed apps. Snapshots are created unless --no-backup is specified."
        }
        "update-core" => {
            "Updates Runtipi core. Installed apps are snapshotted unless --no-backup is specified."
        }
        "update-appstores" => "Refreshes Runtipi app-store metadata.",
        _ => {
            "Mistborn host command. Review the command output below; press any key when it finishes to return."
        }
    }
}

fn encode_key(key: ratatui::crossterm::event::KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let byte = (character.to_ascii_lowercase() as u8).checked_sub(b'a')? + 1;
            Some(vec![byte])
        }
        KeyCode::Char(character) => Some(character.to_string().into_bytes()),
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        _ => None,
    }
}
