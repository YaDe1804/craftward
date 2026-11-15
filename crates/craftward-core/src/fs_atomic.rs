//! Durable file replacement.
//!
//! A temporary file is created with [`OpenOptions::create_new`](std::fs::OpenOptions::create_new),
//! so an existing path, including a symbolic link, is never opened or
//! overwritten. The bytes are written and [`File::sync_all`](std::fs::File::sync_all)
//! runs before [`rename`](std::fs::rename) publishes the file. On Unix the
//! parent directory is synced afterwards so the new directory entry itself
//! reaches disk. Windows skips that step: `File::open` on a directory fails
//! there, and a same-volume rename is a single metadata operation
//! (`MoveFileEx`). The file contents were already synced before the rename.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How many distinct temporary names are tried when one is already taken.
const TEMPORARY_ATTEMPTS: u32 = 8;

/// Replaces `path` with `bytes`.
///
/// Parent directories are created. A crash cannot leave the destination
/// half-written: the temporary file is synced, then renamed. When a temporary
/// name already exists, another name is tried, up to [`TEMPORARY_ATTEMPTS`].
/// Any error removes the temporary file this call created.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    publish(
        path,
        bytes,
        (0..TEMPORARY_ATTEMPTS).map(|attempt| temporary_path(path, attempt)),
    )
}

/// Writes `bytes` to a new temporary file next to `destination` and syncs it.
///
/// The caller publishes that file, for example with a hard link, and then
/// removes it. An existing path of the same name is left untouched.
pub(crate) fn create_temporary(destination: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let mut last = occupied();
    for attempt in 0..TEMPORARY_ATTEMPTS {
        let temporary = temporary_path(destination, attempt);
        match write_new(&temporary, bytes) {
            Ok(()) => return Ok(temporary),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => last = error,
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

/// Creates `path` and writes `bytes` only when nothing is already there.
///
/// The file is synced before this returns. A failure removes the file this
/// call created.
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_new_with(path, |file| file.write_all(bytes))
}

/// Creates `path` exclusively and runs `write_body`, then syncs the file.
///
/// `create_new` fails when the path exists, and that file is not removed.
/// A file this function created is removed when `write_body` or the sync fails.
pub(crate) fn write_new_with(
    path: &Path,
    write_body: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = write_body(&mut file).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

fn publish(path: &Path, bytes: &[u8], names: impl IntoIterator<Item = PathBuf>) -> io::Result<()> {
    let mut last = occupied();
    for temporary in names.into_iter().take(TEMPORARY_ATTEMPTS as usize) {
        match write_new(&temporary, bytes) {
            Ok(()) => {
                if let Err(error) = fs::rename(&temporary, path) {
                    let _ = fs::remove_file(&temporary);
                    return Err(error);
                }
                return sync_parent(path);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => last = error,
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

fn occupied() -> io::Error {
    io::Error::new(ErrorKind::AlreadyExists, "temporary file already exists")
}

fn temporary_path(path: &Path, attempt: u32) -> PathBuf {
    let name = path.file_name().unwrap_or_else(|| OsStr::new("file"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut temporary_name = OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".{}.{nanos}.{attempt}.tmp", std::process::id()));
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(temporary_name),
        _ => PathBuf::from(temporary_name),
    }
}

/// Unix records the rename in the parent directory. Syncing that directory
/// makes the new name durable across a power loss.
#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()
}

/// Windows has no directory `fsync` on the handle `File::open` can return:
/// opening a directory fails. A same-volume rename is one metadata update,
/// and the file contents were synced before it, so this step is omitted.
/// The `Result` matches the Unix function so callers can use `?`.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn sync_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{publish, write_atomic, write_new, write_new_with};

    fn scratch(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!("craftward-atomic-{name}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn the_destination_ends_with_the_new_bytes_and_no_temporary_file() {
        let root = scratch("replace");
        let destination = root.join("craftward.json");
        write_atomic(&destination, b"first\n").unwrap();
        write_atomic(&destination, b"second\n").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"second\n");
        let names = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 1, "{names:?}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_failed_write_leaves_no_temporary_file() {
        let root = scratch("fail");
        let path = root.join(".craftward.json.partial.tmp");
        let error = write_new_with(&path, |_| {
            Err(std::io::Error::new(ErrorKind::StorageFull, "full"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::StorageFull);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_existing_temporary_name_is_not_overwritten() {
        let root = scratch("occupied");
        let destination = root.join("craftward.json");
        let occupied = root.join(".craftward.json.occupied.tmp");
        let fresh = root.join(".craftward.json.fresh.tmp");
        fs::write(&occupied, b"keep").unwrap();
        fs::write(&destination, b"old").unwrap();
        publish(&destination, b"new", [occupied.clone(), fresh.clone()]).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new");
        assert_eq!(fs::read(&occupied).unwrap(), b"keep");
        assert!(!fresh.exists());
        let error = write_new(&occupied, b"changed").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&occupied).unwrap(), b"keep");
        let _ = fs::remove_dir_all(root);
    }
}
