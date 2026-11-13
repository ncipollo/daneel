//! Pure UI state. No terminal access, so every transition is unit-testable.

use crate::feature::events::{Event, EventKind};

const RESULT_PREVIEW_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    User(String),
    Assistant(String),
    Tool(String),
    Error(String),
}

/// What the runtime should do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Submit(String),
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub entries: Vec<Entry>,
    pub input: String,
    pub model: String,
    pub busy: bool,
    /// Lines scrolled up from the bottom of the history; 0 follows new output.
    pub scroll_up: u16,
}

impl App {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            entries: Vec::new(),
            input: String::new(),
            model: model.into(),
            busy: false,
            scroll_up: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.push(c);
    }

    pub fn newline(&mut self) {
        self.input.push('\n');
    }

    pub fn backspace(&mut self) {
        self.input.pop();
    }

    pub fn scroll_by(&mut self, up: i32) {
        let next = i32::from(self.scroll_up).saturating_add(up).max(0);
        self.scroll_up = u16::try_from(next).unwrap_or(u16::MAX);
    }

    /// Submits the input unless it is blank or a turn is already running.
    pub fn submit(&mut self) -> Action {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.busy {
            return Action::None;
        }
        self.input.clear();
        self.busy = true;
        self.scroll_up = 0;
        self.entries.push(Entry::User(text.clone()));
        Action::Submit(text)
    }

    pub fn apply(&mut self, event: Event) {
        match event.kind {
            EventKind::ToolCall { name, args } => {
                self.entries.push(Entry::Tool(format!("{name} {args}")));
            }
            EventKind::ToolResult { output, .. } => {
                self.entries
                    .push(Entry::Tool(format!("  -> {}", preview(&output))));
            }
            EventKind::Message { text } => self.entries.push(Entry::Assistant(text)),
            EventKind::Done => self.busy = false,
            EventKind::Error { message } => {
                self.entries.push(Entry::Error(message));
                self.busy = false;
            }
        }
    }
}

fn preview(output: &str) -> String {
    let first = output.lines().next().unwrap_or("");
    if first.chars().count() > RESULT_PREVIEW_CHARS {
        let cut: String = first.chars().take(RESULT_PREVIEW_CHARS).collect();
        format!("{cut}...")
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(app: &mut App, text: &str) {
        text.chars().for_each(|c| app.insert_char(c));
    }

    #[test]
    fn submit_records_user_entry_and_marks_busy() {
        let mut app = App::new("m");
        typed(&mut app, " hi ");
        assert_eq!(app.submit(), Action::Submit("hi".into()));
        assert_eq!(app.entries, vec![Entry::User("hi".into())]);
        assert!(app.busy && app.input.is_empty());
    }

    #[test]
    fn blank_or_busy_submit_is_ignored() {
        let mut app = App::new("m");
        typed(&mut app, "  ");
        assert_eq!(app.submit(), Action::None);
        typed(&mut app, "x");
        app.busy = true;
        assert_eq!(app.submit(), Action::None);
        assert_eq!(app.input, "  x");
    }

    #[test]
    fn newline_and_backspace_edit_the_input() {
        let mut app = App::new("m");
        typed(&mut app, "a");
        app.newline();
        typed(&mut app, "b");
        assert_eq!(app.input, "a\nb");
        app.backspace();
        assert_eq!(app.input, "a\n");
    }

    #[test]
    fn scrolling_clamps_at_the_bottom() {
        let mut app = App::new("m");
        app.scroll_by(5);
        app.scroll_by(-9);
        assert_eq!(app.scroll_up, 0);
    }

    #[test]
    fn events_become_entries_and_finish_the_turn() {
        let mut app = App::new("m");
        app.busy = true;
        app.apply(Event::new(EventKind::Message { text: "yo".into() }));
        app.apply(Event::new(EventKind::Done));
        assert_eq!(app.entries, vec![Entry::Assistant("yo".into())]);
        assert!(!app.busy);
    }

    #[test]
    fn error_event_is_shown_and_unblocks_input() {
        let mut app = App::new("m");
        app.busy = true;
        app.apply(Event::new(EventKind::Error {
            message: "bad".into(),
        }));
        assert_eq!(app.entries, vec![Entry::Error("bad".into())]);
        assert!(!app.busy);
    }

    #[test]
    fn tool_results_are_previewed_on_one_line() {
        let mut app = App::new("m");
        app.apply(Event::new(EventKind::ToolResult {
            name: "t".into(),
            output: format!("{}\nsecond", "x".repeat(200)),
        }));
        let Entry::Tool(line) = &app.entries[0] else {
            panic!("expected tool entry");
        };
        assert!(line.ends_with("...") && !line.contains("second"));
    }
}
