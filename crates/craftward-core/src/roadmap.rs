//! Versioned HTML roadmaps rendered from a project file.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::LoadError;
use crate::i18n::{Label, Language, fill, text};
use crate::load::load_from_path;
use crate::model::{
    ComponentKind, Issue, IssueSeverity, IssueStatus, Phase, Project, ProjectStatus, Task,
    TaskStatus, WatchItem, WatchKind,
};

const STYLE: &str = r#"
  :root{
    --bg:#0e1116; --card:#161b22; --card2:#1c232c; --line:#2a323d;
    --text:#e6edf3; --muted:#8b98a7;
    --done:#3fb950; --run:#d29922; --open:#6e7b8b; --accent:#58a6ff;
  }
  *{box-sizing:border-box}
  html{overflow-wrap:anywhere}
  body{margin:0;background:var(--bg);color:var(--text);
    font:15px/1.55 "Segoe UI",system-ui,-apple-system,Roboto,sans-serif}
  .wrap{max-width:1180px;margin:0 auto;padding:32px 20px 56px}
  header{display:flex;flex-wrap:wrap;gap:16px;align-items:flex-end;justify-content:space-between;margin-bottom:22px}
  h1{margin:0;font-size:30px;letter-spacing:.2px}
  .sub{color:var(--muted);margin-top:4px}
  .meta{color:var(--muted);font-size:13px;text-align:right}
  .card{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:18px 20px;min-width:0}
  .overall{margin-bottom:22px}
  .overall .row{display:flex;justify-content:space-between;align-items:baseline;gap:12px;margin-bottom:10px}
  .overall b{font-size:22px}
  .bar{height:10px;background:#232b35;border-radius:99px;overflow:hidden}
  .bar i{display:block;height:100%;background:linear-gradient(90deg,var(--done),#56d364);border-radius:99px}
  .legend{display:flex;gap:18px;flex-wrap:wrap;margin-top:12px;color:var(--muted);font-size:13px}
  .dot{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:6px}
  .grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,340px),1fr));gap:16px}
  .phase h2{margin:0;font-size:17px}
  .phase .top{display:flex;justify-content:space-between;gap:10px;align-items:flex-start;margin-bottom:4px}
  .phase .goal{color:var(--muted);font-size:13.5px;margin:2px 0 12px}
  .tag{font-size:11.5px;font-weight:600;padding:3px 9px;border-radius:99px;border:1px solid}
  .tag.run{color:var(--run);border-color:#5c4613;background:#2a2109}
  .tag.done{color:var(--done);border-color:#1f5c2c;background:#0f2a16}
  .tag.open{color:var(--muted);border-color:var(--line);background:var(--card2)}
  .num{color:var(--accent);font-weight:700;margin-right:6px}
  ul{list-style:none;margin:12px 0 0;padding:0}
  li{display:flex;gap:10px;padding:6px 0;border-top:1px solid #212933;font-size:14px}
  li:first-child{border-top:0}
  .st{flex:0 0 16px;height:16px;margin-top:3px;border-radius:4px;border:2px solid var(--open)}
  .st.done{background:var(--done);border-color:var(--done);position:relative}
  .st.done::after{content:"";position:absolute;left:3px;top:0;width:4px;height:8px;border:solid #0e1116;border-width:0 2px 2px 0;transform:rotate(45deg)}
  .st.run{border-color:var(--run);background:repeating-linear-gradient(45deg,#3a2e0c,#3a2e0c 3px,transparent 3px,transparent 6px)}
  .st.dropped{border-color:var(--open);opacity:.55}
  li.done span.t{color:var(--muted)}
  li.dropped span.t{color:var(--muted);text-decoration:line-through}
  .pct{font-size:12.5px;color:var(--muted);display:flex;justify-content:space-between;gap:12px;margin-bottom:5px}
  h3{font-size:13px;text-transform:uppercase;letter-spacing:1.2px;color:var(--muted);margin:30px 0 12px}
  table{width:100%;border-collapse:collapse;font-size:14px}
  td,th{text-align:left;padding:8px 10px;border-top:1px solid #212933;vertical-align:top}
  th{color:var(--muted);font-weight:600;border-top:0;font-size:12.5px}
  td:first-child{font-weight:600}
"#;

/// How far the counted tasks have come.
///
/// Tasks marked `dropped` are not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskProgress {
    /// Tasks with status `done`.
    pub done: usize,
    /// Tasks that are not `dropped`.
    pub total: usize,
    /// `done` as a percentage of `total`, rounded to the nearest integer.
    ///
    /// Zero when `total` is zero.
    pub percent: u32,
}

/// State of a phase, derived from its tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseState {
    /// No counted task is `done` or `in_progress`. An empty phase and a phase
    /// whose only tasks are `dropped` are open.
    Open,
    /// At least one counted task is `done` or `in_progress`, and not every
    /// counted task is `done`.
    InProgress,
    /// Every counted task is `done`.
    Done,
}

/// Why a roadmap could not be written.
#[derive(Debug)]
pub enum RoadmapError {
    /// The project file could not be read or is invalid.
    Project(LoadError),
    /// The destination could not be created.
    Write(io::Error),
    /// The chosen file name was already taken.
    AlreadyExists(PathBuf),
}

impl std::fmt::Display for RoadmapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(error) => write!(formatter, "{error}"),
            Self::Write(error) => write!(formatter, "failed to write roadmap: {error}"),
            Self::AlreadyExists(path) => {
                write!(formatter, "file already exists: {}", path.display())
            }
        }
    }
}

impl std::error::Error for RoadmapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::AlreadyExists(_) => None,
        }
    }
}

