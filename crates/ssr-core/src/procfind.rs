//! Finding processes to attach a loopback capture to.
//!
//! Targets are named rather than pinned to a PID because the interesting ones
//! come and go: a Citrix session reconnects, WSLg restarts, a browser is
//! relaunched. Re-resolving by name is what makes a long recording survive that.

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

use crate::error::{Error, Result};

/// A running process.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: u32,
    /// Executable name including extension, for example `wfica32.exe`.
    pub name: String,
}

/// Snapshot every process currently running.
pub fn list_processes() -> Result<Vec<ProcessInfo>> {
    let mut out = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                out.push(ProcessInfo {
                    pid: entry.th32ProcessID,
                    parent_pid: entry.th32ParentProcessID,
                    name: String::from_utf16_lossy(&entry.szExeFile[..len]),
                });
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    Ok(out)
}

/// Resolve a selector to a single process id.
///
/// The selector is a decimal PID, or a case-insensitive match against the
/// executable name. Name matching prefers an exact match, then a prefix match,
/// then any substring. When several processes match, the one with the lowest PID
/// wins, which is usually the oldest and therefore the session root.
pub fn resolve_pid(selector: &str) -> Result<u32> {
    if let Ok(pid) = selector.parse::<u32>() {
        // Worth checking. Process loopback activation succeeds against a dead
        // process id and then records silence forever, which looks exactly like
        // an application that simply never made a sound.
        if !process_exists(pid)? {
            return Err(Error::NoSuchProcess(format!("pid {pid} is not running")));
        }
        return Ok(pid);
    }
    let needle = selector.to_lowercase();
    let procs = list_processes()?;

    let mut exact: Vec<&ProcessInfo> = Vec::new();
    let mut prefix: Vec<&ProcessInfo> = Vec::new();
    let mut substring: Vec<&ProcessInfo> = Vec::new();
    for p in &procs {
        let lower = p.name.to_lowercase();
        // Exact either way round: `wfica32.exe` and `wfica32` both name the
        // same process, and the selector may be given with or without it.
        if lower == needle || lower.strip_suffix(".exe") == Some(needle.as_str()) {
            exact.push(p);
        } else if lower.starts_with(&needle) {
            prefix.push(p);
        } else if lower.contains(&needle) {
            substring.push(p);
        }
    }

    for bucket in [exact, prefix, substring] {
        if let Some(p) = bucket.iter().min_by_key(|p| p.pid) {
            return Ok(p.pid);
        }
    }
    Err(Error::NoSuchProcess(selector.to_string()))
}

/// Whether a process id belongs to something currently running.
pub fn process_exists(pid: u32) -> Result<bool> {
    Ok(list_processes()?.iter().any(|p| p.pid == pid))
}

/// Process names worth suggesting when the user asks what they could record.
///
/// A thin view over [`crate::catalog`], which is the single list. Kept as a
/// function rather than a constant so the two cannot drift apart, and filtered
/// to the applications a person would actually choose: the Windows shell makes
/// sound, but nobody sets out to record it.
pub fn known_targets() -> Vec<(&'static str, &'static str)> {
    crate::catalog::CATALOG
        .iter()
        .filter(|e| e.category != crate::catalog::Category::System)
        .map(|e| (e.label, e.exe))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_suggestion_list_covers_the_sources_the_tool_exists_for() {
        let targets = known_targets();
        for exe in ["wfica32.exe", "msrdc.exe"] {
            assert!(
                targets.iter().any(|(_, e)| e.eq_ignore_ascii_case(exe)),
                "{exe} is missing from the suggestions"
            );
        }
    }

    #[test]
    fn the_windows_shell_is_not_suggested_as_a_recording_target() {
        assert!(!known_targets()
            .iter()
            .any(|(_, e)| e.eq_ignore_ascii_case("explorer.exe")));
    }
}
