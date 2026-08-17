//! Rust → BEAM log bridge.
//!
//! Implements `log::Log` and forwards all `log::*` calls (from this crate and
//! libkrebs) as `{:krebs_log, level, target, message}` messages to a registered
//! Elixir pid (typically `Krebs.LogBuffer`).
//!
//! Call `init(pid)` from the `nif_log_init` NIF once the LogBuffer GenServer
//! is running. Subsequent calls replace the target pid; the old forwarding
//! thread exits when its channel closes.

use log::{Level, Log, Metadata, Record};
use rustler::{Encoder, LocalPid, OwnedEnv};
use std::sync::{mpsc, LazyLock, Mutex};

// ── Entry type ────────────────────────────────────────────────────────────────

struct LogEntry {
    level: Level,
    target: String,
    message: String,
}

// ── Global sender ─────────────────────────────────────────────────────────────

static SENDER: LazyLock<Mutex<Option<mpsc::SyncSender<LogEntry>>>> =
    LazyLock::new(|| Mutex::new(None));

// ── Logger impl ───────────────────────────────────────────────────────────────

pub static LOGGER: NifLogger = NifLogger;

pub struct NifLogger;

impl Log for NifLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(guard) = SENDER.lock() {
            if let Some(sender) = guard.as_ref() {
                let _ = sender.send(LogEntry {
                    level: record.level(),
                    target: record.target().to_string(),
                    message: record.args().to_string(),
                });
            }
        }
    }

    fn flush(&self) {}
}

// ── Level → Elixir atom ───────────────────────────────────────────────────────

fn level_atom(level: Level) -> rustler::Atom {
    match level {
        Level::Error => crate::atoms::error(),
        Level::Warn  => crate::atoms::warning(),
        Level::Info  => crate::atoms::info(),
        Level::Debug => crate::atoms::debug(),
        Level::Trace => crate::atoms::trace(),
    }
}

// ── Init ──────────────────────────────────────────────────────────────────────

/// Wire up the Rust log crate to send events to `pid` as BEAM messages.
///
/// Spawns a forwarding thread. Safe to call multiple times; each call
/// replaces the target pid (the old thread exits when its channel closes).
pub fn init(pid: LocalPid) {
    let (tx, rx) = mpsc::sync_channel::<LogEntry>(256);

    {
        let mut guard = SENDER.lock().unwrap();
        *guard = Some(tx);
    }

    std::thread::spawn(move || {
        let mut env = OwnedEnv::new();
        for entry in rx {
            let atom = level_atom(entry.level);
            let target = entry.target;
            let message = entry.message;
            let _ = env.send_and_clear(&pid, |e| {
                (
                    crate::atoms::krebs_log(),
                    atom,
                    target.as_str(),
                    message.as_str(),
                )
                    .encode(e)
            });
        }
    });

    // set_logger only succeeds once; SetLoggerError on subsequent calls is ignored.
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Trace);
}
