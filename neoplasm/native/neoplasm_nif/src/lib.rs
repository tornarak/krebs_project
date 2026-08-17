use rustler::{Env, Term};

mod atoms;
mod schema;
pub mod types;
mod window;

pub use types::*;

#[allow(non_local_definitions)]
fn on_load(_env: Env, _info: Term) -> bool {
    log::info!("neoplasm_nif loaded");
    true
}

rustler::init!(
    "Elixir.Neoplasm.Nif",
    [
        window::open_window,
        window::close_window,
        window::set_attached,
        window::set_processes,
        window::set_scan_status,
        window::push_results,
        window::clear_results,
        window::set_schemas,
        window::set_scan_pattern,
        window::set_scan_errors,
        window::set_inferred_fields,
        window::set_hex_dump,
        window::set_memory_layout,
        window::set_error,
        window::set_watch_entries,
        window::set_cast_preview,
        window::set_word_size,
        window::set_schema_validation,
        window::set_schema_generated,
        window::set_schema_load_result,
        window::reset_scan_state,
        window::reset_process_state,
        window::nif_reply,
    ],
    load = on_load
);