/// Counts tasks other than `dropped`.
#[must_use]
pub fn task_progress(tasks: &[Task]) -> TaskProgress {
    progress_from(tasks.iter())
}

/// Derives a phase state from its tasks.
#[must_use]
pub fn phase_state(tasks: &[Task]) -> PhaseState {
    let progress = task_progress(tasks);
    let started = tasks
        .iter()
        .any(|task| matches!(task.status, TaskStatus::Done | TaskStatus::InProgress));
    if progress.total > 0 && progress.done == progress.total {
        PhaseState::Done
    } else if started {
        PhaseState::InProgress
    } else {
        PhaseState::Open
    }
}

/// Renders a standalone HTML roadmap.
///
/// `number` is the roadmap version written into the page. `generated_on` is
/// the generation date, already formatted as `YYYY-MM-DD`. Fixed labels come
/// from the language table. Project text is escaped.
#[must_use]
pub fn render_roadmap(
    project: &Project,
    number: u32,
    generated_on: &str,
    language: Language,
) -> String {
    let mut page = Page {
        language,
        out: String::new(),
    };
    page.write_document(project, number, generated_on);
    page.out
}

/// The next `vN` for `{id}_roadmap_vN.html` in `directory`.
///
/// `N` is one higher than the highest existing number, or 1 when none exist.
/// Gaps are not filled: `v1` and `v3` produce `v4`.
#[must_use]
pub fn next_roadmap_version(directory: &Path, id: &str) -> u32 {
    let prefix = format!("{id}_roadmap_v");
    let mut highest = 0_u32;
    let Ok(entries) = fs::read_dir(directory) else {
        return 1;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(number) = version_number(&prefix, name) else {
            continue;
        };
        highest = highest.max(number);
    }
    highest.saturating_add(1)
}

/// Writes the next roadmap HTML file for `project_file` into `directory`.
///
/// The file is `{id}_roadmap_vN.html`. An existing file is never replaced.
/// The bytes are written to a temporary file in the same directory and then
/// hard-linked into place. When that link fails because the file system does
/// not support hard links, the destination is created exclusively and removed
/// again if the write fails.
///
/// # Errors
///
/// Returns an error when the project file is unreadable or invalid, or when
/// the destination cannot be created.
pub fn write_roadmap(
    project_file: &Path,
    directory: &Path,
    language: Language,
    generated_on: &str,
) -> Result<PathBuf, RoadmapError> {
    let project = load_from_path(project_file).map_err(RoadmapError::Project)?;
    fs::create_dir_all(directory).map_err(RoadmapError::Write)?;
    let number = next_roadmap_version(directory, &project.id);
    let destination = directory.join(format!("{}_roadmap_v{number}.html", project.id));
    let html = render_roadmap(&project, number, generated_on, language);
    write_new_file(&destination, html.as_bytes())?;
    Ok(destination)
}

