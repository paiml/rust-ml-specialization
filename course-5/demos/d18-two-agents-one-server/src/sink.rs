//! Role↔file binding by construction (spec §5.1, survivor s05).
//!
//! Each role writes only through a [`Sink`] the [`Registry`] opened for it,
//! and the registry records every path it opened. `writer_files` and
//! `checker_files` come from here, never from what an agent reports. Every
//! open is `create_new`, so opening one path twice is an error, not a second
//! entry.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Writer,
    Checker,
}

/// Every path a sink opened under one schedule's root, with its role.
#[derive(Debug)]
pub struct Registry {
    root: PathBuf,
    opened: Mutex<Vec<(Role, String)>>,
}

/// One role's open output file.
#[derive(Debug)]
pub struct Sink {
    file: File,
}

impl Registry {
    pub fn new(root: &Path) -> Self {
        Registry {
            root: root.to_path_buf(),
            opened: Mutex::new(Vec::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Open `rel` under the root for `role`, `create_new`. The path is
    /// recorded only once the open succeeded.
    pub fn open(&self, role: Role, rel: &str) -> io::Result<Sink> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        self.opened
            .lock()
            .map_err(|_| io::Error::other("registry poisoned"))?
            .push((role, rel.to_string()));
        Ok(Sink { file })
    }

    /// The paths `role`'s sinks opened, in open order.
    pub fn files(&self, role: Role) -> Vec<String> {
        self.opened
            .lock()
            .map(|o| {
                o.iter()
                    .filter(|(r, _)| *r == role)
                    .map(|(_, p)| p.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Sink {
    /// Append one line and flush it to the file, so a reader that starts
    /// after this returns sees the whole line.
    pub fn append_line(&mut self, line: &str) -> io::Result<()> {
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        self.file.sync_data()
    }
}
