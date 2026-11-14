//! Pure UI state. No terminal access, so every transition is unit-testable.

use crate::feature::events::{Event, EventKind};
use crate::feature::provider::summarize_error;

const RESULT_PREVIEW_CHARS: usize = 80;
pub const TICKS_PER_SECOND: u32 = 10;

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
    /// Ticks since the running turn started; drives the spinner and elapsed time.
    pub ticks: u32,
}

impl App {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            entries: Vec::new(),
            input: String::new(),
            model: model.into(),
            busy: false,
            scroll_up: 0,
            ticks: 0,
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
        self.ticks = 0;
        self.scroll_up = 0;
        self.entries.push(Entry::User(text.clone()));
        Action::Submit(text)
    }

    /// Advances the activity indicator. Does nothing while idle.
    pub fn tick(&mut self) {
        if self.busy {
            self.ticks = self.ticks.saturating_add(1);
        }
    }

    pub fn elapsed_secs(&self) -> u32 {
        self.ticks / TICKS_PER_SECOND
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
                self.entries.push(Entry::Error(summarize_error(&message)));
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
    fn ticks_only_advance_while_busy() {
        let mut app = App::new("m");
        app.tick();
        assert_eq!(app.ticks, 0);
        typed(&mut app, "hi");
        app.submit();
        (0..25).for_each(|_| app.tick());
        assert_eq!((app.ticks, app.elapsed_secs()), (25, 2));
    }

    #[test]
    fn a_new_turn_restarts_the_clock() {
        let mut app = App::new("m");
        app.busy = true;
        (0..30).for_each(|_| app.tick());
        app.apply(Event::new(EventKind::Done));
        typed(&mut app, "again");
        app.submit();
        assert_eq!(app.elapsed_secs(), 0);
    }

    #[test]
    fn error_text_is_shortened_for_display() {
        let mut app = App::new("m");
        app.apply(Event::new(EventKind::Error {
            message: "provider error: CompletionError: HttpError: connection refused".into(),
        }));
        assert_eq!(app.entries, vec![Entry::Error("connection refused".into())]);
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