fn progress_from<'a>(tasks: impl Iterator<Item = &'a Task>) -> TaskProgress {
    let mut done = 0;
    let mut total = 0;
    for task in tasks {
        if task.status == TaskStatus::Dropped {
            continue;
        }
        total += 1;
        if task.status == TaskStatus::Done {
            done += 1;
        }
    }
    TaskProgress {
        done,
        total,
        percent: percent(done, total),
    }
}

fn project_progress(project: &Project) -> TaskProgress {
    progress_from(
        project
            .plan
            .iter()
            .flat_map(|plan| &plan.phases)
            .flat_map(|phase| &phase.tasks),
    )
}

fn percent(done: usize, total: usize) -> u32 {
    done.checked_mul(100)
        .and_then(|value| value.checked_add(total / 2))
        .and_then(|value| value.checked_div(total))
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

fn version_number(prefix: &str, name: &str) -> Option<u32> {
    let rest = name.strip_prefix(prefix)?.strip_suffix(".html")?;
    if rest.is_empty() || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), RoadmapError> {
    write_new_file_with(
        path,
        bytes,
        |original, link| fs::hard_link(original, link),
        write_exclusive,
    )
}

/// Publishes `bytes` at `path` without replacing an existing file.
///
/// `link` is the hard-link step. `create` is the exclusive-create fallback.
/// Tests pass substitutes so a file system without hard links is not required.
fn write_new_file_with(
    path: &Path,
    bytes: &[u8],
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
    create: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
) -> Result<(), RoadmapError> {
    if path.exists() {
        return Err(RoadmapError::AlreadyExists(path.to_path_buf()));
    }
    let temporary = temporary_path(path);
    if let Err(error) = fs::write(&temporary, bytes) {
        let _ = fs::remove_file(&temporary);
        return Err(RoadmapError::Write(error));
    }
    // `hard_link` fails when the destination exists, so a completed file is
    // never replaced. The name appears only after the bytes are on disk.
    if let Err(error) = link(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return match publish_choice(&error) {
            PublishChoice::CreateNew => create(path, bytes).map_err(RoadmapError::Write),
            PublishChoice::AlreadyExists => Err(RoadmapError::AlreadyExists(path.to_path_buf())),
            PublishChoice::Fail => Err(RoadmapError::Write(error)),
        };
    }
    fs::remove_file(&temporary).map_err(RoadmapError::Write)
}

/// What to do after `hard_link` fails.
#[derive(Debug, PartialEq, Eq)]
enum PublishChoice {
    /// The file system does not support hard links. Create the file exclusively.
    CreateNew,
    /// The destination appeared before the link. Leave it untouched.
    AlreadyExists,
    /// Any other failure, including permissions and a full disk.
    Fail,
}

fn publish_choice(error: &io::Error) -> PublishChoice {
    if hard_link_unsupported(error) {
        PublishChoice::CreateNew
    } else if error.kind() == ErrorKind::AlreadyExists {
        PublishChoice::AlreadyExists
    } else {
        PublishChoice::Fail
    }
}

/// Whether `error` is one of the operating-system codes for a file system
/// that cannot create hard links.
///
/// Windows, from `CreateHardLinkW` (NTFS only) and system error codes 0–499:
/// `ERROR_INVALID_FUNCTION` (1, "Incorrect function"), `ERROR_NOT_SUPPORTED`
/// (50), and `ERROR_CALL_NOT_IMPLEMENTED` (120). FAT32 and exFAT have no hard
/// links. Rust 1.100 maps only 120 to [`ErrorKind::Unsupported`], so the raw
/// code decides. `ERROR_ACCESS_DENIED` (5) and `ERROR_NOT_SAME_DEVICE` (17)
/// stay failures.
///
/// Linux, from `link(2)` in man-pages 6.19: `EPERM` (1) when the file system
/// does not support hard links. The same page also uses `EPERM` for a
/// directory, an immutable file, and `protected_hardlinks`; the code alone
/// cannot separate those, so they take this path too. `EOPNOTSUPP` (95, the
/// same value as `ENOTSUP` on Linux) is the errno for an unsupported
/// operation and is what some network file systems return. `EACCES` (13) and
/// `EXDEV` (18) stay failures.
///
/// macOS, from `intro(2)` and XNU `err_link`: `ENOTSUP` (45) when the file
/// system does not implement the link operation. `EOPNOTSUPP` (102) is the
/// distinct Darwin value for an unsupported operation. `link(2)` uses `EPERM`
/// only when the source is a directory, so it stays a failure, as do
/// `EACCES` (13) and `EXDEV` (18).
fn hard_link_unsupported(error: &io::Error) -> bool {
    error
        .raw_os_error()
        .is_some_and(is_unsupported_hard_link_code)
}

fn is_unsupported_hard_link_code(code: i32) -> bool {
    unsupported_hard_link_codes().contains(&code)
}

fn unsupported_hard_link_codes() -> &'static [i32] {
    #[cfg(windows)]
    {
        &[1, 50, 120]
    }
    #[cfg(all(unix, target_os = "linux"))]
    {
        &[1, 95]
    }
    #[cfg(all(unix, target_os = "macos"))]
    {
        &[45, 102]
    }
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    {
        &[1, 95]
    }
    #[cfg(not(any(windows, unix)))]
    {
        &[]
    }
}

