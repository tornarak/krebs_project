#[macro_use]
extern crate rustler;
use rustler::{Env, Term};

mod atoms;
mod erl_channel;
mod errors;
mod nif_logger;
mod pattern;
mod proc_map;
mod process;
mod reader;
mod scanner;

#[nif]
fn nif_log_init(pid: rustler::LocalPid) -> rustler::Atom {
    nif_logger::init(pid);
    atoms::ok()
}

#[allow(non_local_definitions)]
fn on_load(env: Env, _info: Term) -> bool {
    resource!(process::ProcessResource, env);
    resource!(scanner::ScannerResource, env);
    resource!(pattern::PatternResource, env);
    true
}

rustler::init!(
    "Elixir.Krebs.Nif",
    [
        nif_log_init,
        // patterns (unchanged)
        pattern::new_scan_pattern,
        pattern::concat_scan_patterns,
        // process enumeration (static)
        process::list_processes,
        process::search_processes,
        process::list_windows,
        process::search_windows,
        // process attachment + accessors
        process::attach,
        process::process_pid,
        process::executable_name,
        process::window_names,
        process::access_level,
        process::close,
        // memory read/write
        reader::read,
        reader::write,
        // proc map
        proc_map::regions,
        proc_map::modules,
        // scanner lifecycle
        scanner::scanner_new,
        scanner::refresh_layout,
        // scanner operations
        scanner::is_in_memory,
        scanner::scan,
    ],
    load = on_load
);
