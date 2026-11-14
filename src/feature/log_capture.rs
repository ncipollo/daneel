//! Test-only helper: collects what the current test thread logs into a string.
//! One global subscriber writes to a per-thread buffer, because per-test
//! subscribers race on tracing's shared callsite cache when tests run in parallel.

use std::cell::RefCell;
use std::io;
use std::sync::Once;

use tracing_subscriber::fmt::MakeWriter;

thread_local! {
    static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

static INSTALL: Once = Once::new();

pub struct Capture;

impl Capture {
    pub fn text(&self) -> String {
        BUFFER.with(|buffer| String::from_utf8_lossy(&buffer.borrow()).into_owned())
    }
}

struct ThreadWriter;

impl io::Write for ThreadWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        BUFFER.with(|buffer| buffer.borrow_mut().extend_from_slice(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for ThreadWriter {
    type Writer = ThreadWriter;

    fn make_writer(&'a self) -> ThreadWriter {
        ThreadWriter
    }
}

/// Starts a fresh capture for the calling thread.
pub fn capture() -> Capture {
    INSTALL.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(ThreadWriter)
            .with_ansi(false)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
    BUFFER.with(|buffer| buffer.borrow_mut().clear());
    Capture
}
