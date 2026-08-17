use libkrebs::error::KrebsError;
use libkrebs::mem::{MemAddress, Reader, Writer};

use crate::errors::Enc;
use crate::process::ProcessRef;

#[nif(schedule = "DirtyIo")]
pub fn read(proc: ProcessRef, addr: MemAddress, size: usize) -> Result<String, Enc<KrebsError>> {
    // would be interdasting if you could create a string
    // with a precise capacity and write directly to that,
    // or unsafely disassemble a vec into a string...
    // but this is difficult for good reason
    let mut buf = vec![0u8; size];
    let mut guard = proc.resource.inner.lock().unwrap();
    // SAFETY: Same as pattern.rs — this String is encoded by rustler as an
    // Erlang binary, not a charlist. The bytes are arbitrary process memory
    // and will often contain invalid UTF-8, but that's fine because neither
    // Rust nor Erlang will interpret them as text.
    unsafe { guard.read_n_bytes(addr, &mut buf, size) }
        .map(|_| unsafe { String::from_utf8_unchecked(buf) })
        .map_err(|e| Enc(KrebsError::Mem(e)))
}

#[nif(schedule = "DirtyIo")]
pub fn write(proc: ProcessRef, addr: MemAddress, data: Vec<u8>) -> Result<usize, Enc<KrebsError>> {
    let size = data.len();
    let mut guard = proc.resource.inner.lock().unwrap();
    unsafe { guard.write_n_bytes(addr, &data, size) }.map_err(|e| Enc(KrebsError::Mem(e)))
}
