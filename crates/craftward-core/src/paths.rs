//! Resolve `source_dir` and lockfile paths to system paths.

use std::path::{Path, PathBuf};

use crate::model::Project;

/// A source directory and the lockfiles that were resolved against it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    /// The source directory. This is `source_dir` when set, otherwise the
    /// directory that contains the project file.
    pub source_dir: PathBuf,
    /// Lockfiles, in file order, resolved from [`Self::source_dir`].
    pub lockfiles: Vec<PathBuf>,
}

/// A path named by the project file that is not on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingPath {
    /// Whether the missing path is the source directory or a lockfile.
    pub kind: MissingPathKind,
    /// The resolved system path.
    pub path: PathBuf,
}

/// Which recorded path was missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPathKind {
    /// `source_dir` does not name a directory.
    SourceDir,
    /// A lockfile path does not name a file.
    Lockfile,
}

impl std::fmt::Display for MissingPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            MissingPathKind::SourceDir => {
                write!(
                    formatter,
                    "source directory not found: {}",
                    self.path.display()
                )
            }
            MissingPathKind::Lockfile => {
                write!(formatter, "lockfile not found: {}", self.path.display())
            }
        }
    }
}

/// Resolves `source_dir` and each lockfile against `project_file`.
///
/// Relative paths use the separator of this operating system. The stored text
/// may contain `/`. This function does not check that the paths exist.
#[must_use]
pub fn resolve_project_paths(project_file: &Path, project: &Project) -> ProjectPaths {
    let base = project_directory(project_file);
    let source_dir = match &project.source_dir {
        Some(stored) if is_portable_absolute(stored) => platform_path(Path::new(stored)),
        Some(stored) => join_stored(&base, stored),
        None => base,
    };
    let lockfiles = project
        .stack
        .iter()
        .flat_map(|stack| stack.lockfiles.iter())
        .map(|lockfile| join_stored(&source_dir, lockfile))
        .collect();
    ProjectPaths {
        source_dir,
        lockfiles,
    }
}

/// Reports a source directory that is not a directory, and lockfiles that are
/// not files.
///
/// Loading a project file does not call this. A file can be valid on a machine
/// where these paths are absent.
#[must_use]
pub fn missing_paths(project_file: &Path, project: &Project) -> Vec<MissingPath> {
    let resolved = resolve_project_paths(project_file, project);
    let mut missing = Vec::new();
    if !resolved.source_dir.is_dir() {
        missing.push(MissingPath {
            kind: MissingPathKind::SourceDir,
            path: resolved.source_dir,
        });
    }
    for path in resolved.lockfiles {
        if !path.is_file() {
            missing.push(MissingPath {
                kind: MissingPathKind::Lockfile,
                path,
            });
        }
    }
    missing
}

/// Reports whether `text` is absolute on Windows or Unix, independent of the
/// host. A path is absolute when it starts with `/` or `\`, or with an ASCII
/// letter and `:`.
#[must_use]
pub fn is_portable_absolute(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes
        .first()
        .is_some_and(|byte| *byte == b'/' || *byte == b'\\')
    {
        return true;
    }
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Rebuilds `path` so every separator is the one this operating system uses.
///
/// Joining a stored piece such as `app/craftward.json` in one step keeps the
/// `/` on Windows. Collecting components writes the platform separator
/// between them.
#[must_use]
pub fn platform_path(path: &Path) -> PathBuf {
    path.components().collect()
}

fn project_directory(project_file: &Path) -> PathBuf {
    match project_file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => platform_path(parent),
        _ => platform_path(Path::new(".")),
    }
}

fn join_stored(base: &Path, stored: &str) -> PathBuf {
    platform_path(&base.join(platform_path(Path::new(stored))))
}