/// Creates `path` only when it is absent, then writes `bytes`.
///
/// `OpenOptions::create_new` fails without changing a file that is already
/// there. A file this function created is removed when the write fails.
fn write_exclusive(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_created(path, |file| file.write_all(bytes))
}

fn write_created(
    path: &Path,
    write_body: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(error) = write_body(&mut file) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("roadmap.html"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut temporary_name = std::ffi::OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".{}.{nanos}.tmp", std::process::id()));
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(temporary_name),
        _ => PathBuf::from(temporary_name),
    }
}

struct Page {
    language: Language,
    out: String,
}

impl Page {
    fn write_document(&mut self, project: &Project, number: u32, generated_on: &str) {
        let number = number.to_string();
        let name = escape(&project.name);
        let roadmap = text(self.language, Label::Roadmap);
        let title = fill(
            self.language,
            Label::DocumentTitle,
            &[("name", &name), ("roadmap", roadmap), ("number", &number)],
        );
        let _ = write!(
            self.out,
            "<!DOCTYPE html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<div class=\"wrap\">\n",
            self.language.code(),
            title
        );
        self.write_header(project, &number, generated_on, roadmap);
        self.write_overall(project);
        self.write_phases(project);
        self.write_issues(project);
        self.write_watch_items(project);
        self.write_stack(project);
        self.write_links(project);
        self.out.push_str("</div>\n</body>\n</html>\n");
    }

    fn write_header(&mut self, project: &Project, number: &str, generated_on: &str, roadmap: &str) {
        let meta = fill(
            self.language,
            Label::MetaLine,
            &[
                ("roadmap", roadmap),
                ("number", number),
                ("as_of", text(self.language, Label::AsOf)),
                ("date", &escape(generated_on)),
            ],
        );
        let status = fill(
            self.language,
            Label::FieldLine,
            &[
                ("label", text(self.language, Label::Status)),
                (
                    "value",
                    text(self.language, project_status_label(project.status)),
                ),
            ],
        );
        self.out.push_str("<header>\n  <div>\n    <h1>");
        self.out.push_str(&escape(&project.name));
        self.out.push_str("</h1>\n");
        if let Some(description) = project
            .description
            .as_deref()
            .filter(|text| !text.is_empty())
        {
            self.out.push_str("    <div class=\"sub\">");
            self.out.push_str(&escape(description));
            self.out.push_str("</div>\n");
        }
        self.out.push_str("  </div>\n  <div class=\"meta\">");
        self.out.push_str(&meta);
        self.out.push_str("<br>\n");
        self.out.push_str(&status);
        if let Some(version) = &project.current_version {
            let line = fill(
                self.language,
                Label::FieldLine,
                &[
                    ("label", text(self.language, Label::Version)),
                    ("value", &escape(version)),
                ],
            );
            self.out.push_str("<br>\n");
            self.out.push_str(&line);
        }
        self.out.push_str("</div>\n</header>\n");
    }

