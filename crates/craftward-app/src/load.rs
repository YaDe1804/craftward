//! Load workspace projects and open findings for the desktop UI.
//!
//! This module only calls `craftward-core` and `craftward-store`. It does not
//! invent file or database formats of its own.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use craftward_core::{
    ContentFingerprint, DATABASE_FILE, ProcessHost, Project, Severity, StateFilter, StoredFinding,
    content_fingerprint, list_findings, list_projects, load_from_str, locate_data,
    locate_workspace, platform_path,
};
use craftward_store::SqliteStore;

/// Whether the local findings database could be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseStatus {
    /// Open findings were loaded.
    Ready,
    /// `craftward.db` is not present yet.
    Missing,
    /// The file exists but could not be opened or queried.
    Error(String),
}

/// One workspace entry after loading its project file.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    /// Absolute project file path.
    pub path: PathBuf,
    /// Parsed project, or why it could not be loaded.
    pub project: Result<Project, String>,
    /// Fingerprint of the file bytes when the project loaded successfully.
    pub fingerprint: Option<ContentFingerprint>,
    /// Open findings for this project's id (empty when the project failed).
    pub findings: Vec<StoredFinding>,
}

/// Everything the UI needs for one Reload.
#[derive(Debug, Clone)]
pub struct LoadedPortfolio {
    /// Absolute workspace file path, when it could be resolved.
    pub workspace_path: Option<PathBuf>,
    /// Why the workspace file could not be used.
    pub workspace_error: Option<String>,
    /// Absolute path of `craftward.db`, when the data directory resolved.
    pub database_path: Option<PathBuf>,
    /// Database load outcome.
    pub database: DatabaseStatus,
    /// Projects in workspace order (UI sorting happens in the view model).
    pub projects: Vec<LoadedProject>,
}

/// Loads the portfolio using the same path rules as the CLI.
#[must_use]
pub fn load_portfolio() -> LoadedPortfolio {
    let workspace = match locate_workspace(&ProcessHost, None) {
        Ok(location) => match absolute_path(&location.path) {
            Ok(path) => Ok(path),
            Err(message) => Err(message),
        },
        Err(error) => Err(error.to_string()),
    };

    let data_dir = match locate_data(&ProcessHost, None) {
        Ok(path) => absolute_path(&path),
        Err(error) => Err(error.to_string()),
    };

    let (database_path, database, findings_by_project) = match data_dir {
        Ok(dir) => {
            let db_path = dir.join(DATABASE_FILE);
            let (status, map) = load_open_findings(&db_path);
            (Some(db_path), status, map)
        }
        Err(message) => (
            None,
            DatabaseStatus::Error(message),
            BTreeMap::<String, Vec<StoredFinding>>::new(),
        ),
    };

    match workspace {
        Ok(workspace_path) => {
            let rows = match list_projects(&workspace_path) {
                Ok(rows) => rows,
                Err(error) => {
                    return LoadedPortfolio {
                        workspace_path: Some(workspace_path),
                        workspace_error: Some(error.to_string()),
                        database_path,
                        database,
                        projects: Vec::new(),
                    };
                }
            };
            let projects = rows
                .into_iter()
                .map(|row| load_project_entry(row.path, &findings_by_project))
                .collect();
            LoadedPortfolio {
                workspace_path: Some(workspace_path),
                workspace_error: None,
                database_path,
                database,
                projects,
            }
        }
        Err(message) => LoadedPortfolio {
            workspace_path: None,
            workspace_error: Some(message),
            database_path,
            database,
            projects: Vec::new(),
        },
    }
}

fn load_project_entry(
    path: PathBuf,
    findings_by_project: &BTreeMap<String, Vec<StoredFinding>>,
) -> LoadedProject {
    match fs::read(&path) {
        Ok(bytes) => {
            let fingerprint = content_fingerprint(&bytes);
            match String::from_utf8(bytes) {
                Ok(text) => match load_from_str(&text) {
                    Ok(project) => {
                        let findings = findings_by_project
                            .get(&project.id)
                            .cloned()
                            .unwrap_or_default();
                        LoadedProject {
                            path,
                            project: Ok(project),
                            fingerprint: Some(fingerprint),
                            findings,
                        }
                    }
                    Err(error) => LoadedProject {
                        path,
                        project: Err(error.to_string()),
                        fingerprint: None,
                        findings: Vec::new(),
                    },
                },
                Err(error) => LoadedProject {
                    path,
                    project: Err(format!("invalid UTF-8 in project file: {error}")),
                    fingerprint: None,
                    findings: Vec::new(),
                },
            }
        }
        Err(error) => LoadedProject {
            path,
            project: Err(error.to_string()),
            fingerprint: None,
            findings: Vec::new(),
        },
    }
}

fn load_open_findings(path: &Path) -> (DatabaseStatus, BTreeMap<String, Vec<StoredFinding>>) {
    if !path.exists() {
        return (DatabaseStatus::Missing, BTreeMap::new());
    }
    let mut store = match SqliteStore::open(path) {
        Ok(store) => store,
        Err(error) => return (DatabaseStatus::Error(error.to_string()), BTreeMap::new()),
    };
    match list_findings(&mut store, &[], StateFilter::Open, None) {
        Ok(rows) => {
            let mut map = BTreeMap::<String, Vec<StoredFinding>>::new();
            for row in rows {
                map.entry(row.project_id.clone()).or_default().push(row);
            }
            for findings in map.values_mut() {
                findings.sort_by(|left, right| {
                    left.severity
                        .cmp(&right.severity)
                        .then(left.package.cmp(&right.package))
                        .then(left.vulnerability_id.cmp(&right.vulnerability_id))
                });
            }
            (DatabaseStatus::Ready, map)
        }
        Err(error) => (DatabaseStatus::Error(error.to_string()), BTreeMap::new()),
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    let path = platform_path(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        let current = env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?;
        Ok(platform_path(&current.join(path)))
    }
}

/// Counts open findings by severity for tests and the view model.
#[must_use]
pub fn count_by_severity(findings: &[StoredFinding]) -> [usize; 5] {
    let mut counts = [0_usize; 5];
    for finding in findings {
        let index = match finding.severity {
            Severity::Critical => 0,
            Severity::High => 1,
            Severity::Medium => 2,
            Severity::Low => 3,
            Severity::Unknown => 4,
        };
        counts[index] += 1;
    }
    counts
}
