use crate::input::{
    InputAction, InputMode, ModalInput, disable_mouse, enable_mouse, scroll_terminal,
};
use crate::plan::Stage;
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
    Skipped,
    Failed,
}

pub struct Dashboard {
    terminal: DefaultTerminal,
    collection: String,
    stages: Vec<Stage>,
    stage_status: Vec<StepStatus>,
    task_status: Vec<Vec<StepStatus>>,
    active: usize,
    parser: vt100::Parser,
    input: ModalInput,
}

impl Dashboard {
    pub fn new(collection: &str, stages: &[Stage], completed: &[bool]) -> Result<Self, String> {
        let terminal = ratatui::try_init().map_err(|error| error.to_string())?;
        enable_mouse()?;
        let stage_status = stages
            .iter()
            .zip(completed)
            .map(|(_, done)| {
                if *done {
                    StepStatus::Complete
                } else {
                    StepStatus::Pending
                }
            })
            .collect();
        let task_status = stages
            .iter()
            .zip(completed)
            .map(|(stage, done)| {
                vec![
                    if *done {
                        StepStatus::Complete
                    } else {
                        StepStatus::Pending
                    };
                    stage.tasks.len()
                ]
            })
            .collect();
        Ok(Self {
            terminal,
            collection: collection.to_owned(),
            stages: stages.to_vec(),
            stage_status,
            task_status,
            active: 0,
            parser: vt100::Parser::new(24, 120, 500),
            input: ModalInput::new(),
        })
    }

    pub fn run_module(
        &mut self,
        installer: &Path,
        forwarded: &[String],
        stage_index: usize,
        progress_file: &Path,
        tasks: &[String],
        config_adoption_allowed: bool,
        applied_tasks: String,
    ) -> Result<(bool, Option<i32>), String> {
        let stage = self
            .stages
            .get(stage_index)
            .ok_or("unknown dashboard stage")?;
        let mut command = CommandBuilder::new("bash");
        command.arg(installer);
        for argument in forwarded {
            command.arg(argument);
        }
        command.arg("--only");
        command.arg(&stage.id);
        if !tasks.is_empty() {
            command.arg("--tasks");
            command.arg(tasks.join(","));
        }
        command.env("MISTBORN_EMBEDDED_TERMINAL", "1");
        command.env(
            "MISTBORN_RUNNER_BINARY",
            std::env::current_exe().map_err(|error| error.to_string())?,
        );
        command.env("MISTBORN_PROGRESS_FILE", progress_file);
        command.env("MISTBORN_PROGRESS_STAGE", &stage.id);
        command.env(
            "MISTBORN_CONFIG_ADOPTION_ALLOWED",
            if config_adoption_allowed { "1" } else { "0" },
        );
        command.env("MISTBORN_CONFIG_APPLIED_TASKS", applied_tasks);
        self.run_child(stage_index, command, progress_file)
    }