    fn write_overall(&mut self, project: &Project) {
        let progress = project_progress(project);
        let count = fill(
            self.language,
            Label::OverallCount,
            &[
                ("done", &progress.done.to_string()),
                ("total", &progress.total.to_string()),
                ("tasks", text(self.language, Label::Tasks)),
                ("percent", &progress.percent.to_string()),
            ],
        );
        let _ = write!(
            self.out,
            "<div class=\"card overall\">\n  <div class=\"row\"><span>{}</span><b>{count}</b></div>\n  <div class=\"bar\"><i style=\"width:{}%\"></i></div>\n  <div class=\"legend\">\n",
            escape(text(self.language, Label::OverallProgress)),
            progress.percent
        );
        self.write_legend(Label::Done, "var(--done)");
        self.write_legend(Label::InProgress, "var(--run)");
        self.write_legend(Label::Open, "var(--open)");
        self.out.push_str("  </div>\n</div>\n");
    }

    fn write_legend(&mut self, label: Label, color: &str) {
        let _ = writeln!(
            self.out,
            "    <span><span class=\"dot\" style=\"background:{color}\"></span>{}</span>",
            escape(text(self.language, label))
        );
    }

    fn write_phases(&mut self, project: &Project) {
        let Some(plan) = &project.plan else {
            return;
        };
        self.out.push_str("<div class=\"grid\">\n");
        for (index, phase) in plan.phases.iter().enumerate() {
            self.write_phase(phase, index + 1);
        }
        self.out.push_str("</div>\n");
    }

    fn write_phase(&mut self, phase: &Phase, number: usize) {
        let state = phase_state(&phase.tasks);
        let progress = task_progress(&phase.tasks);
        let (class, label) = phase_presentation(state);
        let count = fill(
            self.language,
            Label::PhaseCount,
            &[
                ("done", &progress.done.to_string()),
                ("total", &progress.total.to_string()),
            ],
        );
        let percent = fill(
            self.language,
            Label::Percent,
            &[("percent", &progress.percent.to_string())],
        );
        let _ = write!(
            self.out,
            "<section class=\"card phase\">\n  <div class=\"top\"><h2><span class=\"num\">{number}</span>{}</h2><span class=\"tag {class}\">{}</span></div>\n",
            escape(&phase.title),
            escape(text(self.language, label))
        );
        if let Some(goal) = phase.goal.as_deref().filter(|goal| !goal.is_empty()) {
            self.out.push_str("  <div class=\"goal\">");
            self.out.push_str(&escape(goal));
            self.out.push_str("</div>\n");
        }
        let _ = write!(
            self.out,
            "  <div class=\"pct\"><span>{count}</span><span>{percent}</span></div>\n  <div class=\"bar\"><i style=\"width:{}%\"></i></div>\n  <ul>\n",
            progress.percent
        );
        for task in &phase.tasks {
            self.write_task(task);
        }
        self.out.push_str("  </ul>\n</section>\n");
    }

