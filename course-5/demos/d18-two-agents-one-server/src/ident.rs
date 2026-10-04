//! Server identity (spec §5.1, survivor s02): the pid comes from the spawned
//! child handle, and `/proc/<pid>/stat` field 22 (start time in clock ticks)
//! plus `/proc/<pid>/exe` are read for it. A respawn that reused the pid
//! number still has different start ticks.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub pid: u32,
    /// Field 22 of `/proc/<pid>/stat`.
    pub start_ticks: u64,
    /// What `/proc/<pid>/exe` resolves to.
    pub exe: PathBuf,
}

/// Read `pid`'s identity from `/proc`.
pub fn read(pid: u32) -> Result<Ident, String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|e| format!("/proc/{pid}/stat: {e}"))?;
    let start_ticks =
        start_ticks_from_stat(&stat).ok_or_else(|| format!("/proc/{pid}/stat: no field 22"))?;
    let exe = std::fs::read_link(format!("/proc/{pid}/exe"))
        .map_err(|e| format!("/proc/{pid}/exe: {e}"))?;
    Ok(Ident {
        pid,
        start_ticks,
        exe,
    })
}

/// Field 22 of a `/proc/<pid>/stat` line. Field 2 (`comm`) is parenthesised
/// and may itself hold spaces and `)`, so fields are counted after the LAST
/// `)`: the first token there is field 3, so field 22 is the 20th.
pub fn start_ticks_from_stat(stat: &str) -> Option<u64> {
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(22 - 3)?.parse().ok()
}
