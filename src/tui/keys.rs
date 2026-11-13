//! Maps key presses onto `App` transitions.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::tui::app::{Action, App};

const PAGE_LINES: i32 = 10;

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c' | 'd') if ctrl => Action::Quit,
        KeyCode::Char('j') if ctrl => newline(app),
        KeyCode::Enter if inserts_newline(key.modifiers) => newline(app),
        KeyCode::Enter => app.submit(),
        KeyCode::Char(c) if !ctrl => edit(app, |a| a.insert_char(c)),
        KeyCode::Backspace => edit(app, App::backspace),
        KeyCode::PageUp => edit(app, |a| a.scroll_by(PAGE_LINES)),
        KeyCode::PageDown => edit(app, |a| a.scroll_by(-PAGE_LINES)),
        _ => Action::None,
    }
}

/// Alt+Enter is the fallback for terminals that cannot report Shift+Enter.
fn inserts_newline(modifiers: KeyModifiers) -> bool {
    modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::ALT)
}

fn newline(app: &mut App) -> Action {
    edit(app, App::newline)
}

fn edit(app: &mut App, change: impl FnOnce(&mut App)) -> Action {
    change(app);
    Action::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Action {
        handle_key(app, key(code, modifiers))
    }

    #[test]
    fn enter_submits_and_shift_enter_adds_a_line() {
        let mut app = App::new("m");
        press(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
        press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        press(&mut app, KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(app.input, "a\nb");
        let action = press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(action, Action::Submit("a\nb".into()));
    }

    #[test]
    fn ctrl_c_quits() {
        let mut app = App::new("m");
        assert_eq!(
            press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL),
            Action::Quit
        );
    }

    #[test]
    fn page_keys_scroll_history() {
        let mut app = App::new("m");
        press(&mut app, KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.scroll_up, 10);
        press(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.scroll_up, 0);
    }

    #[test]
    fn key_release_is_ignored() {
        let mut app = App::new("m");
        let mut release = key(KeyCode::Char('x'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        handle_key(&mut app, release);
        assert!(app.input.is_empty());
    }
}
