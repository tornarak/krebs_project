use std::env::args;
use std::io;

use libkrebs::mem::{AccessLevel, Process, ProcessId};
use libkrebs::NativeProcess;

mod assault_cube_hack;

fn main() {
    let mut argv = args();
    let ac = if argv.len() >= 2 {
        let pid: ProcessId = argv.nth(1).unwrap().parse().expect("invalid PID");
        NativeProcess::attach(pid, AccessLevel::ReadWrite).expect("failed to attach")
    } else {
        let stdin = io::stdin();
        ask_until_process(&stdin)
    };

    assault_cube_hack::hax_game(ac);
}

pub fn ask_until_process(stdin: &io::Stdin) -> NativeProcess {
    let mut read_proc_opt: Option<NativeProcess>;

    loop {
        read_proc_opt = ask_for_process(&stdin);

        if let Some(procc) = read_proc_opt {
            return procc;
        } else {
            println!("Could not find process")
        }

        println!()
    }
}

pub fn ask_for_process(stdin: &io::Stdin) -> Option<NativeProcess> {
    let mut input = String::new();
    println!("Type a process ID:");
    stdin.read_line(&mut input).expect("failed to read line");
    let pid_result = input.trim().parse::<ProcessId>();

    match pid_result {
        Ok(pid) => NativeProcess::attach(pid, AccessLevel::ReadWrite).ok(),
        Err(_) => None,
    }
}
