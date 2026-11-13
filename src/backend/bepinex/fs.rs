//! Linux directory capabilities: game paths never resolve through symlinks.

use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_MANIFEST: AtomicU64 = AtomicU64::new(0);

pub(super) struct Root(File);

pub(super) struct Captured<'a> {
    root: &'a Root,
    parent: File,
    leaf: CString,
    temporary: String,
    finished: bool,
}

impl Captured<'_> {
    fn remove_if_identity(mut self, expected: (u64, u64)) -> Result<bool, String> {
        let file =
            open_at(&self.root.0, &self.temporary, libc::O_PATH).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if (metadata.dev(), metadata.ino()) == expected {
            self.root.remove(&self.temporary)?;
            self.finished = true;
            Ok(true)
        } else {
            self.restore()?;
            Ok(false)
        }
    }
    fn restore(&mut self) -> Result<(), String> {
        let temporary = name(&self.temporary).map_err(|e| e.to_string())?;
        // SAFETY: names and both directory capabilities are live; NOREPLACE preserves replacements.
        let result = unsafe {
            libc::renameat2(
                self.root.0.as_raw_fd(),
                temporary.as_ptr(),
                self.parent.as_raw_fd(),
                self.leaf.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        self.finished = true;
        if result < 0 {
            return Err(format!(
                "A changed file was preserved as {}; its original destination was not overwritten",
                self.temporary
            ));
        }
        Ok(())
    }

    pub(super) fn remove_if_matches(mut self, expected: &str, limit: u64) -> Result<bool, String> {
        let result = self.root.read(&self.temporary, limit);
        match result {
            Ok(Some(bytes)) if super::package::digest(&bytes) == expected => {
                self.root.remove(&self.temporary)?;
                self.finished = true;
                Ok(true)
            }
            Ok(_) => {
                self.restore()?;
                Ok(false)
            }
            Err(error) => {
                self.restore()?;
                Err(error)
            }
        }
    }
}

impl Drop for Captured<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.restore();
        }
    }
}

pub(super) fn parts(path: &str) -> Result<Vec<&str>, String> {
    if path.is_empty()
        || path.len() > 512
        || path.contains(['\\', ':'])
        || path.chars().any(char::is_control)
    {
        return Err("Invalid BepInEx archive path".into());
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() > 32
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err("Absolute, empty or traversing archive paths are not allowed".into());
    }
    Ok(parts)
}

fn name(value: &str) -> io::Result<CString> {
    CString::new(value).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in filename"))
}

fn open_at(parent: &File, path: &str, flags: i32) -> io::Result<File> {
    let path = name(path)?;
    // SAFETY: the descriptor and NUL-terminated name remain live for this call.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            path.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a fresh descriptor, now owned exactly once.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn vacant_name(parent: &File, leaf: &str) -> io::Result<()> {
    // /proc resolves only our owned descriptor, never an archive-controlled link.
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", parent.as_raw_fd()))? {
        if entry?
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(leaf)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "A case-insensitive destination already exists",
            ));
        }
    }
    Ok(())
}

impl Root {
    pub(super) fn capture(&self, path: &str) -> Result<Option<Captured<'_>>, String> {
        let (parent, leaf) = match self.parent(path, false, &mut Vec::new()) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let leaf = name(&leaf).map_err(|e| e.to_string())?;
        let temporary = format!(
            ".corkytux-bepinex.remove-{}-{}",
            std::process::id(),
            NEXT_MANIFEST.fetch_add(1, Ordering::Relaxed)
        );
        let destination = name(&temporary).map_err(|e| e.to_string())?;
        // SAFETY: rename captures one directory entry atomically and never follows its leaf.
        let result = unsafe {
            libc::renameat2(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                self.0.as_raw_fd(),
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(error.to_string());
        }
        Ok(Some(Captured {
            root: self,
            parent,
            leaf,
            temporary,
            finished: false,
        }))
    }

