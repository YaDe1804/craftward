//! Workspace location, registry edits, and the project overview.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    Host, LocateError, OsFamily, ProjectStatus, Workspace, WorkspaceEntry, WorkspaceError,
    WorkspaceOrigin, add_project, init_project, list_projects, load_workspace, locate_cache,
    locate_data, locate_workspace, remove_project, save_workspace, snapshot,
};

struct FakeHost {
    os: OsFamily,
    vars: Vec<(&'static str, &'static str)>,
    home: Option<PathBuf>,
}

impl Host for FakeHost {
    fn var(&self, name: &str) -> Option<String> {
        self.vars
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }

    fn os(&self) -> OsFamily {
        self.os
    }
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("create scratch directory");
    path
}

fn minimal(id: &str) -> String {
    format!(
        "{{\n  \"schema_version\": 1,\n  \"id\": \"{id}\",\n  \"name\": \"{id}\",\n  \"status\": \"idea\"\n}}\n"
    )
}

#[test]
fn option_beats_the_environment_and_the_default() {
    let host = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("CRAFTWARD_WORKSPACE", "from-env.json")],
        home: Some(PathBuf::from("/home/ada")),
    };
    let location = locate_workspace(&host, Some(Path::new("from-option.json"))).expect("located");
    assert_eq!(location.path, PathBuf::from("from-option.json"));
    assert_eq!(location.origin, WorkspaceOrigin::Option);
}

#[test]
fn environment_beats_the_default() {
    let host = FakeHost {
        os: OsFamily::Windows,
        vars: vec![
            ("CRAFTWARD_WORKSPACE", "from-env.json"),
            ("APPDATA", r"C:\AppData"),
        ],
        home: None,
    };
    let location = locate_workspace(&host, None).expect("located");
    assert_eq!(location.path, PathBuf::from("from-env.json"));
    assert_eq!(location.origin, WorkspaceOrigin::Environment);
}

#[test]
fn empty_environment_variable_falls_through() {
    let host = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("CRAFTWARD_WORKSPACE", ""), ("XDG_CONFIG_HOME", "/xdg")],
        home: Some(PathBuf::from("/home/ada")),
    };
    let location = locate_workspace(&host, None).expect("located");
    assert_eq!(
        location.path,
        PathBuf::from("/xdg")
            .join("craftward")
            .join("workspace.json")
    );
    assert_eq!(location.origin, WorkspaceOrigin::Default);
}

#[test]
fn windows_default_uses_appdata() {
    let host = FakeHost {
        os: OsFamily::Windows,
        vars: vec![("APPDATA", r"C:\Users\ada\AppData\Roaming")],
        home: Some(PathBuf::from(r"C:\Users\ada")),
    };
    let location = locate_workspace(&host, None).expect("located");
    assert_eq!(
        location.path,
        PathBuf::from(r"C:\Users\ada\AppData\Roaming")
            .join("Craftward")
            .join("workspace.json")
    );
}

#[test]
fn windows_without_appdata_has_no_default() {
    let host = FakeHost {
        os: OsFamily::Windows,
        vars: Vec::new(),
        home: Some(PathBuf::from(r"C:\Users\ada")),
    };
    let error = locate_workspace(&host, None).expect_err("missing APPDATA");
    assert_eq!(error, LocateError::NoAppData);
}

#[test]
fn linux_uses_xdg_config_home() {
    let host = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("XDG_CONFIG_HOME", "/xdg")],
        home: Some(PathBuf::from("/home/ada")),
    };
    let location = locate_workspace(&host, None).expect("located");
    assert_eq!(
        location.path,
        PathBuf::from("/xdg")
            .join("craftward")
            .join("workspace.json")
    );
}

#[test]
fn linux_without_xdg_uses_the_home_config_directory() {
    let host = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("XDG_CONFIG_HOME", "")],
        home: Some(PathBuf::from("/home/ada")),
    };
    let location = locate_workspace(&host, None).expect("located");
    assert_eq!(
        location.path,
        PathBuf::from("/home/ada")
            .join(".config")
            .join("craftward")
            .join("workspace.json")
    );
}

