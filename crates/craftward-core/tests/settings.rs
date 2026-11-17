//! Settings file, first-run detection, and workspace relocation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    Host, OsFamily, Settings, default_settings_file, is_first_run, load_settings_from,
    parse_settings, relocate_workspace_file, remove_relocated_file, save_settings_to,
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
        .map_or(0, |d| d.as_nanos());
    let root = std::env::temp_dir().join(format!("craftward-settings-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn first_run_when_settings_and_workspace_are_absent() {
    let root = scratch("first");
    let app_data = Box::leak(root.to_string_lossy().into_owned().into_boxed_str());
    let host = FakeHost {
        os: OsFamily::Windows,
        vars: vec![("APPDATA", app_data)],
        home: None,
    };
    assert!(is_first_run(&host, None).expect("first"));
    assert_eq!(
        default_settings_file(&host).unwrap(),
        root.join("Craftward").join("settings.json")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn not_first_run_when_settings_exist() {
    let root = scratch("has-settings");
    let config = root.join("Craftward");
    fs::create_dir_all(&config).unwrap();
    let settings_path = config.join("settings.json");
    save_settings_to(
        &settings_path,
        &Settings {
            workspace: Some(config.join("workspace.json")),
            ..Settings::default()
        },
    )
    .unwrap();
    let app_data = Box::leak(root.to_string_lossy().into_owned().into_boxed_str());
    let host = FakeHost {
        os: OsFamily::Windows,
        vars: vec![("APPDATA", app_data)],
        home: None,
    };
    assert!(!is_first_run(&host, None).expect("not first"));
    let loaded = load_settings_from(&settings_path).unwrap().unwrap();
    assert!(loaded.workspace.is_some());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn relocate_workspace_copies_then_source_can_be_removed() {
    let root = scratch("relocate");
    let from = root.join("old").join("workspace.json");
    let to = root.join("new").join("workspace.json");
    fs::create_dir_all(from.parent().unwrap()).unwrap();
    fs::write(
        &from,
        "{\n  \"schema_version\": 1,\n  \"projects\": []\n}\n",
    )
    .unwrap();
    relocate_workspace_file(&from, &to).unwrap();
    assert!(to.exists());
    assert!(from.exists());
    remove_relocated_file(&from).unwrap();
    assert!(!from.exists());
    assert!(to.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn relocate_workspace_refuses_existing_target() {
    let root = scratch("occupied");
    let from = root.join("from.json");
    let to = root.join("to.json");
    fs::write(&from, "{}\n").unwrap();
    fs::write(&to, "{}\n").unwrap();
    let error = relocate_workspace_file(&from, &to).expect_err("occupied");
    assert!(error.to_string().contains("already exists"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn parse_round_trip_through_save() {
    let root = scratch("roundtrip");
    let path = root.join("settings.json");
    let settings = Settings {
        workspace: Some(PathBuf::from("/w/workspace.json")),
        data_dir: Some(PathBuf::from("/d")),
        cache_dir: Some(PathBuf::from("/c")),
    };
    save_settings_to(&path, &settings).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    let parsed = parse_settings(&text).unwrap();
    assert_eq!(parsed, settings);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_settings_file_loads_as_none() {
    assert!(
        load_settings_from(Path::new("/definitely/missing/craftward-settings-xyz.json"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn complete_first_run_creates_workspace_and_settings() {
    use craftward_core::complete_first_run;

    let root = scratch("complete");
    let data = root.join("data");
    let cache = root.join("cache");
    let workspace = root.join("Craftward").join("workspace.json");
    let app_data = Box::leak(root.to_string_lossy().into_owned().into_boxed_str());
    let host = FakeHost {
        os: OsFamily::Windows,
        vars: vec![("APPDATA", app_data)],
        home: None,
    };
    assert!(is_first_run(&host, None).unwrap());
    complete_first_run(
        &host,
        &Settings {
            workspace: Some(workspace.clone()),
            data_dir: Some(data.clone()),
            cache_dir: Some(cache.clone()),
        },
    )
    .unwrap();
    assert!(workspace.exists());
    assert!(data.is_dir());
    assert!(cache.is_dir());
    assert!(!is_first_run(&host, None).unwrap());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolution_order_cli_env_settings_default() {
    use craftward_core::{
        PathOrigin, WorkspaceOrigin, locate_cache_with, locate_data_with, locate_workspace_with,
    };

    let host = FakeHost {
        os: OsFamily::Linux,
        vars: vec![],
        home: Some(PathBuf::from("/home/dev")),
    };
    let settings_ws = PathBuf::from("/settings/workspace.json");
    let settings_data = PathBuf::from("/settings/data");
    let settings_cache = PathBuf::from("/settings/cache");

    let from_settings = locate_workspace_with(&host, None, Some(&settings_ws)).expect("settings");
    assert_eq!(from_settings.origin, WorkspaceOrigin::Settings);

    let from_default = locate_workspace_with(&host, None, None).expect("default");
    assert_eq!(from_default.origin, WorkspaceOrigin::Default);

    let cli = PathBuf::from("/cli/workspace.json");
    let from_cli = locate_workspace_with(&host, Some(&cli), Some(&settings_ws)).expect("cli");
    assert_eq!(from_cli.origin, WorkspaceOrigin::Option);

    let env_host = FakeHost {
        os: OsFamily::Linux,
        vars: vec![("CRAFTWARD_WORKSPACE", "/env/workspace.json")],
        home: Some(PathBuf::from("/home/dev")),
    };
    let from_env = locate_workspace_with(&env_host, None, Some(&settings_ws)).expect("env");
    assert_eq!(from_env.origin, WorkspaceOrigin::Environment);

    let data = locate_data_with(&host, None, Some(&settings_data)).expect("data");
    assert_eq!(data.origin, PathOrigin::Settings);
    let cache = locate_cache_with(&host, None, Some(&settings_cache)).expect("cache");
    assert_eq!(cache.origin, PathOrigin::Settings);
}