    pub fn wait_for_exit(&mut self, home_available: bool) -> Result<bool, String> {
        self.input.navigation();
        let output = self.parser.screen().contents();
        let mode = self.input.mode();
        self.terminal
            .draw(|frame| {
                draw(
                    frame,
                    &self.collection,
                    &self.stages,
                    &self.stage_status,
                    &self.task_status,
                    self.active,
                    &output,
                    mode,
                    true,
                    home_available,
                )
            })
            .map_err(|error| error.to_string())?;
        loop {
            if event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
                let event = event::read().map_err(|error| error.to_string())?;
                if let Event::Key(key) = &event
                    && key.kind == KeyEventKind::Press
                    && home_available
                    && matches!(key.code, KeyCode::Char('h') | KeyCode::Home)
                {
                    return Ok(true);
                }
                match self.input.handle(&event) {
                    InputAction::Quit => return Ok(false),
                    action @ (InputAction::ScrollUp(_) | InputAction::ScrollDown(_)) => {
                        scroll_terminal(&mut self.parser, action);
                        let output = self.parser.screen().contents();
                        let mode = self.input.mode();
                        self.terminal
                            .draw(|frame| {
                                draw(
                                    frame,
                                    &self.collection,
                                    &self.stages,
                                    &self.stage_status,
                                    &self.task_status,
                                    self.active,
                                    &output,
                                    mode,
                                    true,
                                    home_available,
                                )
                            })
                            .map_err(|error| error.to_string())?;
                    }
                    _ => {}
                }
            }
        }
    }

    fn run_child(
        &mut self,
        stage_index: usize,
        command: CommandBuilder,
        progress_file: &Path,
    ) -> Result<(bool, Option<i32>), String> {
        self.active = stage_index;
        self.input.navigation();
        self.stage_status[self.active] = StepStatus::Running;
        self.task_status[self.active].fill(StepStatus::Pending);
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

        let mut seen_events = 0;
        loop {
            ingest_events(
                progress_file,
                &mut seen_events,
                &self.stages,
                &mut self.stage_status,
                &mut self.task_status,
                &mut self.active,
            )?;
            while let Ok(bytes) = receiver.try_recv() {
                self.parser.process(&bytes);
            }
            let output = self.parser.screen().contents();
            let collection = &self.collection;
            let stages = &self.stages;
            let stage_status = &self.stage_status;
            let task_status = &self.task_status;
            let active = self.active;
            let mode = self.input.mode();
            self.terminal
                .draw(|frame| {
                    draw(
                        frame,
                        collection,
                        stages,
                        stage_status,
                        task_status,
                        active,
                        &output,
                        mode,
                        false,
                        false,
                    )
                })
                .map_err(|error| error.to_string())?;

            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                while let Ok(bytes) = receiver.try_recv() {
                    self.parser.process(&bytes);
                }
                ingest_events(
                    progress_file,
                    &mut seen_events,
                    &self.stages,
                    &mut self.stage_status,
                    &mut self.task_status,
                    &mut self.active,
                )?;
                self.stage_status[self.active] = if status.success() {
                    StepStatus::Complete
                } else {
                    StepStatus::Failed
                };
                if status.success() {
                    for task in &mut self.task_status[self.active] {
                        if *task == StepStatus::Pending {
                            *task = StepStatus::Skipped;
                        }
                    }
                } else {
                    let mut marked_failure = false;
                    for task in &mut self.task_status[self.active] {
                        if *task == StepStatus::Running {
                            *task = StepStatus::Failed;
                            marked_failure = true;
                        }
                    }
                    if !marked_failure
                        && let Some(task) = self.task_status[self.active]
                            .iter_mut()
                            .find(|task| **task == StepStatus::Pending)
                    {
                        *task = StepStatus::Failed;
                    }
                }
                let output = self.parser.screen().contents();
                self.terminal
                    .draw(|frame| {
                        draw(
                            frame,
                            &self.collection,
                            &self.stages,
                            &self.stage_status,
                            &self.task_status,
                            self.active,
                            &output,
                            self.input.mode(),
                            false,
                            false,
                        )
                    })
                    .map_err(|error| error.to_string())?;
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
        disable_mouse();
        let _ = ratatui::try_restore();
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    frame: &mut Frame,
    collection: &str,
    stages: &[Stage],
    stage_status: &[StepStatus],
    task_status: &[Vec<StepStatus>],
    active: usize,
    terminal_output: &str,
    mode: InputMode,
    finished: bool,
    home_available: bool,
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

    let items: Vec<ListItem> = stages
        .iter()
        .enumerate()
        .map(|(index, stage)| {
            let (marker, color) = match stage_status[index] {
                StepStatus::Pending => ("○", Color::DarkGray),
                StepStatus::Running => ("▶", Color::Cyan),
                StepStatus::Complete => ("✓", Color::Green),
                StepStatus::Skipped => ("–", Color::DarkGray),
                StepStatus::Failed => ("✗", Color::Red),
            };
            let style = if index == active {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            ListItem::new(format!(" {marker} {}", stage.title)).style(style)
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
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(top[1]);
    frame.render_widget(
        Paragraph::new(stages[active].help.as_str())
            .wrap(Wrap { trim: true })
            .block(Block::default().title(" Help ").borders(Borders::ALL)),
        right[0],
    );
    let tasks: Vec<ListItem> = stages[active]
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let (marker, color) = match task_status[active][index] {
                StepStatus::Pending => ("○", Color::DarkGray),
                StepStatus::Running => ("▶", Color::Cyan),
                StepStatus::Complete => ("✓", Color::Green),
                StepStatus::Skipped => ("–", Color::DarkGray),
                StepStatus::Failed => ("✗", Color::Red),
            };
            ListItem::new(format!(" {marker} {}", task.title)).style(Style::default().fg(color))
        })
        .collect();
    frame.render_widget(
        List::new(tasks).block(
            Block::default()
                .title(" Stage tasks ")
                .borders(Borders::ALL),
        ),
        right[1],
    );
    let terminal_title = match mode {
        InputMode::Navigation => " Terminal · NAVIGATION ",
        InputMode::Insert => " Terminal · INSERT ",
    };
    frame.render_widget(
        Paragraph::new(terminal_output.to_owned())
            .wrap(Wrap { trim: false })
            .block(Block::default().title(terminal_title).borders(Borders::ALL)),
        body[1],
    );
    let total_weight: u64 = stages
        .iter()
        .flat_map(|stage| &stage.tasks)
        .map(|task| u64::from(task.weight))
        .sum();
    let completed_weight: u64 = stages
        .iter()
        .enumerate()
        .flat_map(|(stage_index, stage)| {
            stage
                .tasks
                .iter()
                .enumerate()
                .filter_map(move |(task_index, task)| {
                    matches!(
                        task_status[stage_index][task_index],
                        StepStatus::Complete | StepStatus::Skipped
                    )
                    .then_some(u64::from(task.weight))
                })
        })
        .sum();
    let ratio = if total_weight == 0 {
        stage_status
            .iter()
            .filter(|status| **status == StepStatus::Complete)
            .count() as f64
            / stages.len().max(1) as f64
    } else {
        completed_weight as f64 / total_weight as f64
    };
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title(if finished && home_available {
                        " Complete · H/Home menu · ↑/↓ scroll · q/Esc exit "
                    } else if finished {
                        " Complete · ↑/↓ scroll · q/Esc exit "
                    } else if mode == InputMode::Insert {
                        " INSERT · keys go to installer · Esc navigation "
                    } else {
                        " NAV · i/Enter insert · ↑/↓ scroll · q/Esc quit "
                    })
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .ratio(ratio)
            .label(format!(
                "{completed_weight}/{total_weight} · {:.0}%",
                ratio * 100.0
            )),
        outer[1],
    );
}

fn ingest_events(
    path: &Path,
    seen: &mut usize,
    stages: &[Stage],
    stage_status: &mut [StepStatus],
    task_status: &mut [Vec<StepStatus>],
    active: &mut usize,
) -> Result<(), String> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let lines: Vec<&str> = contents.lines().collect();
    for line in lines.iter().skip(*seen) {
        let mut fields = line.split('\t');
        let (Some(stage_id), Some(task_id), Some(state)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(stage_index) = stages.iter().position(|stage| stage.id == stage_id) else {
            continue;
        };
        let Some(task_index) = stages[stage_index]
            .tasks
            .iter()
            .position(|task| task.id == task_id)
        else {
            continue;
        };
        let status = match state {
            "started" => StepStatus::Running,
            "completed" => StepStatus::Complete,
            "skipped" => StepStatus::Skipped,
            "failed" => StepStatus::Failed,
            _ => continue,
        };
        *active = stage_index;
        stage_status[stage_index] = StepStatus::Running;
        task_status[stage_index][task_index] = status;
        if task_status[stage_index]
            .iter()
            .all(|status| matches!(status, StepStatus::Complete | StepStatus::Skipped))
        {
            stage_status[stage_index] = StepStatus::Complete;
        }
    }
    *seen = lines.len();
    Ok(())
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