#[test]
fn macos_default_uses_application_support() {
    let host = FakeHost {
        os: OsFamily::MacOs,
        vars: Vec::new(),
        home: Some(PathBuf::from("/Users/ada")),
    };
    let location = locate_workspace(&host, None).expect("located");
    assert_eq!(
        location.path,
        PathBuf::from("/Users/ada")
            .join("Library")
            .join("Application Support")
            .join("Craftward")
            .join("workspace.json")
    );
}

#[test]
fn macos_without_a_home_directory_has_no_default() {
    let host = FakeHost {
        os: OsFamily::MacOs,
        vars: Vec::new(),
        home: None,
    };
    let error = locate_workspace(&host, None).expect_err("missing home");
    assert_eq!(error, LocateError::NoHome);
}

#[test]
fn init_writes_a_valid_file_and_refuses_to_overwrite() {
    let root = scratch("init");
    let created =
        init_project(&root, "harbor-light", "Harbor Light", ProjectStatus::Idea).expect("init");
    let text = fs::read_to_string(&created).expect("read");
    assert!(text.ends_with('\n'));
    assert!(!text.contains('\r'));
    let project = craftward_core::load_from_str(&text).expect("valid project");
    assert_eq!(project.id, "harbor-light");
    assert_eq!(project.status, ProjectStatus::Idea);
    assert!(project.description.is_none());
    let error = init_project(&root, "other", "Other", ProjectStatus::Paused).expect_err("exists");
    assert!(matches!(error, WorkspaceError::AlreadyExists(_)));
    assert!(
        fs::read_to_string(&created)
            .unwrap()
            .contains("harbor-light")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_rejects_an_invalid_id_without_writing() {
    let root = scratch("init-bad");
    let error = init_project(&root, "-bad", "Name", ProjectStatus::Idea).expect_err("bad id");
    assert!(matches!(error, WorkspaceError::Validation(_)));
    assert!(!root.join("craftward.json").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_and_remove_update_the_registry() {
    let root = scratch("add");
    let project_dir = root.join("app");
    fs::create_dir_all(&project_dir).unwrap();
    init_project(
        &project_dir,
        "harbor-light",
        "Harbor Light",
        ProjectStatus::Idea,
    )
    .unwrap();
    let workspace = root.join("workspace.json");
    let id = add_project(&workspace, &project_dir).expect("add");
    assert_eq!(id, "harbor-light");
    let loaded = load_workspace(&workspace).expect("workspace");
    assert_eq!(loaded.projects.len(), 1);
    assert!(!Path::new(&loaded.projects[0].path).is_absolute());
    assert!(loaded.projects[0].path.ends_with("craftward.json"));
    let names: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|name| !name.contains(".tmp")),
        "temporary file left behind: {names:?}"
    );
    let duplicate = add_project(&workspace, &project_dir.join("craftward.json")).expect_err("dup");
    assert!(matches!(duplicate, WorkspaceError::DuplicatePath(_)));
    remove_project(&workspace, "harbor-light").expect("remove");
    assert_eq!(load_workspace(&workspace).unwrap().projects.len(), 0);
    let missing = remove_project(&workspace, "harbor-light").expect_err("gone");
    assert!(matches!(missing, WorkspaceError::UnknownId(_)));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_rejects_a_duplicate_id_and_an_invalid_file() {
    let root = scratch("dup-id");
    let first = root.join("one");
    let second = root.join("two");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(first.join("craftward.json"), minimal("harbor-light")).unwrap();
    fs::write(second.join("craftward.json"), minimal("harbor-light")).unwrap();
    let workspace = root.join("workspace.json");
    add_project(&workspace, &first).unwrap();
    let error = add_project(&workspace, &second).expect_err("duplicate id");
    assert!(matches!(error, WorkspaceError::DuplicateId(_)));
    assert_eq!(load_workspace(&workspace).unwrap().projects.len(), 1);

    let broken = root.join("broken.json");
    fs::write(&broken, "{").unwrap();
    let error = add_project(&workspace, &broken).expect_err("invalid");
    assert!(matches!(error, WorkspaceError::InvalidProject { .. }));
    assert_eq!(load_workspace(&workspace).unwrap().projects.len(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn relative_paths_resolve_from_the_workspace_directory() {
    let root = scratch("relative");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("craftward.json"), minimal("harbor-light")).unwrap();
    let workspace = root.join("workspace.json");
    save_workspace(
        &workspace,
        &Workspace {
            schema_version: 1,
            projects: vec![WorkspaceEntry {
                path: "nested/craftward.json".to_owned(),
            }],
        },
    )
    .unwrap();
    let rows = list_projects(&workspace).expect("list");
    let snapshot = rows[0].detail.as_ref().expect("readable");
    assert_eq!(snapshot.id, "harbor-light");
    assert_eq!(rows[0].path, nested.join("craftward.json"));
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn add_outside_the_workspace_stores_a_normal_absolute_path() {
    let root = scratch("outside");
    let registry = root.join("registry");
    let project_dir = root.join("project");
    fs::create_dir_all(&registry).unwrap();
    fs::create_dir_all(&project_dir).unwrap();
    let project = project_dir.join("craftward.json");
    fs::write(&project, minimal("harbor-light")).unwrap();
    let workspace = registry.join("workspace.json");
    add_project(&workspace, &project).expect("add");
    let text = fs::read_to_string(&workspace).expect("read workspace");
    let stored = &load_workspace(&workspace).expect("workspace").projects[0].path;
    let normal = std::path::absolute(&project).expect("absolute");
    let canonical = fs::canonicalize(&project).expect("canonical");
    assert_eq!(stored, &normal.to_string_lossy());
    assert!(Path::new(stored).is_absolute());
    assert!(
        !stored.starts_with(r"\\?\"),
        "verbatim prefix in stored path: {stored}"
    );
    assert!(
        canonical.to_string_lossy().starts_with(r"\\?\"),
        "expected canonicalize to use the verbatim prefix, got {canonical:?}"
    );
    assert_ne!(stored, &canonical.to_string_lossy());
    assert!(
        !text.contains(r"\\?\"),
        "workspace file contains a verbatim prefix:\n{text}"
    );
    let rows = list_projects(&workspace).expect("list");
    assert_eq!(rows[0].path, normal);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_reports_a_missing_file_and_keeps_going() {
    let root = scratch("missing");
    let workspace = root.join("workspace.json");
    save_workspace(
        &workspace,
        &Workspace {
            schema_version: 1,
            projects: vec![
                WorkspaceEntry {
                    path: "missing/craftward.json".to_owned(),
                },
                WorkspaceEntry {
                    path: "present/craftward.json".to_owned(),
                },
            ],
        },
    )
    .unwrap();
    fs::create_dir_all(root.join("present")).unwrap();
    fs::write(
        root.join("present").join("craftward.json"),
        minimal("present-app"),
    )
    .unwrap();
    let rows = list_projects(&workspace).expect("list");
    assert!(rows[0].detail.is_err());
    assert_eq!(rows[1].detail.as_ref().unwrap().id, "present-app");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn snapshot_counts_open_work_and_skips_finished_dates() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "in_development",
      "current_version": "0.4.0",
      "plan": {
        "phases": [
          {
            "id": "build",
            "title": "Build",
            "tasks": [
              { "id": "open-task", "title": "Open", "status": "open", "due": "2026-12-01" },
              { "id": "active", "title": "Active", "status": "in_progress" },
              { "id": "finished", "title": "Finished", "status": "done", "due": "2026-01-01" },
              { "id": "skipped", "title": "Skipped", "status": "dropped", "due": "2026-02-01" }
            ]
          }
        ]
      },
      "issues": [
        { "id": "open-issue", "title": "Open", "severity": "low", "status": "open" },
        { "id": "active-issue", "title": "Active", "severity": "high", "status": "in_progress" },
        { "id": "fixed", "title": "Fixed", "severity": "low", "status": "resolved" },
        { "id": "wont", "title": "Wont", "severity": "low", "status": "wont_fix" }
      ],
      "watch_items": [
        { "id": "review", "kind": "review", "title": "Review", "due": "2026-06-01" }
      ]
    }
    "#;
    let project = craftward_core::load_from_str(text).expect("project");
    let summary = snapshot(&project);
    assert_eq!(summary.open_tasks, 2);
    assert_eq!(summary.open_issues, 2);
    assert_eq!(summary.current_version.as_deref(), Some("0.4.0"));
    assert_eq!(summary.next_due.as_deref(), Some("2026-06-01"));
}

#[test]
fn workspace_file_rejects_unknown_fields_and_a_duplicate_path() {
    let root = scratch("bad-ws");
    let path = root.join("workspace.json");
    fs::write(&path, r#"{"schema_version":1,"projects":[],"extra":true}"#).unwrap();
    let error = load_workspace(&path).expect_err("unknown");
    match error {
        WorkspaceError::Validation(errors) => {
            assert!(errors.iter().any(|error| error.message == "unknown field"));
        }
        other => panic!("expected validation, got {other}"),
    }
    fs::write(
        &path,
        r#"{"schema_version":1,"projects":[{"path":"a.json"},{"path":"a.json"}]}"#,
    )
    .unwrap();
    let error = load_workspace(&path).expect_err("duplicate");
    match error {
        WorkspaceError::Validation(errors) => {
            assert!(errors.iter().any(|error| error.path == "projects[1].path"));
        }
        other => panic!("expected validation, got {other}"),
    }
    fs::write(&path, r#"{"schema_version":2,"projects":[]}"#).unwrap();
    let error = load_workspace(&path).expect_err("newer");
    assert!(matches!(
        error,
        WorkspaceError::UnsupportedSchema { version: 2 }
    ));
    assert_eq!(
        error.to_string(),
        "file was written by a newer Craftward version"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_directory_follows_the_option_then_the_environment_then_the_default() {
    let windows = FakeHost {
        os: OsFamily::Windows,
        vars: vec![
            ("CRAFTWARD_CACHE_DIR", ""),
            ("LOCALAPPDATA", "C:/Users/me/AppData/Local"),
        ],
        home: None,
    };
    let chosen = PathBuf::from("D:/explicit");
    assert_eq!(locate_cache(&windows, Some(&chosen)).unwrap(), chosen);
    assert_eq!(
        locate_cache(&windows, None).unwrap(),
        PathBuf::from("C:/Users/me/AppData/Local/Craftward/cache")
    );
    let linux = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("XDG_CACHE_HOME", "/var/cache")],
        home: Some(PathBuf::from("/home/me")),
    };
    assert_eq!(
        locate_cache(&linux, None).unwrap(),
        PathBuf::from("/var/cache/craftward")
    );
    let home_only = FakeHost {
        os: OsFamily::Linux,
        vars: vec![],
        home: Some(PathBuf::from("/home/me")),
    };
    assert_eq!(
        locate_cache(&home_only, None).unwrap(),
        PathBuf::from("/home/me/.cache/craftward")
    );
    let mac = FakeHost {
        os: OsFamily::MacOs,
        vars: vec![("CRAFTWARD_CACHE_DIR", "/tmp/craftward-cache")],
        home: Some(PathBuf::from("/Users/me")),
    };
    assert_eq!(
        locate_cache(&mac, None).unwrap(),
        PathBuf::from("/tmp/craftward-cache")
    );
    let data = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("XDG_DATA_HOME", "/var/lib")],
        home: Some(PathBuf::from("/home/me")),
    };
    assert_eq!(
        locate_data(&data, None).unwrap(),
        PathBuf::from("/var/lib/craftward")
    );
    let windows = FakeHost {
        os: OsFamily::Windows,
        vars: vec![("LOCALAPPDATA", "C:/Users/me/AppData/Local")],
        home: None,
    };
    assert_eq!(
        locate_data(&windows, None).unwrap(),
        PathBuf::from("C:/Users/me/AppData/Local/Craftward")
    );
}