    pub(super) fn lock(&self) -> Result<(), String> {
        // SAFETY: flock operates on the live, owned game-directory descriptor.
        if unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
            return Err("Another BepInEx operation is using this game directory".into());
        }
        Ok(())
    }

    pub(super) fn open(path: &Path) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("The game directory must be absolute".into());
        }
        let mut directory = File::open("/").map_err(|e| e.to_string())?;
        let mut depth = 0;
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(part) => {
                    let part = part.to_str().ok_or("Game paths must be valid UTF-8")?;
                    directory = open_at(&directory, part, libc::O_RDONLY | libc::O_DIRECTORY)
                        .map_err(|e| format!("Cannot open game directory without symlinks: {e}"))?;
                    depth += 1;
                }
                _ => return Err("Game directory traversal is not allowed".into()),
            }
        }
        if depth < 2 {
            return Err("Select a game directory, not a filesystem root".into());
        }
        Ok(Self(directory))
    }

    fn directory(
        &self,
        path: &[&str],
        create: bool,
        created: &mut Vec<String>,
    ) -> io::Result<File> {
        let mut directory = self.0.try_clone()?;
        for (index, part) in path.iter().enumerate() {
            let opened = open_at(&directory, part, libc::O_RDONLY | libc::O_DIRECTORY);
            directory = match opened {
                Ok(opened) => opened,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    vacant_name(&directory, part)?;
                    if !create {
                        return Err(error);
                    }
                    let entry = name(part)?;
                    // SAFETY: directory is owned; entry is NUL-terminated.
                    let result =
                        unsafe { libc::mkdirat(directory.as_raw_fd(), entry.as_ptr(), 0o755) };
                    if result < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    created.push(path[..=index].join("/"));
                    open_at(&directory, part, libc::O_RDONLY | libc::O_DIRECTORY)?
                }
                Err(error) => return Err(error),
            };
        }
        Ok(directory)
    }

    fn parent(
        &self,
        path: &str,
        create: bool,
        created: &mut Vec<String>,
    ) -> io::Result<(File, String)> {
        let parts = parts(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let directory = self.directory(&parts[..parts.len() - 1], create, created)?;
        Ok((directory, parts[parts.len() - 1].to_string()))
    }

    pub(super) fn is_directory(&self, path: &str) -> Result<bool, String> {
        let parts = parts(path)?;
        match self.directory(&parts, false, &mut Vec::new()) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!("Unsafe or unreadable game directory: {error}")),
        }
    }

    pub(super) fn read(&self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, String> {
        self.read_bounded(path, limit, true)
    }

    pub(super) fn prefix(&self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, String> {
        self.read_bounded(path, limit, false)
    }

    fn read_bounded(&self, path: &str, limit: u64, full: bool) -> Result<Option<Vec<u8>>, String> {
        let result = (|| -> io::Result<Option<Vec<u8>>> {
            let (parent, leaf) = self.parent(path, false, &mut Vec::new())?;
            let file = open_at(&parent, &leaf, libc::O_RDONLY | libc::O_NONBLOCK)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || (full && metadata.len() > limit) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Expected a bounded regular file",
                ));
            }
            let mut bytes = Vec::new();
            file.take(limit + u64::from(full)).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "File exceeds limit",
                ));
            }
            Ok(Some(bytes))
        })();
        match result {
            Ok(value) => Ok(value),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Unsafe or unreadable game file: {error}")),
        }
    }

    pub(super) fn vacant(&self, path: &str) -> Result<(), String> {
        let (parent, leaf) = match self.parent(path, false, &mut Vec::new()) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        vacant_name(&parent, &leaf).map_err(|e| e.to_string())
    }

    pub(super) fn create(
        &self,
        path: &str,
        bytes: &[u8],
        created: &mut Vec<String>,
    ) -> Result<(), String> {
        self.vacant(path)?;
        let (parent, leaf) = self
            .parent(path, true, created)
            .map_err(|e| e.to_string())?;
        let mut file = open_at(
            &parent,
            &leaf,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )
        .map_err(|e| e.to_string())?;
        if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            if let Ok(metadata) = file.metadata() {
                if let Ok(Some(captured)) = self.capture(path) {
                    let _ = captured.remove_if_identity((metadata.dev(), metadata.ino()));
                }
            }
            return Err(error.to_string());
        }
        Ok(())
    }

    pub(super) fn update(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if path != super::MANIFEST {
            return Err("Only the ownership manifest can be replaced".into());
        }
        let (parent, leaf) = self
            .parent(path, false, &mut Vec::new())
            .map_err(|e| e.to_string())?;
        let file = open_at(&parent, &leaf, libc::O_RDONLY | libc::O_NONBLOCK)
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Manifest is not a regular file".into());
        }
        let temporary = format!(
            ".corkytux-bepinex.tmp-{}-{}",
            std::process::id(),
            NEXT_MANIFEST.fetch_add(1, Ordering::Relaxed)
        );
        self.create(&temporary, bytes, &mut Vec::new())?;
        let old = name(&temporary).map_err(|e| e.to_string())?;
        let new = name(&leaf).map_err(|e| e.to_string())?;
        // SAFETY: both names are bounded and refer to the owned game-directory capability.
        if unsafe {
            libc::renameat(
                parent.as_raw_fd(),
                old.as_ptr(),
                parent.as_raw_fd(),
                new.as_ptr(),
            )
        } < 0
        {
            let error = io::Error::last_os_error().to_string();
            let _ = self.remove(&temporary);
            return Err(error);
        }
        self.0.sync_all().map_err(|e| e.to_string())
    }

    pub(super) fn remove(&self, path: &str) -> Result<(), String> {
        let (parent, leaf) = self
            .parent(path, false, &mut Vec::new())
            .map_err(|e| e.to_string())?;
        let leaf = name(&leaf).map_err(|e| e.to_string())?;
        // SAFETY: parent is an owned directory capability; unlinkat never follows leaf symlinks.
        if unsafe { libc::unlinkat(parent.as_raw_fd(), leaf.as_ptr(), 0) } < 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        Ok(())
    }

    pub(super) fn remove_empty_directory(&self, path: &str) {
        if let Ok((parent, leaf)) = self.parent(path, false, &mut Vec::new()) {
            if let Ok(leaf) = name(&leaf) {
                // SAFETY: AT_REMOVEDIR removes only an empty directory, not a symlink target.
                unsafe {
                    libc::unlinkat(parent.as_raw_fd(), leaf.as_ptr(), libc::AT_REMOVEDIR);
                }
            }
        }
    }

    pub(super) fn identity(&self) -> Result<(u64, u64), String> {
        let metadata = self.0.metadata().map_err(|e| e.to_string())?;
        Ok((metadata.dev(), metadata.ino()))
    }
}
