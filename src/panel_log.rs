//! In-memory ring buffer of the panel's own tracing output.
//!
//! The log viewer shows panel events and Xray core output in separate panes.
//! Xray's stdout/stderr already has a buffer in the supervisor; this module
//! mirrors the panel's tracing stream so both can be served without touching
//! journald or adding a log file dependency.

use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use tracing_subscriber::fmt::MakeWriter;

pub type PanelLogBuffer = Arc<Mutex<VecDeque<String>>>;

const MAX_LINES: usize = 500;

pub fn buffer() -> PanelLogBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LINES)))
}

pub fn recent(buffer: &PanelLogBuffer, limit: usize) -> Vec<String> {
    let Ok(queue) = buffer.lock() else {
        return Vec::new();
    };
    let skip = queue.len().saturating_sub(limit);
    queue.iter().skip(skip).cloned().collect()
}

fn push(buffer: &PanelLogBuffer, line: &str) {
    if let Ok(mut queue) = buffer.lock() {
        if queue.len() >= MAX_LINES {
            queue.pop_front();
        }
        queue.push_back(line.to_string());
    }
}

/// `MakeWriter` for a tracing `fmt` layer. Each event gets its own writer and
/// only reaches the shared buffer when dropped, so two events formatting at
/// the same time cannot interleave halfway through a line.
#[derive(Clone)]
pub struct PanelLogMakeWriter {
    buffer: PanelLogBuffer,
}

impl PanelLogMakeWriter {
    pub fn new(buffer: PanelLogBuffer) -> Self {
        Self { buffer }
    }
}

impl<'a> MakeWriter<'a> for PanelLogMakeWriter {
    type Writer = PanelLogEventWriter;

    fn make_writer(&'a self) -> Self::Writer {
        PanelLogEventWriter {
            buffer: self.buffer.clone(),
            pending: String::new(),
        }
    }
}

pub struct PanelLogEventWriter {
    buffer: PanelLogBuffer,
    pending: String,
}

impl Write for PanelLogEventWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PanelLogEventWriter {
    fn drop(&mut self) {
        for line in self.pending.lines() {
            let line = line.trim_end();
            if !line.is_empty() {
                push(&self.buffer, line);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_keeps_the_newest_lines_and_caps_memory() {
        let buffer = buffer();
        for i in 0..(MAX_LINES + 5) {
            push(&buffer, &format!("line {i}"));
        }
        let lines = recent(&buffer, MAX_LINES + 100);
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines.first().unwrap(), "line 5");
        assert_eq!(lines.last().unwrap(), &format!("line {}", MAX_LINES + 4));
    }

    #[test]
    fn writer_flushes_complete_event_on_drop() {
        let buffer = buffer();
        {
            let mut writer = PanelLogMakeWriter::new(buffer.clone()).make_writer();
            writer
                .write_all(b"2026/10/31 21:00:00  INFO hello\nsecond line\n")
                .unwrap();
        }
        assert_eq!(
            recent(&buffer, 10),
            vec!["2026/10/31 21:00:00  INFO hello", "second line"]
        );
    }
}
