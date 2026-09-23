use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
pub enum StepStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

pub struct Dashboard {
    terminal: DefaultTerminal,
    collection: String,
    modules: Vec<(String, StepStatus)>,
    active: usize,
    parser: vt100::Parser,
}

impl Dashboard {
    pub fn new(collection: &str, modules: &[String], completed: &[bool]) -> Result<Self, String> {
        let terminal = ratatui::try_init().map_err(|error| error.to_string())?;
        let modules = modules
            .iter()
            .zip(completed)
            .map(|(name, done)| {
                (
                    name.clone(),
                    if *done {
                        StepStatus::Complete
                    } else {
                        StepStatus::Pending
                    },
                )
            })
            .collect();
        Ok(Self {
            terminal,
            collection: collection.to_owned(),
            modules,
            active: 0,
            parser: vt100::Parser::new(24, 120, 500),
        })
    }

    pub fn run_module(
        &mut self,
        installer: &Path,
        forwarded: &[String],
        module: &str,
    ) -> Result<(bool, Option<i32>), String> {
        let mut command = CommandBuilder::new("bash");
        command.arg(installer);
        for argument in forwarded {
            command.arg(argument);
        }
        command.arg("--only");
        command.arg(module);
        command.env("MISTBORN_EMBEDDED_TERMINAL", "1");
        command.env(
            "MISTBORN_RUNNER_BINARY",
            std::env::current_exe().map_err(|error| error.to_string())?,
        );
        self.run_child(module, command)
    }

    pub fn run_host_command(
        &mut self,
        script: &Path,
        arguments: &[String],
        operation: &str,
    ) -> Result<(bool, Option<i32>), String> {
        let mut command = CommandBuilder::new("bash");
        command.arg(script);
        for argument in arguments {
            command.arg(argument);
        }
        command.env("MISTBORN_EMBEDDED_TERMINAL", "1");
        self.run_child(operation, command)
    }

    pub fn screen_contents(&self) -> String {
        self.parser.screen().contents()
    }

    fn run_child(
        &mut self,
        step: &str,
        command: CommandBuilder,
    ) -> Result<(bool, Option<i32>), String> {
        self.active = self
            .modules
            .iter()
            .position(|(name, _)| name == step)
            .ok_or_else(|| format!("unknown dashboard step: {step}"))?;
        self.modules[self.active].1 = StepStatus::Running;
        self.parser = vt100::Parser::new(24, 120, 500);

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
            let output = self.parser.screen().contents();
            let collection = &self.collection;
            let modules = &self.modules;
            let active = self.active;
            self.terminal
                .draw(|frame| draw(frame, collection, modules, active, &output))
                .map_err(|error| error.to_string())?;

            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                while let Ok(bytes) = receiver.try_recv() {
                    self.parser.process(&bytes);
                }
                self.modules[self.active].1 = if status.success() {
                    StepStatus::Complete
                } else {
                    StepStatus::Failed
                };
                let output = self.parser.screen().contents();
                self.terminal
                    .draw(|frame| {
                        draw(frame, &self.collection, &self.modules, self.active, &output)
                    })
                    .map_err(|error| error.to_string())?;
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
                        let rows = height.saturating_sub(16).max(4);
                        pair.master
                            .resize(PtySize {
                                rows,
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
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        let _ = ratatui::try_restore();
    }
}

fn draw(
    frame: &mut Frame,
    collection: &str,
    modules: &[(String, StepStatus)],
    active: usize,
    terminal_output: &str,
) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(3)])
        .split(frame.area());
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(outer[0]);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(body[0]);

    let items: Vec<ListItem> = modules
        .iter()
        .enumerate()
        .map(|(index, (name, status))| {
            let (marker, color) = match status {
                StepStatus::Pending => ("○", Color::DarkGray),
                StepStatus::Running => ("▶", Color::Cyan),
                StepStatus::Complete => ("✓", Color::Green),
                StepStatus::Failed => ("✗", Color::Red),
            };
            let style = if index == active {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            ListItem::new(format!(" {marker} {name}")).style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(format!(" Mistborn {collection} "))
                .borders(Borders::ALL),
        ),
        top[0],
    );
    frame.render_widget(
        Paragraph::new(help_for(&modules[active].0))
            .wrap(Wrap { trim: true })
            .block(Block::default().title(" Help ").borders(Borders::ALL)),
        top[1],
    );
    frame.render_widget(
        Paragraph::new(terminal_output.to_owned())
            .wrap(Wrap { trim: false })
            .block(Block::default().title(" Terminal ").borders(Borders::ALL)),
        body[1],
    );
    let completed = modules
        .iter()
        .filter(|(_, status)| *status == StepStatus::Complete)
        .count();
    let ratio = completed as f64 / modules.len().max(1) as f64;
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title(" Overall progress ")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .ratio(ratio)
            .label(format!(
                "{completed}/{} · {:.0}%",
                modules.len(),
                ratio * 100.0
            )),
        outer[1],
    );
}

fn help_for(module: &str) -> &'static str {
    match module {
        "status" => {
            "Read-only overview of installed components, service health, and Tailscale connectivity."
        }
        "doctor" => "Runs read-only diagnostics. Review each warning before choosing a repair.",
        "fix" => {
            "Enables and starts Docker and Tailscale services. SSH and firewall settings are not changed."
        }
        "update" => {
            "Updates Runtipi core, app stores, and apps. App snapshots are created before updates."
        }
        "update-apps" => {
            "Updates selected apps or all installed apps, creating snapshots by default."
        }
        "update-core" => "Updates the Runtipi core after creating app snapshots by default.",
        "tailscale" => {
            "Authenticate with the URL shown below. Exit nodes also require approval in the Tailscale admin console."
        }
        "rclone" => {
            "Answer the prompts in the terminal pane. Input is sent directly to rclone through an embedded PTY."
        }
        "security" => {
            "Keep a second SSH session open. Confirm key-based or Tailscale SSH access before disconnecting."
        }
        "docker" => "Installs and enables Docker Engine.",
        "runtipi" => {
            "Installs Runtipi using its official installer when it is not already present."
        }
        "zsh" => "Installs Zsh, Oh My Zsh, and Powerlevel10k for the selected target user.",
        _ => "Installer output appears below. Ctrl-C is forwarded to the active process.",
    }
}

fn encode_key(key: KeyEvent) -> Option<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_input_is_encoded_for_the_pty() {
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(vec![b'\r'])
        );
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(vec![3])
        );
    }

    #[test]
    fn child_process_receives_interactive_pty_input() {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 10,
                cols: 40,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new("bash");
        command.arg("-c");
        command.arg("read -r value; test \"$value\" = mistborn");
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut writer = pair.master.take_writer().unwrap();
        writer.write_all(b"mistborn\r").unwrap();
        writer.flush().unwrap();
        assert!(child.wait().unwrap().success());
    }
}
