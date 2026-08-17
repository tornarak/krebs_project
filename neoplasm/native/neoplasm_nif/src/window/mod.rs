use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{mpsc::Sender, Arc, Mutex};

use rustler::LocalPid;

use crate::types::{HexRow, MemRegion, ScanStatus};

pub mod app;
pub mod events;
pub mod hex_dump_tab;
pub mod nifs;
pub mod scanner_tab;
pub mod schema_builder_tab;

pub use nifs::*;

// ---------------------------------------------------------------------------
// Shared GUI state — written by NIF calls, read by the render loop
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct GuiState {
    pub processes: Vec<(u32, String)>,
    pub results: Vec<u64>,
    pub scan_status: ScanStatus,
    pub schemas: Vec<crate::schema::SchemaInfo>,
    pub attached: Option<(u64, String)>,
    pub scan_pattern: String,
    pub scan_errors: Vec<(String, String)>,
    pub hex_rows: Vec<HexRow>,
    pub memory_layout: Vec<MemRegion>,
    pub error: Option<String>,
    /// Inferred field values for schema mode: (field_name, display_value)
    pub inferred_fields: Vec<(String, String)>,
    /// Full watch list pushed by Elixir: (addr, label, type_name, auto_refresh, bare_map, Ok(value)|Err(reason))
    pub watch_entries: Vec<(u64, String, String, bool, bool, Result<String, String>)>,
    /// Result of the most recent cast preview, pushed by Elixir.
    pub cast_preview: Option<Result<String, String>>,
    /// Configured word size in bytes (from Rekto.Serialization.get_word_size).
    pub word_size: u64,
    // Schema builder results (take-semantics — read once, then cleared)
    pub schema_validation: Vec<String>,
    pub schema_generated_code: Option<Result<String, String>>,
    pub schema_load_status: Option<String>,
}

// ---------------------------------------------------------------------------
// State shared between NIF calls and the GUI thread
// ---------------------------------------------------------------------------

pub struct WindowState {
    pub alive: AtomicBool,
    pub cmd_tx: Sender<GuiCommand>,
    pub gui_state: Arc<Mutex<GuiState>>,
    pub calls: Mutex<HashMap<u64, Sender<Vec<u8>>>>,
    pub next_token: AtomicU64,
    /// Pid of `Neoplasm.Window` GenServer — GUI events are sent here.
    pub recipient: LocalPid,
}

pub enum GuiCommand {
    Close,
}

pub static STATE: Mutex<Option<WindowState>> = Mutex::new(None);

/// Apply `f` to the live `GuiState`, if any. No-op when no window is running.
pub fn with_gui_state<F: FnOnce(&mut GuiState)>(f: F) {
    if let Ok(guard) = STATE.lock() {
        if let Some(ws) = guard.as_ref() {
            if let Ok(mut gs) = ws.gui_state.lock() {
                f(&mut gs);
            }
        }
    }
}
