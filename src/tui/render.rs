//! Draws `App` into a ratatui frame: history on top, input below, status line last.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::app::{App, Entry};

const MAX_INPUT_LINES: u16 = 8;

pub fn draw(frame: &mut Frame, app: &App) {
    let input_height = input_height(app);
    let [history, input, status] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(input_height),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_history(frame, app, history);
    draw_input(frame, app, input);
    frame.render_widget(status_bar(app), status);
}

fn input_height(app: &App) -> u16 {
    let lines = u16::try_from(app.input.split('\n').count()).unwrap_or(MAX_INPUT_LINES);
    lines.clamp(1, MAX_INPUT_LINES) + 2
}

fn draw_history(frame: &mut Frame, app: &App, area: Rect) {
    let paragraph = Paragraph::new(history_text(app)).wrap(Wrap { trim: false });
    let total = u16::try_from(paragraph.line_count(area.width)).unwrap_or(u16::MAX);
    let max_scroll = total.saturating_sub(area.height);
    let offset = max_scroll.saturating_sub(app.scroll_up.min(max_scroll));
    frame.render_widget(paragraph.scroll((offset, 0)), area);
}

fn history_text(app: &App) -> Text<'static> {
    app.entries.iter().flat_map(entry_lines).collect()
}

fn entry_lines(entry: &Entry) -> Vec<Line<'static>> {
    let (prefix, text, style) = match entry {
        Entry::User(t) => ("> ", t, Style::new().fg(Color::Cyan)),
        Entry::Assistant(t) => ("", t, Style::new()),
        Entry::Tool(t) => ("", t, Style::new().fg(Color::DarkGray)),
        Entry::Error(t) => ("error: ", t, Style::new().fg(Color::Red)),
    };
    let mut lines: Vec<Line<'static>> = text
        .split('\n')
        .map(|line| Line::styled(line.to_string(), style))
        .collect();
    if let Some(first) = lines.first_mut() {
        first.spans.insert(0, prefix.to_string().into());
        first.style = style;
    }
    lines.push(Line::default());
    lines
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(Paragraph::new(app.input.as_str()).block(block), area);
    let lines: Vec<&str> = app.input.split('\n').collect();
    let row = u16::try_from(lines.len() - 1).unwrap_or(0);
    let col = u16::try_from(lines[lines.len() - 1].chars().count()).unwrap_or(0);
    let x = (inner.x + col).min(inner.right().saturating_sub(1));
    let y = (inner.y + row).min(inner.bottom().saturating_sub(1));
    frame.set_cursor_position((x, y));
}

fn status_bar(app: &App) -> Paragraph<'static> {
    let state = if app.busy { "working..." } else { "ready" };
    let text = format!(
        " {} | {state} | Enter send, Shift+Enter newline, Ctrl+C quit",
        app.model
    );
    Paragraph::new(text).style(Style::new().add_modifier(Modifier::REVERSED))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn rendered(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn status_bar_shows_the_active_model() {
        let app = App::new("qwen3");
        assert!(rendered(&app, 80, 10).contains("qwen3"));
    }

    #[test]
    fn history_and_input_are_visible() {
        let mut app = App::new("m");
        app.entries.push(Entry::User("hello".into()));
        app.entries.push(Entry::Assistant("world".into()));
        app.input = "draft".into();
        let screen = rendered(&app, 40, 12);
        assert!(screen.contains("> hello") && screen.contains("world") && screen.contains("draft"));
    }

    #[test]
    fn newest_output_stays_in_view_when_history_overflows() {
        let mut app = App::new("m");
        for i in 0..30 {
            app.entries.push(Entry::Assistant(format!("line {i}")));
        }
        let screen = rendered(&app, 40, 10);
        assert!(screen.contains("line 29") && !screen.contains("line 0 "));
    }

    #[test]
    fn scrolling_up_reveals_older_output() {
        let mut app = App::new("m");
        for i in 0..30 {
            app.entries.push(Entry::Assistant(format!("line {i}")));
        }
        app.scroll_by(1000);
        assert!(rendered(&app, 40, 10).contains("line 0"));
    }
}
