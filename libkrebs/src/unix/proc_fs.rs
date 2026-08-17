use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::Path;

use libc::pid_t;

use crate::error::{MemError, UnixMemError};
use crate::mem::{AccessLevel, MemAddress, Module, Region};
use crate::mem::{ProcMap, Process, ProcessId, Reader, Writer};

use super::proc_maps::*;

pub struct ProcFs {
    pid: pid_t,
    exe_path: String,
    mem_handle: File,
    access_level: AccessLevel,
}

impl ProcFs {
    pub fn new(pid: pid_t) -> Result<ProcFs, MemError> {
        Self::open(pid, AccessLevel::Read)
    }

    fn open(pid: pid_t, access: AccessLevel) -> Result<ProcFs, MemError> {
        let proc_mem_path = format!("/proc/{}/mem", pid);
        let proc_exe_link = format!("/proc/{}/exe", pid);

        let exe_path = std::fs::read_link(&proc_exe_link)
            .map_err(|e| UnixMemError::ProcessOpenFailed {
                pid,
                kind: e.kind(),
            })?
            .to_string_lossy()
            .into_owned();

        let mem_handle = match access {
            AccessLevel::Read => File::open(&proc_mem_path),
            AccessLevel::ReadWrite => OpenOptions::new()
                .read(true)
                .write(true)
                .open(&proc_mem_path),
        }
        .map_err(|e| UnixMemError::ProcessOpenFailed {
            pid,
            kind: e.kind(),
        })?;

        log::info!("attach: pid={} exe={} access={:?}", pid, exe_path, access);

        Ok(ProcFs {
            pid,
            exe_path,
            mem_handle,
            access_level: access,
        })
    }

    pub fn exe_path(&self) -> &str {
        &self.exe_path
    }

    fn parse_proc_maps_file(&self) -> Result<Vec<ProcMapsRow>, MemError> {
        let proc_maps_path = format!("/proc/{}/maps", self.pid);

        let proc_maps_lines: Vec<String> = {
            let mut file =
                File::open(&proc_maps_path).map_err(|e| UnixMemError::ProcMapsOpenFailed {
                    pid: self.pid,
                    kind: e.kind(),
                })?;
            let mut file_data = String::new();
            file.read_to_string(&mut file_data)
                .map_err(|e| UnixMemError::ProcMapsOpenFailed {
                    pid: self.pid,
                    kind: e.kind(),
                })?;
            file_data.split('\n').map(str::to_string).collect()
        };

        let mut proc_maps_rows: Vec<ProcMapsRow> = proc_maps_lines
            .iter()
            .filter(|l| l.len() > 0)
            .map(|row_str| {
                let row_fields: Vec<&str> = row_str.split_whitespace().collect();

                if row_fields.len() < 5 {
                    return Err(MemError::Unix(UnixMemError::ProcMapsRowParseError {
                        field_count: row_fields.len(),
                        row: row_str.clone(),
                    }));
                }

                // fields: [0] addr_range  [1] perms  [2] offset  [3] dev  [4] inode  [5..] pathname
                let range_str = row_fields[0];
                let perms_str = row_fields[1];
                // pathname can contain spaces (e.g. "(deleted)" suffix or paths with spaces),
                // so join everything from field 5 onwards.
                let pathname = if row_fields.len() <= 5 {
                    String::new()
                } else {
                    row_fields[5..].join(" ")
                };

                let range = get_addr_range(range_str).map_err(MemError::Unix)?;
                let perms = get_perms(perms_str).map_err(MemError::Unix)?;

                Ok(ProcMapsRow {
                    range,
                    perms,
                    pathname,
                })
            })
            .map(Result::unwrap)
            .collect();

        proc_maps_rows.sort_by_key(|r| r.range);

        Ok(proc_maps_rows)
    }
}