    fn write_task(&mut self, task: &Task) {
        let (item_class, mark_class) = task_classes(task.status);
        let item = if item_class.is_empty() {
            String::new()
        } else {
            format!(" class=\"{item_class}\"")
        };
        let mark = if mark_class.is_empty() {
            String::new()
        } else {
            format!(" {mark_class}")
        };
        let _ = write!(
            self.out,
            "    <li{item}><span class=\"st{mark}\"></span><span class=\"t\">{}",
            escape(&task.title)
        );
        if let Some(due) = &task.due {
            self.out.push(' ');
            self.out.push_str(&escape(due));
        }
        self.out.push_str("</span></li>\n");
    }

    fn write_issues(&mut self, project: &Project) {
        let mut issues: Vec<&Issue> = project
            .issues
            .iter()
            .flatten()
            .filter(|issue| matches!(issue.status, IssueStatus::Open | IssueStatus::InProgress))
            .collect();
        if issues.is_empty() {
            return;
        }
        issues.sort_by_key(|issue| severity_rank(issue.severity));
        self.write_heading(Label::OpenIssues);
        self.out.push_str("<div class=\"card\">\n<table>\n");
        self.write_row_heading(&[
            Label::ColumnSeverity,
            Label::ColumnTitle,
            Label::ColumnStatus,
        ]);
        for issue in issues {
            self.out.push_str("<tr><td>");
            self.out
                .push_str(&escape(text(self.language, severity_label(issue.severity))));
            self.out.push_str("</td><td>");
            if let Some(url) = &issue.url {
                self.write_anchor(url, &issue.title);
            } else {
                self.out.push_str(&escape(&issue.title));
            }
            self.out.push_str("</td><td>");
            self.out.push_str(&escape(text(
                self.language,
                issue_status_label(issue.status),
            )));
            self.out.push_str("</td></tr>\n");
        }
        self.out.push_str("</table>\n</div>\n");
    }

    fn write_watch_items(&mut self, project: &Project) {
        let mut items: Vec<&WatchItem> = project.watch_items.iter().flatten().collect();
        if items.is_empty() {
            return;
        }
        items.sort_by(|left, right| left.due.cmp(&right.due));
        self.write_heading(Label::WatchItems);
        self.out.push_str("<div class=\"card\">\n<table>\n");
        self.write_row_heading(&[
            Label::ColumnKind,
            Label::ColumnTitle,
            Label::ColumnDue,
            Label::ColumnNote,
        ]);
        for item in items {
            self.out.push_str("<tr><td>");
            self.out
                .push_str(&escape(text(self.language, watch_kind_label(item.kind))));
            self.out.push_str("</td><td>");
            self.out.push_str(&escape(&item.title));
            self.out.push_str("</td><td>");
            self.out.push_str(&escape(&item.due));
            self.out.push_str("</td><td>");
            if let Some(note) = &item.note {
                self.out.push_str(&escape(note));
            }
            self.out.push_str("</td></tr>\n");
        }
        self.out.push_str("</table>\n</div>\n");
    }

    fn write_stack(&mut self, project: &Project) {
        let Some(stack) = &project.stack else {
            return;
        };
        if stack.components.is_empty() {
            return;
        }
        self.write_heading(Label::Stack);
        self.out.push_str("<div class=\"card\">\n<table>\n");
        self.write_row_heading(&[
            Label::ColumnKind,
            Label::ColumnName,
            Label::ColumnVersion,
            Label::ColumnNote,
        ]);
        for component in &stack.components {
            self.out.push_str("<tr><td>");
            self.out.push_str(&escape(text(
                self.language,
                component_kind_label(component.kind),
            )));
            self.out.push_str("</td><td>");
            self.out.push_str(&escape(&component.name));
            self.out.push_str("</td><td>");
            if let Some(version) = &component.version {
                self.out.push_str(&escape(version));
            }
            self.out.push_str("</td><td>");
            if let Some(note) = &component.note {
                self.out.push_str(&escape(note));
            }
            self.out.push_str("</td></tr>\n");
        }
        self.out.push_str("</table>\n</div>\n");
    }

