//! C++ integration test.
//!
//! Builds and spawns the `cpp_component` fixture (`examples/cpp_fixture/linux/main.cpp`),
//! scans its live memory for a known `std::string`, and hands the matched address back to
//! the fixture for it to confirm.
//!
//! On Windows the fixture is a separate Visual Studio project under
//! `examples/cpp_fixture/windows/` and isn't auto-built here — pass an already-running
//! process's PID as `argv[1]`, or omit it to be prompted interactively.

use std::env::args;
#[cfg(windows)]
use std::io;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

use libkrebs::error::MemError;
use libkrebs::mem::{region, AccessLevel, Buffer, ProcMap, Process, Region};
use libkrebs::pattern_scan::Simd32Pattern;
use libkrebs::NativeProcess;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The fact that our chosen string is only 14 bytes (incl. null) gives us an advantage.
    // If it were much longer, it wouldn't be stored in the internal buffer, making scans
    // much more difficult.
    #[cfg(windows)]
    let string_siggy = {
        use libkrebs::cpp_std::vcpp;
        Simd32Pattern::from(vcpp::string::str_to_pattern("Hello, World!"))
    };

    #[cfg(unix)]
    let string_siggy = {
        use libkrebs::cpp_std::gcc;
        Simd32Pattern::from(gcc::string::str_to_pattern("Hello, World!"))
    };

    let mut argv = args();
    let explicit_pid = if argv.len() >= 2 {
        Some(argv.nth(1).expect("at least one argv").parse::<usize>()?)
    } else {
        None
    };

    #[cfg(unix)]
    let (mut fixture, o_proc) = match explicit_pid {
        Some(pid) => (None, process_from_pid(pid)?),
        None => {
            let (child, reader, pid) = fixture::spawn()?;
            println!("Built and spawned cpp_component fixture (PID {pid})");
            (Some((child, reader)), process_from_pid(pid)?)
        }
    };

    #[cfg(windows)]
    let (_fixture, o_proc): (Option<()>, NativeProcess) = {
        let proc = match explicit_pid {
            Some(pid) => process_from_pid(pid)?,
            None => ask_for_process(&io::stdin())?,
        };
        (None, proc)
    };

    // All pages of the host process's address space
    // (including the free ones, which make up most of it).
    // Addresses above 0x7FFFFFFF are reserved by Windows.
    let mem_blocks = o_proc.get_regions().expect("Error getting regions");

    // We wrap the opened process in a mutex, because various
    // Windows APIs (such as error handling) rely on linear
    // sequencing of events.
    let wrapped_o_proc = Arc::new(Mutex::new(o_proc));

    // Create our buffer. It reads one megabyte at a time (less if the range is smaller).
    // `mem::Buffer`s are easily parallelizable.
    let mut buf = Buffer::new(wrapped_o_proc, 1024 * 1024);

    let valid_blocks: Vec<Region> = mem_blocks
        .into_iter()
        .filter(|x| {
            // Obviously, we're not scanning free memory.
            x.state == region::State::COMMITTED
                // We want to scan private memory, not mapped files.
                && x.mem_type == region::MemType::PRIVATE
                // Don't touch the guard pages.
                && (x.perms & (region::Perms::GUARD | region::Perms::NOPE)).is_empty()
        })
        .collect();

    let mut found = None;
    for block in valid_blocks.iter() {
        println!(
            "Scanning block from {} (type {:?}, perms {})",
            block.range,
            block.mem_type,
            block.perms.to_string()
        );

        let (str_scan_result, _scan_time) = buf.find_one(&string_siggy, block.range);

        match str_scan_result {
            Some(scan_match) => {
                println!("Matched {:?} at {:#08X}", scan_match.value, scan_match.addr);
                found = Some(scan_match);
                break;
            }
            None => println!("Nothing in {}", block.range),
        }
    }

    // Hand the matched address back to the fixture so it can confirm this is really the
    // address of its own string, rather than just trusting the scan.
    #[cfg(unix)]
    if let (Some((mut child, mut reader)), Some(scan_match)) = (fixture.take(), found) {
        writeln!(child.stdin.as_mut().expect("piped stdin"), "{:x}", scan_match.addr)?;

        // Skip past the fixture's startup diagnostics (byte dump, etc.) to the
        // "string address: (in)correct" verdict it prints after reading the address.
        // Check "incorrect" before "correct" — the latter is a substring of the former.
        let mut line = String::new();
        let verdict = loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break "ERR";
            }
            if line.contains("incorrect") {
                break "incorrect";
            }
            if line.contains("correct") {
                break "correct";
            }
            if line.contains("ERR") {
                break "ERR";
            }
        };
        println!("Fixture says the address is {}", verdict);

        child.wait()?;

        if verdict != "correct" {
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(windows)]
fn ask_for_process(stdin: &io::Stdin) -> Result<NativeProcess, MemError> {
    let mut input = String::new();
    println!("Type the process PID: ");
    stdin.read_line(&mut input).expect("failed to read line");

    let pid = input.trim().parse::<usize>().unwrap();

    println!("SEARCHING FOR {}", pid);

    process_from_pid(pid)
}

#[cfg(windows)]
fn process_from_pid(pid: usize) -> Result<NativeProcess, MemError> {
    NativeProcess::attach(pid, AccessLevel::Read)
}

#[cfg(unix)]
fn process_from_pid(pid: usize) -> Result<NativeProcess, MemError> {
    NativeProcess::attach(pid, AccessLevel::Read)
}

/// Builds (if necessary) and launches the `cpp_fixture` companion process.
#[cfg(unix)]
mod fixture {
    use std::io::{self, BufRead, BufReader};
    use std::path::Path;
    use std::process::{Child, ChildStdout, Command, Stdio};

    pub fn spawn() -> io::Result<(Child, BufReader<ChildStdout>, usize)> {
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/cpp_fixture");
        let binary = fixture_dir.join("cpp_component");

        if !binary.exists() {
            println!("cpp_component not found, building it (`make` in {:?})...", fixture_dir);
            let status = Command::new("make").current_dir(&fixture_dir).status()?;
            if !status.success() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "failed to build cpp_component — is `make`/`g++` installed?",
                ));
            }
        }

        let mut child = Command::new(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));

        let mut pid_line = String::new();
        reader.read_line(&mut pid_line)?;
        let pid = pid_line
            .trim()
            .strip_prefix("PID: ")
            .expect("fixture did not report its PID on the first line")
            .parse()
            .expect("fixture PID was not a number");

        Ok((child, reader, pid))
    }
}
