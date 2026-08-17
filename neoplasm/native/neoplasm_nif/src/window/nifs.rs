use std::collections::HashMap;
use std::sync::atomic::{self, AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use log::{debug, error, info};
use rustler::{Atom, Decoder, Encoder, Env, LocalPid, OwnedEnv, Term};
use winit::platform::x11::EventLoopBuilderExtX11;

use super::app::NeoplasmApp;
use super::{with_gui_state, GuiCommand, GuiState, WindowState, STATE};
use crate::types::{HexRow, MemKind, MemRegion, ScanStatus};

#[rustler::nif]
pub fn open_window(recipient: LocalPid) -> Atom {
    info!("open_window called");

    let gui_state = {
        let mut guard = STATE.lock().unwrap();
        if let Some(ws) = guard.as_mut() {
            if ws.alive.load(atomic::Ordering::SeqCst) {
                // Window is alive but the Elixir side restarted — just update the recipient.
                info!("open_window: window alive, updating recipient to new pid");
                ws.recipient = recipient;
                return crate::atoms::ok();
            }
        }
        // Dead or absent — build fresh state and fall through to spawn.
        let gui_state = Arc::new(Mutex::new(GuiState::default()));
        let (cmd_tx, _) = std::sync::mpsc::channel::<GuiCommand>();
        *guard = Some(WindowState {
            alive: AtomicBool::new(true),
            cmd_tx,
            gui_state: Arc::clone(&gui_state),
            calls: Mutex::new(HashMap::new()),
            next_token: AtomicU64::new(0),
            recipient,
        });
        gui_state
    };

    std::thread::spawn(move || {
        info!("[neoplasm] window thread starting");
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("NEOPLASM")
                .with_inner_size([1024.0, 768.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        };
        let result = eframe::run_native(
            "NEOPLASM",
            options,
            Box::new(|cc| Ok(Box::new(NeoplasmApp::new(cc, gui_state)))),
        );
        error!("[neoplasm] run_native returned: {:?}", result);
        {
            if let Ok(guard) = STATE.lock() {
                if let Some(ws) = guard.as_ref() {
                    ws.alive.store(false, atomic::Ordering::SeqCst);
                }
            }
        }
        info!("[neoplasm] sending window_closed to Elixir");
        let mut env = OwnedEnv::new();
        let _ = env.send_and_clear(&recipient, |env| {
            use rustler::types::tuple::make_tuple;
            make_tuple(
                env,
                &[
                    crate::atoms::neoplasm().encode(env),
                    crate::atoms::window_closed().encode(env),
                ],
            )
        });
        info!("[neoplasm] window_closed sent");
    });

    crate::atoms::ok()
}

#[rustler::nif]
pub fn close_window() -> Atom {
    info!("close_window called");
    if let Ok(guard) = STATE.lock() {
        if let Some(ws) = guard.as_ref() {
            let _ = ws.cmd_tx.send(GuiCommand::Close);
        }
    }
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_processes(processes: Vec<(u32, String)>) -> Atom {
    info!("set_processes: {} entries", processes.len());
    let mut processes = processes;
    processes.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    with_gui_state(|gs| gs.processes = processes);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_scan_status<'a>(env: Env<'a>, status: Term<'a>) -> Atom {
    let scan_status = if status == crate::atoms::idle().encode(env) {
        ScanStatus::Idle
    } else if status == crate::atoms::scanning().encode(env) {
        ScanStatus::Scanning
    } else if let Ok((tag, count)) = <(Atom, usize)>::decode(status) {
        if tag == crate::atoms::done() {
            ScanStatus::Done(count)
        } else {
            error!("set_scan_status: unrecognised tag");
            return crate::atoms::error();
        }
    } else {
        error!("set_scan_status: could not decode term");
        return crate::atoms::error();
    };
    info!("set_scan_status: {:?}", scan_status);
    with_gui_state(|gs| gs.scan_status = scan_status);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn push_results(addrs: Vec<u64>) -> Atom {
    debug!("push_results: {} addrs", addrs.len());
    with_gui_state(|gs| {
        for addr in addrs {
            let pos = gs.results.binary_search(&addr).unwrap_or_else(|i| i);
            gs.results.insert(pos, addr);
        }
    });
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_schemas(env: Env, schemas: Vec<(Atom, crate::schema::SchemaInfoDecoded)>) -> Atom {
    info!("set_schemas: {} schemas", schemas.len());
    let schemas: Vec<_> = schemas
        .into_iter()
        .map(|(module_atom, info)| {
            let name = module_atom
                .encode(env)
                .atom_to_string()
                .map(|s| s.trim_start_matches("Elixir.").to_string())
                .unwrap_or_default();
            crate::schema::SchemaInfo::new(module_atom, name, info)
        })
        .collect();
    with_gui_state(|gs| gs.schemas = schemas);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_attached(attached: Option<(u64, String)>, attach_error: Option<String>) {
    info!(
        "set_attached: attached={:?} error={:?}",
        attached, attach_error
    );
    with_gui_state(|gs| {
        gs.attached = attached;
        gs.error = attach_error;
    });
}

#[rustler::nif]
pub fn clear_results() -> Atom {
    info!("clear_results");
    with_gui_state(|gs| gs.results.clear());
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_scan_pattern(hex: String) -> Atom {
    debug!("set_scan_pattern: {} chars", hex.len());
    with_gui_state(|gs| gs.scan_pattern = hex);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_scan_errors(errors: Vec<(String, String)>) -> Atom {
    debug!("set_scan_errors: {} errors", errors.len());
    with_gui_state(|gs| gs.scan_errors = errors);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_hex_dump(rows: Vec<(u64, Vec<u8>, Vec<String>)>) -> Atom {
    debug!("set_hex_dump: {} rows", rows.len());
    let hex_rows: Vec<HexRow> = rows
        .into_iter()
        .map(|(addr, bytes, annotations)| HexRow {
            addr,
            bytes,
            annotations,
        })
        .collect();
    with_gui_state(|gs| gs.hex_rows = hex_rows);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_memory_layout(regions: Vec<(u64, u64, rustler::Atom)>) -> Atom {
    debug!("set_memory_layout: {} regions", regions.len());
    let layout: Vec<MemRegion> = regions
        .into_iter()
        .map(|(base, size, kind_atom)| {
            let kind = if kind_atom == crate::atoms::heap() {
                MemKind::Heap
            } else if kind_atom == crate::atoms::module() {
                MemKind::Module
            } else {
                MemKind::Other
            };
            MemRegion { base, size, kind }
        })
        .collect();
    with_gui_state(|gs| gs.memory_layout = layout);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_error(message: String) -> Atom {
    error!("set_error: {}", message);
    with_gui_state(|gs| gs.error = Some(message));
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_watch_entries(
    entries: Vec<(u64, String, crate::schema::DataType, bool, bool, Result<String, String>)>,
) -> Atom {
    debug!("set_watch_entries: {} entries", entries.len());
    let entries = entries
        .into_iter()
        .map(|(addr, label, data_type, auto_refresh, bare_map, result)| {
            let type_name = data_type.0.trim_start_matches("Elixir.").to_string();
            (addr, label, type_name, auto_refresh, bare_map, result)
        })
        .collect();
    with_gui_state(|gs| gs.watch_entries = entries);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_cast_preview(result: Result<String, String>) -> Atom {
    debug!("set_cast_preview");
    with_gui_state(|gs| gs.cast_preview = Some(result));
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_inferred_fields(fields: Vec<(String, String)>) -> Atom {
    debug!("set_inferred_fields: {} fields", fields.len());
    with_gui_state(|gs| gs.inferred_fields = fields);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_word_size(size: u64) -> Atom {
    debug!("set_word_size: {}", size);
    with_gui_state(|gs| gs.word_size = size);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_schema_validation(errors: Vec<String>) -> Atom {
    debug!("set_schema_validation: {} errors", errors.len());
    with_gui_state(|gs| gs.schema_validation = errors);
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_schema_generated(result: (bool, String)) -> Atom {
    debug!("set_schema_generated: ok={}", result.0);
    let r = if result.0 {
        Ok(result.1)
    } else {
        Err(result.1)
    };
    with_gui_state(|gs| gs.schema_generated_code = Some(r));
    crate::atoms::ok()
}

#[rustler::nif]
pub fn set_schema_load_result(msg: String) -> Atom {
    debug!("set_schema_load_result: {}", msg);
    with_gui_state(|gs| gs.schema_load_status = Some(msg));
    crate::atoms::ok()
}

#[rustler::nif]
pub fn reset_scan_state() -> Atom {
    info!("reset_scan_state");
    with_gui_state(|gs| {
        gs.results.clear();
        gs.scan_status = ScanStatus::Idle;
        gs.scan_pattern.clear();
        gs.scan_errors.clear();
        gs.inferred_fields.clear();
    });
    crate::atoms::ok()
}

#[rustler::nif]
pub fn reset_process_state() -> Atom {
    info!("reset_process_state");
    with_gui_state(|gs| {
        gs.attached = None;
        gs.processes.clear();
        gs.watch_entries.clear();
        gs.hex_rows.clear();
        gs.memory_layout.clear();
        gs.results.clear();
        gs.scan_status = ScanStatus::Idle;
        gs.scan_pattern.clear();
        gs.scan_errors.clear();
        gs.inferred_fields.clear();
    });
    crate::atoms::ok()
}

#[rustler::nif]
pub fn nif_reply(token: u64, _response: rustler::Binary) -> Atom {
    info!("nif_reply: token={}", token);
    if let Ok(guard) = STATE.lock() {
        if let Some(ws) = guard.as_ref() {
            let mut calls = ws.calls.lock().unwrap();
            if let Some(tx) = calls.remove(&token) {
                drop(tx);
            }
        }
    }
    crate::atoms::ok()
}