    fn write_links(&mut self, project: &Project) {
        let Some(links) = &project.links else {
            return;
        };
        if links.is_empty() {
            return;
        }
        self.write_heading(Label::Links);
        self.out.push_str("<div class=\"card\">\n<table>\n");
        self.write_row_heading(&[Label::ColumnLabel, Label::ColumnUrl]);
        for link in links {
            self.out.push_str("<tr><td>");
            self.out.push_str(&escape(&link.label));
            self.out.push_str("</td><td>");
            self.write_anchor(&link.url, &link.url);
            self.out.push_str("</td></tr>\n");
        }
        self.out.push_str("</table>\n</div>\n");
    }

    fn write_heading(&mut self, label: Label) {
        self.out.push_str("<h3>");
        self.out.push_str(&escape(text(self.language, label)));
        self.out.push_str("</h3>\n");
    }

    fn write_row_heading(&mut self, labels: &[Label]) {
        self.out.push_str("<tr>");
        for label in labels {
            self.out.push_str("<th>");
            self.out.push_str(&escape(text(self.language, *label)));
            self.out.push_str("</th>");
        }
        self.out.push_str("</tr>\n");
    }

    fn write_anchor(&mut self, url: &str, label: &str) {
        let _ = write!(
            self.out,
            "<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>",
            escape(url),
            escape(label)
        );
    }
}

fn phase_presentation(state: PhaseState) -> (&'static str, Label) {
    match state {
        PhaseState::Open => ("open", Label::Open),
        PhaseState::InProgress => ("run", Label::InProgress),
        PhaseState::Done => ("done", Label::Done),
    }
}

fn task_classes(status: TaskStatus) -> (&'static str, &'static str) {
    match status {
        TaskStatus::Open => ("", ""),
        TaskStatus::InProgress => ("", "run"),
        TaskStatus::Done => ("done", "done"),
        TaskStatus::Dropped => ("dropped", "dropped"),
    }
}

fn severity_rank(severity: IssueSeverity) -> u8 {
    match severity {
        IssueSeverity::Critical => 0,
        IssueSeverity::High => 1,
        IssueSeverity::Medium => 2,
        IssueSeverity::Low => 3,
    }
}

fn project_status_label(status: ProjectStatus) -> Label {
    match status {
        ProjectStatus::Idea => Label::ProjectIdea,
        ProjectStatus::InDevelopment => Label::ProjectInDevelopment,
        ProjectStatus::Released => Label::ProjectReleased,
        ProjectStatus::Maintenance => Label::ProjectMaintenance,
        ProjectStatus::Paused => Label::ProjectPaused,
        ProjectStatus::Discontinued => Label::ProjectDiscontinued,
    }
}

fn severity_label(severity: IssueSeverity) -> Label {
    match severity {
        IssueSeverity::Low => Label::SeverityLow,
        IssueSeverity::Medium => Label::SeverityMedium,
        IssueSeverity::High => Label::SeverityHigh,
        IssueSeverity::Critical => Label::SeverityCritical,
    }
}

fn issue_status_label(status: IssueStatus) -> Label {
    match status {
        IssueStatus::InProgress => Label::IssueInProgress,
        IssueStatus::Open | IssueStatus::Resolved | IssueStatus::WontFix => Label::IssueOpen,
    }
}

fn watch_kind_label(kind: WatchKind) -> Label {
    match kind {
        WatchKind::Deadline => Label::WatchDeadline,
        WatchKind::Expiry => Label::WatchExpiry,
        WatchKind::Review => Label::WatchReview,
        WatchKind::Other => Label::WatchOther,
    }
}

fn component_kind_label(kind: ComponentKind) -> Label {
    match kind {
        ComponentKind::Platform => Label::KindPlatform,
        ComponentKind::Language => Label::KindLanguage,
        ComponentKind::Framework => Label::KindFramework,
        ComponentKind::Runtime => Label::KindRuntime,
        ComponentKind::Sdk => Label::KindSdk,
        ComponentKind::Service => Label::KindService,
        ComponentKind::Tool => Label::KindTool,
        ComponentKind::Other => Label::KindOther,
    }
}

