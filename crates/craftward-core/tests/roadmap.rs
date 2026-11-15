//! Roadmap numbering, progress, escaping, languages, and the golden file.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    Language, PhaseState, Task, TaskStatus, load_from_path, load_from_str, next_roadmap_version,
    phase_state, render_roadmap, task_progress, write_roadmap,
};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-roadmap-{name}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn task(title: &str, status: TaskStatus) -> Task {
    Task {
        id: "task".to_owned(),
        title: title.to_owned(),
        status,
        due: None,
        note: None,
    }
}

#[test]
fn next_version_starts_at_one_and_skips_gaps() {
    let root = scratch("numbers");
    assert_eq!(next_roadmap_version(&root, "harbor-light"), 1);
    fs::write(root.join("harbor-light_roadmap_v1.html"), "v1").unwrap();
    fs::write(root.join("harbor-light_roadmap_v3.html"), "v3").unwrap();
    fs::write(root.join("other_roadmap_v9.html"), "other").unwrap();
    assert_eq!(next_roadmap_version(&root, "harbor-light"), 4);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn writing_a_roadmap_never_overwrites() {
    let root = scratch("write");
    let project = root.join("craftward.json");
    fs::write(
        &project,
        r#"{"schema_version":1,"id":"harbor-light","name":"Harbor","status":"idea"}"#,
    )
    .unwrap();
    let directory = root.join("out");
    let first = write_roadmap(&project, &directory, Language::En, "2026-11-15").unwrap();
    assert_eq!(first.file_name().unwrap(), "harbor-light_roadmap_v1.html");
    let original = fs::read_to_string(&first).unwrap();
    let second = write_roadmap(&project, &directory, Language::En, "2026-11-15").unwrap();
    assert_eq!(second.file_name().unwrap(), "harbor-light_roadmap_v2.html");
    assert_eq!(fs::read_to_string(&first).unwrap(), original);
    let names: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|name| !name.contains(".tmp")), "{names:?}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn progress_ignores_dropped_tasks() {
    let tasks = [
        task("finished", TaskStatus::Done),
        task("skipped", TaskStatus::Dropped),
        task("left", TaskStatus::Open),
    ];
    let progress = task_progress(&tasks);
    assert_eq!(progress.done, 1);
    assert_eq!(progress.total, 2);
    assert_eq!(progress.percent, 50);
    assert_eq!(phase_state(&tasks), PhaseState::InProgress);
    assert_eq!(
        phase_state(&[task("skip", TaskStatus::Dropped)]),
        PhaseState::Open
    );
    assert_eq!(
        phase_state(&[task("finished", TaskStatus::Done)]),
        PhaseState::Done
    );
    let two_thirds = task_progress(&[
        task("a", TaskStatus::Done),
        task("b", TaskStatus::Done),
        task("c", TaskStatus::Open),
    ]);
    assert_eq!(two_thirds.percent, 67);
}

#[test]
fn project_text_is_escaped_and_both_languages_keep_it() {
    let project = load_from_str(
        r#"{"schema_version":1,"id":"harbor-light","name":"Harbor <script>alert('x')</script>","status":"idea","plan":{"phases":[{"id":"one","title":"Phase","tasks":[{"id":"task","title":"A & B","status":"done"}]}]}}"#,
    )
    .unwrap();
    let english = render_roadmap(&project, 2, "2026-11-15", Language::En);
    let german = render_roadmap(&project, 2, "2026-11-15", Language::De);
    for html in [&english, &german] {
        assert!(html.contains("2026-11-15"), "{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script"));
        assert!(html.contains("A &amp; B"));
        assert!(html.contains("&#39;"));
        assert!(html.contains("rel=\"noopener noreferrer\"") || !html.contains("<a "));
    }
    assert!(english.contains("Overall progress"));
    assert!(german.contains("Gesamtfortschritt"));
    assert!(!english.contains("Gesamtfortschritt"));
    assert!(english.contains("Harbor &lt;script&gt;"));
    assert!(german.contains("Harbor &lt;script&gt;"));
}

#[test]
fn golden_file_matches_the_full_example() {
    let project_file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/full.craftward.json");
    let project = load_from_path(&project_file).expect("example");
    let html = render_roadmap(&project, 1, "2026-11-15", Language::En);
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/full_roadmap_en.html");
    let expected = fs::read_to_string(&fixture).expect("golden file");
    assert_eq!(html, expected);
}
