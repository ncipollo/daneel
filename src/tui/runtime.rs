//! The terminal loop: reads keys, runs chat turns in the background, redraws.

use std::io;
use std::sync::Arc;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, Event as TermEvent, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::{execute, terminal};
use tokio::sync::{Mutex, mpsc};

use crate::feature::chat::{Chat, completion_events};
use crate::feature::events::{Event, EventSink};
use crate::tui::app::{Action, App};
use crate::tui::keys::handle_key;
use crate::tui::render::draw;

type SharedChat = Arc<Mutex<Chat>>;

pub async fn run(chat: Chat, model: String) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let enhanced = enable_shift_enter();
    let result = event_loop(&mut terminal, chat, model).await;
    if enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// Lets terminals that support it report Shift+Enter distinctly from Enter.
fn enable_shift_enter() -> bool {
    let supported = terminal::supports_keyboard_enhancement().unwrap_or(false);
    supported
        && execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok()
}

async fn event_loop(terminal: &mut DefaultTerminal, chat: Chat, model: String) -> io::Result<()> {
    let chat: SharedChat = Arc::new(Mutex::new(chat));
    let mut app = App::new(model);
    let mut terminal_events = spawn_terminal_reader();
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<Event>();
    loop {
        terminal.draw(|frame| draw(frame, &app))?;
        tokio::select! {
            Some(event) = terminal_events.recv() => {
                if let TermEvent::Key(key) = event {
                    match handle_key(&mut app, key) {
                        Action::Quit => return Ok(()),
                        Action::Submit(text) => spawn_turn(&chat, text, agent_tx.clone()),
                        Action::None => {}
                    }
                }
            }
            Some(event) = agent_rx.recv() => app.apply(event),
            else => return Ok(()),
        }
    }
}

fn spawn_terminal_reader() -> mpsc::UnboundedReceiver<TermEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            if tx.send(event).is_err() {
                break;
            }
        }
    });
    rx
}

fn spawn_turn(chat: &SharedChat, text: String, tx: mpsc::UnboundedSender<Event>) {
    let chat = Arc::clone(chat);
    tokio::spawn(async move {
        let sink_tx = tx.clone();
        let sink: EventSink = Arc::new(move |event| {
            let _ = sink_tx.send(event);
        });
        let result = chat.lock().await.send(&text, sink).await;
        for event in completion_events(&result) {
            let _ = tx.send(event);
        }
    });
}