/// Escapes `&`, `<`, `>`, `"`, and `'` for HTML text and attributes.
#[must_use]
pub fn escape_html(text: &str) -> String {
    escape(text)
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{self, ErrorKind, Write as _};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        PublishChoice, RoadmapError, hard_link_unsupported, publish_choice, write_created,
        write_exclusive, write_new_file_with,
    };

    fn scratch(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!("craftward-publish-{name}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn names(directory: &std::path::Path) -> Vec<String> {
        fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn only_documented_unsupported_codes_select_the_fallback() {
        for code in super::unsupported_hard_link_codes() {
            let error = io::Error::from_raw_os_error(*code);
            assert!(
                hard_link_unsupported(&error),
                "code {code} should select the fallback"
            );
            assert_eq!(publish_choice(&error), PublishChoice::CreateNew);
        }
        // Access denied, cross-device, and an already existing name are not
        // "this file system cannot make hard links".
        for code in access_denied_and_other_codes() {
            let error = io::Error::from_raw_os_error(*code);
            assert!(
                !hard_link_unsupported(&error),
                "code {code} must stay an error"
            );
            assert_ne!(publish_choice(&error), PublishChoice::CreateNew);
        }
        let without_a_code = io::Error::new(ErrorKind::Unsupported, "no raw code");
        assert!(!hard_link_unsupported(&without_a_code));
        assert_eq!(publish_choice(&without_a_code), PublishChoice::Fail);
        let exists = io::Error::new(ErrorKind::AlreadyExists, "exists");
        assert_eq!(publish_choice(&exists), PublishChoice::AlreadyExists);
    }

    fn access_denied_and_other_codes() -> &'static [i32] {
        #[cfg(windows)]
        {
            // ERROR_ACCESS_DENIED, ERROR_NOT_SAME_DEVICE, ERROR_FILE_EXISTS
            &[5, 17, 80]
        }
        #[cfg(unix)]
        {
            // EACCES, EXDEV, EEXIST. On macOS EPERM is a directory, not support.
            #[cfg(target_os = "macos")]
            {
                &[13, 18, 17, 1]
            }
            #[cfg(not(target_os = "macos"))]
            {
                &[13, 18, 17]
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            &[]
        }
    }

    #[test]
    fn fallback_writes_a_new_file_and_leaves_an_existing_one() {
        let root = scratch("fallback");
        let path = root.join("harbor-light_roadmap_v1.html");
        let code = super::unsupported_hard_link_codes()
            .first()
            .copied()
            .expect("this platform has an unsupported-link code");
        let unsupported = io::Error::from_raw_os_error(code);
        write_new_file_with(
            &path,
            b"hello",
            move |_, _| Err(unsupported),
            write_exclusive,
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello");
        assert!(
            names(&root).iter().all(|name| !name.contains(".tmp")),
            "{:?}",
            names(&root)
        );

        let denied_code = access_denied_and_other_codes()
            .first()
            .copied()
            .expect("this platform has a non-fallback code");
        let denied = io::Error::from_raw_os_error(denied_code);
        let second = root.join("harbor-light_roadmap_v2.html");
        let error = write_new_file_with(&second, b"nope", move |_, _| Err(denied), write_exclusive)
            .unwrap_err();
        assert!(matches!(error, RoadmapError::Write(_)), "{error}");
        assert!(!second.exists());

        fs::write(&path, b"keep").unwrap();
        let result = write_new_file_with(
            &path,
            b"replaced",
            |_, _| panic!("an existing file is not linked"),
            write_exclusive,
        );
        assert!(matches!(result, Err(RoadmapError::AlreadyExists(_))));
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exclusive_create_removes_a_partial_file_and_does_not_truncate() {
        let root = scratch("partial");
        let path = root.join("page.html");
        let error = write_created(&path, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::new(ErrorKind::StorageFull, "full"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::StorageFull);
        assert!(!path.exists(), "the incomplete file must be removed");

        fs::write(&path, b"keep").unwrap();
        let error = write_exclusive(&path, b"new").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        let _ = fs::remove_dir_all(root);
    }
}
