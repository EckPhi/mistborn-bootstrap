use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEventKind};
use ratatui::crossterm::{event::DisableMouseCapture, event::EnableMouseCapture, execute};
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Navigation,
    Insert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAction {
    None,
    Quit,
    Forward(KeyEvent),
    ScrollUp(usize),
    ScrollDown(usize),
}

pub struct ModalInput {
    mode: InputMode,
}

impl ModalInput {
    pub fn new() -> Self {
        Self {
            mode: InputMode::Navigation,
        }
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn navigation(&mut self) {
        self.mode = InputMode::Navigation;
    }

    pub fn handle(&mut self, event: &Event) -> InputAction {
        match event {
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => InputAction::ScrollUp(3),
                MouseEventKind::ScrollDown => InputAction::ScrollDown(3),
                _ => InputAction::None,
            },
            Event::Key(key) if key.kind == KeyEventKind::Press => match self.mode {
                InputMode::Navigation => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => InputAction::Quit,
                    KeyCode::Enter | KeyCode::Char('i') => {
                        self.mode = InputMode::Insert;
                        InputAction::None
                    }
                    KeyCode::Up | KeyCode::Char('k') => InputAction::ScrollUp(1),
                    KeyCode::Down | KeyCode::Char('j') => InputAction::ScrollDown(1),
                    KeyCode::PageUp => InputAction::ScrollUp(10),
                    KeyCode::PageDown => InputAction::ScrollDown(10),
                    _ => InputAction::None,
                },
                InputMode::Insert => {
                    if key.code == KeyCode::Esc {
                        self.mode = InputMode::Navigation;
                        InputAction::None
                    } else {
                        InputAction::Forward(*key)
                    }
                }
            },
            _ => InputAction::None,
        }
    }
}

pub fn scroll_terminal(parser: &mut vt100::Parser, action: InputAction) {
    let current = parser.screen().scrollback();
    match action {
        InputAction::ScrollUp(lines) => parser
            .screen_mut()
            .set_scrollback(current.saturating_add(lines)),
        InputAction::ScrollDown(lines) => parser
            .screen_mut()
            .set_scrollback(current.saturating_sub(lines)),
        _ => {}
    }
}

pub fn enable_mouse() -> Result<(), String> {
    execute!(io::stdout(), EnableMouseCapture).map_err(|error| error.to_string())
}

pub fn disable_mouse() {
    let _ = execute!(io::stdout(), DisableMouseCapture);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyModifiers, MouseEvent};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn navigation_is_default_and_owns_control_keys() {
        let mut input = ModalInput::new();
        assert_eq!(input.mode(), InputMode::Navigation);
        assert_eq!(input.handle(&key(KeyCode::Char('q'))), InputAction::Quit);
        assert_eq!(input.handle(&key(KeyCode::Up)), InputAction::ScrollUp(1));
    }

    #[test]
    fn insert_forwards_text_until_escape() {
        let mut input = ModalInput::new();
        assert_eq!(input.handle(&key(KeyCode::Char('i'))), InputAction::None);
        assert_eq!(input.mode(), InputMode::Insert);
        assert!(matches!(
            input.handle(&key(KeyCode::Char('q'))),
            InputAction::Forward(_)
        ));
        assert_eq!(input.handle(&key(KeyCode::Esc)), InputAction::None);
        assert_eq!(input.mode(), InputMode::Navigation);
    }

    #[test]
    fn mouse_wheel_scrolls_without_entering_insert_mode() {
        let mut input = ModalInput::new();
        let event = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(input.handle(&event), InputAction::ScrollDown(3));
        assert_eq!(input.mode(), InputMode::Navigation);
    }

    #[test]
    fn terminal_scrollback_moves_in_both_directions() {
        let mut parser = vt100::Parser::new(2, 20, 10);
        parser.process(b"one\r\ntwo\r\nthree\r\nfour");
        scroll_terminal(&mut parser, InputAction::ScrollUp(2));
        assert_eq!(parser.screen().scrollback(), 2);
        scroll_terminal(&mut parser, InputAction::ScrollDown(1));
        assert_eq!(parser.screen().scrollback(), 1);
    }
}
