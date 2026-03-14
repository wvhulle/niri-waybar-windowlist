use std::path::Path;

use futures::AsyncReadExt;
use procfs::process::Process;
use thiserror::Error;
use waybar_cffi::gtk::{
    gio::{File, prelude::InputStreamExtManual, traits::FileExt},
    glib::{self, Priority},
};

pub struct ProcessInfo {
    pub parent_id: Option<i64>,
}

impl ProcessInfo {
    #[tracing::instrument(level = "TRACE", err)]
    pub async fn query(pid: i64) -> Result<Self, ProcessError> {
        let stat_file = File::for_path(format!("/proc/{pid}/stat"));

        let mut reader = stat_file
            .read_future(Priority::DEFAULT)
            .await
            .map_err(|e| ProcessError::FileOpen { e, pid })?
            .into_async_buf_read(4096);

        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .await
            .map_err(|e| ProcessError::FileRead { e, pid })?;

        let ppid_str = content
            .split(' ')
            .nth(3)
            .ok_or_else(|| ProcessError::MalformedStat { pid })?;

        let ppid = ppid_str
            .parse()
            .map_err(|_| ProcessError::InvalidPpid { value: ppid_str.to_owned(), pid })?;

        Ok(Self {
            parent_id: if ppid == 0 { None } else { Some(ppid) },
        })
    }
}

pub struct ForegroundProcessInfo {
    pub cwd: Option<String>,
    pub command: Option<String>,
}

pub fn query_foreground(terminal_pid: u32) -> Result<ForegroundProcessInfo, ForegroundError> {
    let pid = i32::try_from(terminal_pid)
        .map_err(|_| ForegroundError::InvalidPid { pid: terminal_pid })?;
    let terminal = Process::new(pid)?;

    let shell_pids: Vec<i32> = terminal
        .tasks()?
        .filter_map(|t| t.ok())
        .flat_map(|task| task.children().unwrap_or_default())
        .filter_map(|child_pid| {
            let child_pid = child_pid as i32;
            let child = Process::new(child_pid).ok()?;
            let stat = child.stat().ok()?;
            (stat.tty_nr != 0).then_some(child_pid)
        })
        .collect();

    if shell_pids.is_empty() {
        return Err(ForegroundError::NoChildren { pid: terminal_pid });
    }

    let fg_pid = find_foreground_pid(&shell_pids).unwrap_or(shell_pids[0]);
    let fg_process = Process::new(fg_pid)?;

    let cwd = fg_process.cwd()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());

    let command = fg_process.cmdline()
        .ok()
        .and_then(|args| args.into_iter().next())
        .map(|argv0| {
            Path::new(&argv0)
                .file_name()
                .map_or(argv0.clone(), |n| n.to_string_lossy().into_owned())
        });

    Ok(ForegroundProcessInfo { cwd, command })
}

fn find_foreground_pid(shell_pids: &[i32]) -> Option<i32> {
    for &shell_pid in shell_pids {
        let Ok(shell) = Process::new(shell_pid) else { continue };
        let Ok(stat) = shell.stat() else { continue };

        if stat.tpgid == stat.pgrp {
            return Some(shell_pid);
        }

        return find_process_in_group(shell_pid, stat.tpgid)
            .or(Some(shell_pid));
    }
    None
}

fn find_process_in_group(pid: i32, target_pgrp: i32) -> Option<i32> {
    let proc = Process::new(pid).ok()?;
    let children: Vec<u32> = proc.tasks().ok()?
        .filter_map(|t| t.ok())
        .flat_map(|task| task.children().unwrap_or_default())
        .collect();

    for child_pid in children {
        let child_pid = child_pid as i32;
        if let Ok(child) = Process::new(child_pid) {
            if let Ok(child_stat) = child.stat() {
                if child_stat.pgrp == target_pgrp {
                    return Some(child_pid);
                }
            }
        }
        if let Some(found) = find_process_in_group(child_pid, target_pgrp) {
            return Some(found);
        }
    }
    None
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("malformed /proc/{pid}/stat: missing fields")]
    MalformedStat { pid: i64 },

    #[error("invalid PPID in /proc/{pid}/stat: {value}")]
    InvalidPpid { value: String, pid: i64 },

    #[error("cannot open /proc/{pid}/stat: {e}")]
    FileOpen {
        #[source]
        e: glib::Error,
        pid: i64,
    },

    #[error("cannot read /proc/{pid}/stat: {e}")]
    FileRead {
        #[source]
        e: futures::io::Error,
        pid: i64,
    },
}

#[derive(Error, Debug)]
pub enum ForegroundError {
    #[error(transparent)]
    Proc(#[from] procfs::ProcError),

    #[error("invalid pid: {pid}")]
    InvalidPid { pid: u32 },

    #[error("no children for pid {pid}")]
    NoChildren { pid: u32 },
}
