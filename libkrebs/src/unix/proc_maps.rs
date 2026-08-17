use crate::error::UnixMemError;
use crate::mem::region::{MemType, Perms, State};
use crate::mem::{AddressRange, MemAddress, Module, Region};

pub struct ProcMapsRow {
    pub range: AddressRange,
    pub perms: Perms,
    pub pathname: String,
}

pub fn get_addr_range(addr_range_str: &str) -> Result<AddressRange, UnixMemError> {
    let dash_index = addr_range_str.find('-').ok_or_else(|| {
        UnixMemError::ProcMapsAddrRangeParseFailed {
            input: addr_range_str.to_string(),
        }
    })?;

    let start_addr_str = &addr_range_str[..dash_index];
    let end_addr_str = &addr_range_str[dash_index + 1..];

    let start_addr = MemAddress::from_str_radix(start_addr_str, 16).map_err(|_| {
        UnixMemError::ProcMapsAddrRangeParseFailed {
            input: addr_range_str.to_string(),
        }
    })?;
    let end_addr = MemAddress::from_str_radix(end_addr_str, 16).map_err(|_| {
        UnixMemError::ProcMapsAddrRangeParseFailed {
            input: addr_range_str.to_string(),
        }
    })?;

    Ok(AddressRange::new(start_addr, end_addr))
}

pub fn get_perms(perms_str: &str) -> Result<Perms, UnixMemError> {
    let valid_chars = "rwxps-";
    let valid = perms_str.chars().all(|c| valid_chars.contains(c)) && perms_str.len() == 4;
    if !valid {
        return Err(UnixMemError::ProcMapsPermsParseFailed {
            input: perms_str.to_string(),
        });
    }

    let mut perms = Perms::NONE;
    if &perms_str[0..1] == "r" {
        perms |= Perms::READ;
    }
    if &perms_str[1..2] == "w" {
        perms |= Perms::WRITE;
    }
    if &perms_str[2..3] == "x" {
        perms |= Perms::EXECUTE;
    }
    if &perms_str[0..3] == "rwx" {
        perms |= Perms::GUARD;
    }

    Ok(perms)
}

pub fn proc_maps_rows_to_modules(rows: &[ProcMapsRow]) -> Vec<Module> {
    rows.chunk_by(|a, b| a.pathname == b.pathname)
        .map(|group| -> Option<Module> {
            if group
                .iter()
                .find(|b| (b.perms & Perms::EXECUTE).bits() != 0)
                .is_some()
            {
                Some(Module {
                    range: AddressRange::new(
                        group[0].range.start_addr,
                        group[group.len() - 1].range.end_addr,
                    ),
                    name: group[0].pathname.to_string(),
                })
            } else {
                None
            }
        })
        .filter(Option::is_some)
        .map(Option::unwrap)
        .collect()
}

pub fn proc_maps_rows_to_regions(rows: &[ProcMapsRow]) -> Vec<Region> {
    rows.chunk_by(|a, b| a.pathname == b.pathname)
        .map(|group| {
            let group_base = group[0].range.start_addr;

            // The region categories (originally devised for windows)
            // don't translate well into linux terms...
            //
            // we will consider any mapped block with an executable section to
            // be an "image", any without an executable section to be a "map",
            // and any other to be "private"
            let group_type: MemType = {
                if !(group[0].pathname.len() == 0
                    || group[0].pathname == "[stack]"
                    || group[0].pathname == "[heap]")
                {
                    if group
                        .iter()
                        .find(|b| (b.perms & Perms::EXECUTE).bits() != 0)
                        .is_some()
                    {
                        MemType::IMAGE
                    } else {
                        MemType::MAPPED
                    }
                } else {
                    MemType::PRIVATE
                }
            };

            group.iter().map(move |b| Region {
                state: State::COMMITTED,

                base: group_base,
                mem_type: group_type,

                range: b.range,
                perms: b.perms,
            })
        })
        .flatten()
        .collect()
}