impl Reader for ProcFs {
    unsafe fn read_n_bytes(
        &mut self,
        base_addr: MemAddress,
        buffer: &mut [u8],
        size: usize,
    ) -> Result<usize, MemError> {
        debug_assert!(buffer.len() <= size);

        self.mem_handle
            .seek(std::io::SeekFrom::Start(base_addr as u64))
            .map_err(|e| {
                MemError::Unix(UnixMemError::ProcMemReadFailed {
                    addr: base_addr,
                    size,
                    kind: e.kind(),
                })
            })?;

        self.mem_handle.read(&mut buffer[..size]).map_err(|e| {
            MemError::Unix(UnixMemError::ProcMemReadFailed {
                addr: base_addr,
                size,
                kind: e.kind(),
            })
        })
    }
}

impl Writer for ProcFs {
    unsafe fn write_n_bytes(
        &mut self,
        base_addr: MemAddress,
        bytes: &[u8],
        size: usize,
    ) -> Result<usize, MemError> {
        debug_assert!(bytes.len() <= size);

        self.mem_handle
            .seek(std::io::SeekFrom::Start(base_addr as u64))
            .map_err(|e| {
                MemError::Unix(UnixMemError::ProcMemWriteFailed {
                    addr: base_addr,
                    size,
                    kind: e.kind(),
                })
            })?;

        self.mem_handle.write(&bytes[..size]).map_err(|e| {
            MemError::Unix(UnixMemError::ProcMemWriteFailed {
                addr: base_addr,
                size,
                kind: e.kind(),
            })
        })
    }
}

impl ProcMap for ProcFs {
    fn get_regions(&self) -> Result<Vec<Region>, MemError> {
        let proc_maps_rows = self.parse_proc_maps_file()?;
        Ok(proc_maps_rows_to_regions(&proc_maps_rows))
    }

    fn get_modules(&self) -> Result<Vec<Module>, MemError> {
        let proc_maps_rows = self.parse_proc_maps_file()?;
        Ok(proc_maps_rows_to_modules(&proc_maps_rows))
    }
}

impl Process for ProcFs {
    fn attach(pid: ProcessId, access: AccessLevel) -> Result<Self, MemError> {
        // ProcessId is usize; pid_t is i32. Linux PIDs fit in i32 in practice.
        Self::open(pid as pid_t, access)
    }

    fn close(&mut self) -> Result<(), MemError> {
        Ok(())
    }

    /// Scans `/proc/{pid}/comm` for every numeric directory under `/proc`.
    fn list_processes() -> HashMap<ProcessId, String> {
        let mut map = HashMap::new();

        let Ok(entries) = std::fs::read_dir("/proc") else {
            return map;
        };

        for entry in entries.flatten() {
            let fname = entry.file_name();
            let Ok(pid) = fname.to_string_lossy().parse::<ProcessId>() else {
                continue;
            };
            let comm_path = format!("/proc/{}/comm", pid);
            if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                map.insert(pid, comm.trim_end_matches('\n').to_string());
            }
        }

        map
    }

    fn search_processes(exe_name: &str) -> Vec<ProcessId> {
        Self::list_processes()
            .into_iter()
            .filter(|(_, name)| name.contains(exe_name))
            .map(|(pid, _)| pid)
            .collect()
    }

    /// Window title enumeration requires X11 or Wayland bindings.
    /// Not implemented yet.
    fn list_windows() -> HashMap<ProcessId, Vec<String>> {
        HashMap::new()
    }

    fn search_windows(_window_title: &str) -> Vec<ProcessId> {
        vec![]
    }

    fn pid(&self) -> ProcessId {
        self.pid as ProcessId
    }

    fn access_level(&self) -> AccessLevel {
        self.access_level
    }

    fn executable_name(&self) -> String {
        Path::new(&self.exe_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.exe_path.clone())
    }

    /// Window titles for this process.  Requires X11/Wayland; not implemented yet.
    fn window_names(&self) -> Vec<String> {
        vec![]
    }
}
