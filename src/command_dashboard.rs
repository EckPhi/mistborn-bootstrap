use crate::input::{
    InputAction, InputMode, ModalInput, disable_mouse, enable_mouse, scroll_terminal,
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};
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
    input: ModalInput,
}

#[derive(Clone, Copy)]
enum CommandStatus {
    Running,
    Complete,
    Failed,
}

impl CommandDashboard {
    pub fn select_command() -> Result<Option<&'static str>, String> {
        let terminal = ratatui::try_init().map_err(|error| error.to_string())?;
        enable_mouse()?;
        let mut menu = CommandMenu { terminal };
        let mut selected = 0;
        loop {
            menu.draw(selected)?;
            if !event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
                continue;
            }
            match event::read().map_err(|error| error.to_string())? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.checked_sub(1).unwrap_or(MENU_COMMANDS.len() - 1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1) % MENU_COMMANDS.len();
                    }
                    KeyCode::Enter => {
                        return Ok(MENU_COMMANDS[selected].operation);
                    }
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        selected = selected.checked_sub(1).unwrap_or(MENU_COMMANDS.len() - 1);
                    }
                    MouseEventKind::ScrollDown => selected = (selected + 1) % MENU_COMMANDS.len(),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    pub fn new(operation: &str, command: &str) -> Result<Self, String> {
        let terminal = ratatui::try_init().map_err(|error| error.to_string())?;
        enable_mouse()?;
        Ok(Self {
            terminal,
            operation: operation.to_owned(),
            command: command.to_owned(),
            help: command_help(operation),
            parser: vt100::Parser::new(24, 120, 500),
            status: CommandStatus::Running,
            input: ModalInput::new(),
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
                    event @ (Event::Key(_) | Event::Mouse(_)) => match self.input.handle(&event) {
                        InputAction::Quit => {
                            child.kill().map_err(|error| error.to_string())?;
                            return Ok((false, None));
                        }
                        InputAction::Forward(key) => {
                            if let Some(bytes) = encode_key(key) {
                                writer
                                    .write_all(&bytes)
                                    .map_err(|error| error.to_string())?;
                                writer.flush().map_err(|error| error.to_string())?;
                            }
                        }
                        action @ (InputAction::ScrollUp(_) | InputAction::ScrollDown(_)) => {
                            scroll_terminal(&mut self.parser, action);
                        }
                        InputAction::None => {}
                    },
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

    pub fn wait_for_exit(&mut self) -> Result<bool, String> {
        self.input.navigation();
        self.draw()?;
        loop {
            if event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
                let event = event::read().map_err(|error| error.to_string())?;
                if let Event::Key(key) = &event
                    && key.kind == KeyEventKind::Press
                    && matches!(key.code, KeyCode::Char('h') | KeyCode::Home)
                {
                    return Ok(true);
                }
                match self.input.handle(&event) {
                    InputAction::Quit => return Ok(false),
                    action @ (InputAction::ScrollUp(_) | InputAction::ScrollDown(_)) => {
                        scroll_terminal(&mut self.parser, action);
                        self.draw()?;
                    }
                    _ => {}
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
        let mode = self.input.mode();
        self.terminal
            .draw(|frame| draw(frame, &operation, &command, help, status, mode, &output))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

struct CommandMenu {
    terminal: DefaultTerminal,
}

impl CommandMenu {
    fn draw(&mut self, selected: usize) -> Result<(), String> {
        self.terminal
            .draw(|frame| {
                let areas = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(8),
                        Constraint::Length(3),
                    ])
                    .split(frame.area());
                frame.render_widget(
                    Paragraph::new("Choose a Mistborn command")
                        .style(
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )
                        .block(Block::default().borders(Borders::ALL).title(" Mistborn ")),
                    areas[0],
                );
                let columns = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
                    .split(areas[1]);
                let commands = MENU_COMMANDS
                    .iter()
                    .enumerate()
                    .map(|(index, command)| {
                        let marker = if index == selected { "▶" } else { " " };
                        let style = if index == selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(format!(" {marker} {}", command.label)).style(style)
                    })
                    .collect::<Vec<_>>();
                frame.render_widget(
                    List::new(commands)
                        .block(Block::default().borders(Borders::ALL).title(" Commands ")),
                    columns[0],
                );
                frame.render_widget(
                    Paragraph::new(MENU_COMMANDS[selected].help)
                        .wrap(Wrap { trim: true })
                        .block(Block::default().borders(Borders::ALL).title(" Help ")),
                    columns[1],
                );
                frame.render_widget(
                    Paragraph::new("↑/↓ or j/k move · Enter run · q/Esc quit")
                        .block(Block::default().borders(Borders::ALL).title(" Controls ")),
                    areas[2],
                );
            })
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl Drop for CommandMenu {
    fn drop(&mut self) {
        disable_mouse();
        let _ = ratatui::try_restore();
    }
}

struct MenuCommand {
    label: &'static str,
    operation: Option<&'static str>,
    help: &'static str,
}

const MENU_COMMANDS: &[MenuCommand] = &[
    MenuCommand {
        label: "Status",
        operation: Some("status"),
        help: "Read-only overview of installed components, service health, and Tailscale connectivity.",
    },
    MenuCommand {
        label: "Doctor",
        operation: Some("doctor"),
        help: "Audit Docker, Runtipi, Tailscale, rclone, and security tools for common issues.",
    },
    MenuCommand {
        label: "Fix services",
        operation: Some("fix"),
        help: "Enable and start installed Docker and Tailscale services. SSH and firewall settings are left unchanged.",
    },
    MenuCommand {
        label: "Security status",
        operation: Some("security-status"),
        help: "Review SSH daemon policy, firewall rules, fail2ban, and Tailscale status.",
    },
    MenuCommand {
        label: "Tailscale status",
        operation: Some("tailscale-status"),
        help: "Show current Tailscale connectivity and peer status.",
    },
    MenuCommand {
        label: "Configure rclone",
        operation: Some("rclone-config"),
        help: "Open rclone's interactive configuration. Answers are forwarded to its terminal session.",
    },
    MenuCommand {
        label: "Upgrade Mistborn",
        operation: Some("upgrade"),
        help: "Check for the latest stable Mistborn Bootstrap release and refresh the installed tool.",
    },
    MenuCommand {
        label: "Update Runtipi",
        operation: Some("update-runtipi"),
        help: "Snapshot installed apps, then update Runtipi core, app stores, and apps.",
    },
    MenuCommand {
        label: "Update apps",
        operation: Some("update-apps"),
        help: "Update all installed apps with snapshots. You can pass app references or --no-backup to mistborn update-apps.",
    },
    MenuCommand {
        label: "Update core",
        operation: Some("update-core"),
        help: "Update Runtipi core (latest by default), with app snapshots unless --no-backup is passed.",
    },
    MenuCommand {
        label: "Update app stores",
        operation: Some("update-appstores"),
        help: "Refresh Runtipi app-store metadata.",
    },
    MenuCommand {
        label: "Quit",
        operation: None,
        help: "Return to the shell without running a command.",
    },
];

impl Drop for CommandDashboard {
    fn drop(&mut self) {
        disable_mouse();
        let _ = ratatui::try_restore();
    }
}

fn draw(
    frame: &mut Frame,
    operation: &str,
    command: &str,
    help: &str,
    status: CommandStatus,
    mode: InputMode,
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
    let terminal_title = match mode {
        InputMode::Navigation => format!(" {command} · NAVIGATION "),
        InputMode::Insert => format!(" {command} · INSERT "),
    };
    frame.render_widget(
        Paragraph::new(terminal_output.to_owned())
            .wrap(Wrap { trim: false })
            .block(Block::default().title(terminal_title).borders(Borders::ALL)),
        outer[1],
    );
    let (ratio, footer) = match status {
        CommandStatus::Running => (
            0.0,
            match mode {
                InputMode::Navigation => "NAV · i/Enter insert · ↑/↓ scroll · q/Esc quit",
                InputMode::Insert => "INSERT · keys go to command · Esc navigation",
            },
        ),
        CommandStatus::Complete => (1.0, "H/Home menu · ↑/↓ scroll · q/Esc exit"),
        CommandStatus::Failed => (1.0, "H/Home menu · ↑/↓ scroll · q/Esc exit"),
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
