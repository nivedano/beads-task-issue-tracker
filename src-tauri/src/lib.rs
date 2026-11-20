use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, WebviewWindowBuilder};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

mod grace;

// Global flags for logging
static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);
static VERBOSE_LOGGING: AtomicBool = AtomicBool::new(false);

// Sync cooldown: skip redundant syncs within 10 seconds
static LAST_SYNC_TIME: Mutex<Option<Instant>> = Mutex::new(None);
const SYNC_COOLDOWN_SECS: u64 = 10;

// Filesystem mtime tracking for change detection (per-project)
static LAST_KNOWN_MTIME: LazyLock<Mutex<HashMap<String, std::time::SystemTime>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// Configurable CLI binary name (default: "br")
static CLI_BINARY: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("br".to_string()));

// Global child process handle for beads-probe
static PROBE_CHILD: LazyLock<Mutex<Option<std::process::Child>>> =
    LazyLock::new(|| Mutex::new(None));

// Per-project mutex to serialize CLI invocations.
// The CLI store is a single-writer SQLite database; concurrent invocations against
// the same project can fail or corrupt it. This serializes all CLI calls per project.
static PROJECT_LOCKS: LazyLock<Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// Conditional logging macros
macro_rules! log_info {
    ($($arg:tt)*) => {
        if LOGGING_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            log::info!($($arg)*);
        }
    };
}

macro_rules! log_warn {
    ($($arg:tt)*) => {
        if LOGGING_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            log::warn!($($arg)*);
        }
    };
}

macro_rules! log_error {
    ($($arg:tt)*) => {
        if LOGGING_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            log::error!($($arg)*);
        }
    };
}

macro_rules! log_debug {
    ($($arg:tt)*) => {
        if LOGGING_ENABLED.load(std::sync::atomic::Ordering::Relaxed) && VERBOSE_LOGGING.load(std::sync::atomic::Ordering::Relaxed) {
            log::debug!($($arg)*);
        }
    };
}

// ============================================================================
// Update Checker Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    #[serde(rename = "currentVersion")]
    pub current_version: String,
    #[serde(rename = "latestVersion")]
    pub latest_version: String,
    #[serde(rename = "hasUpdate")]
    pub has_update: bool,
    #[serde(rename = "releaseUrl")]
    pub release_url: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    pub platform: String,
    #[serde(rename = "releaseNotes")]
    pub release_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrCliUpdateInfo {
    #[serde(rename = "currentVersion")]
    pub current_version: String,
    #[serde(rename = "latestVersion")]
    pub latest_version: String,
    #[serde(rename = "hasUpdate")]
    pub has_update: bool,
    #[serde(rename = "releaseUrl")]
    pub release_url: String,
}

// ============================================================================
// File Watcher (debounced native fs watcher via notify crate)
// ============================================================================

struct WatcherState {
    debouncer: Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>,
    watched_path: Option<String>,
    watch_session_id: u64,
    project_emit_state: HashMap<String, Arc<Mutex<ProjectWatcherEmitState>>>,
}

impl Default for WatcherState {
    fn default() -> Self {
        Self {
            debouncer: None,
            watched_path: None,
            watch_session_id: 0,
            project_emit_state: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct ProjectWatcherEmitState {
    session_id: u64,
    last_emit: Option<Instant>,
    pending: bool,
    flush_scheduled: bool,
    emitted_batches: u64,
    suppressed_batches: u64,
}

impl ProjectWatcherEmitState {
    fn new(session_id: u64) -> Self {
        Self {
            session_id,
            last_emit: None,
            pending: false,
            flush_scheduled: false,
            emitted_batches: 0,
            suppressed_batches: 0,
        }
    }
}

const WATCHER_DEBOUNCE_INTERVAL_MS: u64 = 1000;
const WATCHER_MIN_EMIT_INTERVAL_MS_DEFAULT: u64 = 2000;

static WATCHER_MIN_EMIT_INTERVAL_MS: LazyLock<u64> = LazyLock::new(|| {
    env::var("WATCHER_MIN_EMIT_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(WATCHER_MIN_EMIT_INTERVAL_MS_DEFAULT)
        .max(250)
});

fn watcher_min_emit_interval() -> Duration {
    Duration::from_millis(*WATCHER_MIN_EMIT_INTERVAL_MS)
}

fn is_watched_path(path: &std::path::Path) -> bool {
    let path = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if !(path.contains("/.beads/") || path.contains("/.grace5/")) {
        return false;
    }

    // Ignore high-churn internal files that do not carry registry data.
    !(path.ends_with(".lock")
        || path.ends_with(".tmp")
        || path.ends_with(".swp")
        || path.ends_with('~'))
}

fn should_process_watcher_event(event: &notify_debouncer_mini::DebouncedEvent) -> bool {
    if !matches!(event.kind, DebouncedEventKind::Any | DebouncedEventKind::AnyContinuous) {
        return false;
    }

    is_watched_path(&event.path)
}

fn emit_beads_changed(app_handle: &tauri::AppHandle, project_path: &str) {
    if let Err(err) = app_handle.emit(
        "beads-changed",
        BeadsChangedPayload {
            path: project_path.to_string(),
        },
    ) {
        log::error!("[watcher] Failed to emit beads-changed for {}: {:?}", project_path, err);
    }
}

fn schedule_pending_watcher_emit(
    app_handle: tauri::AppHandle,
    project_path: String,
    emit_state: Arc<Mutex<ProjectWatcherEmitState>>,
    session_id: u64,
) {
    std::thread::spawn(move || {
        let min_interval = watcher_min_emit_interval();
        loop {
            let mut should_emit = false;
            let mut counters = None;
            let mut wait_time = Duration::from_millis(50);

            match emit_state.lock() {
                Ok(mut state) => {
                    if state.session_id != session_id || !state.pending {
                        state.flush_scheduled = false;
                        return;
                    }

                    let can_emit_now = state
                        .last_emit
                        .map(|last| last.elapsed() >= min_interval)
                        .unwrap_or(true);

                    if can_emit_now {
                        state.pending = false;
                        state.flush_scheduled = false;
                        state.last_emit = Some(Instant::now());
                        state.emitted_batches += 1;
                        counters = Some((state.emitted_batches, state.suppressed_batches));
                        should_emit = true;
                    } else if let Some(last_emit) = state.last_emit {
                        wait_time = min_interval.saturating_sub(last_emit.elapsed());
                    }
                }
                Err(err) => {
                    log::error!("[watcher] Failed to lock emit state for delayed flush: {}", err);
                    return;
                }
            }

            if should_emit {
                emit_beads_changed(&app_handle, &project_path);
                if let Some((emitted, suppressed)) = counters {
                    log::info!(
                        "[watcher] Delayed coalesced emit for {} (emitted={}, suppressed={})",
                        project_path,
                        emitted,
                        suppressed
                    );
                }
                return;
            }

            std::thread::sleep(wait_time);
        }
    });
}

#[derive(Debug, Clone, Serialize)]
struct BeadsChangedPayload {
    path: String,
}

// ============================================================================
// Types
// ============================================================================

/// Dependency relationship as returned by br CLI
/// Format: {"issue_id": "...", "depends_on_id": "...", "type": "blocks", "created_at": "...", "created_by": "..."}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrRawDependency {
    pub id: Option<String>,
    pub issue_id: Option<String>,
    pub depends_on_id: Option<String>,
    #[serde(rename = "type", alias = "dependency_type")]
    pub dependency_type: Option<String>,
    pub created_at: Option<String>,
    pub created_by: Option<String>,
}

/// Dependent info (for parent-child relationships with full issue info)
/// Some CLI versions may return this format instead
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrRawDependent {
    pub id: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub issue_type: Option<String>,
    pub dependency_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrRawIssue {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: i32,
    pub issue_type: String,
    pub owner: Option<String>,
    pub assignee: Option<String>,
    pub labels: Option<Vec<String>>,
    pub created_at: String,
    pub created_by: Option<String>,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub close_reason: Option<String>,
    pub blocked_by: Option<Vec<String>>,
    pub blocks: Option<Vec<String>>,
    pub comments: Option<Vec<BrRawComment>>,
    pub external_ref: Option<String>,
    pub estimate: Option<i32>,
    pub design: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub notes: Option<String>,
    pub parent: Option<String>,
    pub dependents: Option<Vec<BrRawDependent>>,
    pub dependencies: Option<Vec<BrRawDependency>>,
    pub dependency_count: Option<i32>,
    pub dependent_count: Option<i32>,
    pub metadata: Option<String>,
    pub spec_id: Option<String>,
    pub comment_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrRawComment {
    pub id: serde_json::Value,
    pub issue_id: Option<String>,
    pub author: String,
    pub text: Option<String>,
    pub content: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub issue_type: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "closedAt")]
    pub closed_at: Option<String>,
    pub comments: Vec<Comment>,
    #[serde(rename = "blockedBy")]
    pub blocked_by: Option<Vec<String>>,
    pub blocks: Option<Vec<String>>,
    #[serde(rename = "externalRef")]
    pub external_ref: Option<String>,
    #[serde(rename = "estimateMinutes")]
    pub estimate_minutes: Option<i32>,
    #[serde(rename = "designNotes")]
    pub design_notes: Option<String>,
    #[serde(rename = "acceptanceCriteria")]
    pub acceptance_criteria: Option<String>,
    #[serde(rename = "workingNotes")]
    pub working_notes: Option<String>,
    pub parent: Option<ParentIssue>,
    pub children: Option<Vec<ChildIssue>>,
    pub relations: Option<Vec<Relation>>,
    pub metadata: Option<String>,
    #[serde(rename = "specId")]
    pub spec_id: Option<String>,
    #[serde(rename = "commentCount")]
    pub comment_count: Option<i32>,
    #[serde(rename = "dependencyCount")]
    pub dependency_count: Option<i32>,
    #[serde(rename = "dependentCount")]
    pub dependent_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChildIssue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParentIssue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Relation {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    pub direction: String, // "dependency" or "dependent"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CountResult {
    pub count: usize,
    #[serde(rename = "byType")]
    pub by_type: HashMap<String, usize>,
    #[serde(rename = "byPriority")]
    pub by_priority: HashMap<String, usize>,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDirectory")]
    pub is_directory: bool,
    #[serde(rename = "hasBeads")]
    pub has_beads: bool,
    #[serde(rename = "hasGrace")]
    pub has_grace: bool,
}

#[derive(Debug, Serialize)]
pub struct PurgeResult {
    #[serde(rename = "deletedCount")]
    pub deleted_count: usize,
    #[serde(rename = "deletedFolders")]
    pub deleted_folders: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FsListResult {
    #[serde(rename = "currentPath")]
    pub current_path: String,
    #[serde(rename = "hasBeads")]
    pub has_beads: bool,
    #[serde(rename = "hasGrace")]
    pub has_grace: bool,
    pub entries: Vec<DirectoryEntry>,
}

// ============================================================================
// Options structs for commands
// ============================================================================

#[derive(Debug, Deserialize, Default)]
pub struct ListOptions {
    pub status: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub issue_type: Option<Vec<String>>,
    pub priority: Option<Vec<String>>,
    pub assignee: Option<String>,
    #[serde(rename = "includeAll")]
    pub include_all: Option<bool>,
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CwdOptions {
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePayload {
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub issue_type: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub labels: Option<Vec<String>>,
    #[serde(rename = "externalRef")]
    pub external_ref: Option<String>,
    #[serde(rename = "estimateMinutes")]
    pub estimate_minutes: Option<i32>,
    #[serde(rename = "designNotes")]
    pub design_notes: Option<String>,
    #[serde(rename = "acceptanceCriteria")]
    pub acceptance_criteria: Option<String>,
    #[serde(rename = "workingNotes")]
    pub working_notes: Option<String>,
    pub parent: Option<String>, // Parent epic ID for hierarchical child
    #[serde(rename = "specId")]
    pub spec_id: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePayload {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub issue_type: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub labels: Option<Vec<String>>,
    #[serde(rename = "externalRef")]
    pub external_ref: Option<String>,
    #[serde(rename = "estimateMinutes")]
    pub estimate_minutes: Option<i32>,
    #[serde(rename = "designNotes")]
    pub design_notes: Option<String>,
    #[serde(rename = "acceptanceCriteria")]
    pub acceptance_criteria: Option<String>,
    #[serde(rename = "workingNotes")]
    pub working_notes: Option<String>,
    pub parent: Option<String>, // Some("") to detach, Some("id") to attach
    pub metadata: Option<String>,
    #[serde(rename = "specId")]
    pub spec_id: Option<String>,
    pub cwd: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

fn priority_to_string(priority: i32) -> String {
    let p = if (0..=4).contains(&priority) { priority } else { 3 };
    format!("p{}", p)
}

fn priority_to_number(priority: &str) -> String {
    if let Some(caps) = priority.strip_prefix('p') {
        if caps.len() == 1 && caps.chars().next().unwrap_or('x').is_ascii_digit() {
            return caps.to_string();
        }
    }
    "3".to_string()
}

fn normalize_issue_type(issue_type: &str) -> String {
    let valid_types = ["bug", "task", "feature", "epic", "chore"];
    if valid_types.contains(&issue_type) {
        issue_type.to_string()
    } else {
        "task".to_string()
    }
}

fn normalize_issue_status(status: &str) -> String {
    let valid_statuses = ["open", "in_progress", "blocked", "closed", "deferred", "pinned", "hooked"];
    if valid_statuses.contains(&status) {
        status.to_string()
    } else {
        "open".to_string()
    }
}

fn transform_issue(raw: BrRawIssue) -> Issue {
    // Parent info - dependencies array now contains relationship info, not full issue details
    // For now, we just use the parent ID if available
    let parent = raw.parent.as_ref().map(|parent_id| {
        ParentIssue {
            id: parent_id.clone(),
            title: String::new(), // Not available in dependency format
            status: "open".to_string(),
            priority: "p3".to_string(),
        }
    });

    // Extract children from dependents array (with dependency_type: "parent-child")
    let children: Option<Vec<ChildIssue>> = raw.dependents.as_ref().map(|deps| {
        deps.iter()
            .filter(|d| d.dependency_type.as_deref() == Some("parent-child") && d.id.is_some())
            .map(|c| ChildIssue {
                id: c.id.clone().unwrap_or_default(),
                title: c.title.clone().unwrap_or_default(),
                status: normalize_issue_status(&c.status.clone().unwrap_or_else(|| "open".to_string())),
                priority: priority_to_string(c.priority.unwrap_or(3)),
            })
            .collect()
    }).filter(|v: &Vec<ChildIssue>| !v.is_empty());

    // Extract non-blocking relations (everything except "blocks" and "parent-child")
    let structural_types = ["blocks", "parent-child"];
    let mut relations: Vec<Relation> = Vec::new();
    let mut seen_relations: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    // From dependencies array (these are issues the current issue depends on)
    if let Some(ref deps) = raw.dependencies {
        for dep in deps {
            if let Some(ref dep_type) = dep.dependency_type {
                if structural_types.contains(&dep_type.as_str()) {
                    continue;
                }
                let id = dep.id.clone().or_else(|| dep.depends_on_id.clone()).unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let key = (id.clone(), dep_type.clone());
                if !seen_relations.contains(&key) {
                    seen_relations.insert(key);
                    relations.push(Relation {
                        id,
                        title: String::new(),
                        status: String::new(),
                        priority: String::new(),
                        relation_type: dep_type.clone(),
                        direction: "dependency".to_string(),
                    });
                }
            }
        }
    }

    // From dependents array (these are issues that depend on the current issue — has full metadata)
    if let Some(ref dependents) = raw.dependents {
        for dep in dependents {
            if let Some(ref dep_type) = dep.dependency_type {
                if structural_types.contains(&dep_type.as_str()) {
                    continue;
                }
                let id = dep.id.clone().unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let key = (id.clone(), dep_type.clone());
                if seen_relations.contains(&key) {
                    // Replace existing entry from dependencies if this one has more metadata
                    if dep.title.is_some() {
                        if let Some(existing) = relations.iter_mut().find(|r| r.id == id && r.relation_type == *dep_type) {
                            existing.title = dep.title.clone().unwrap_or_default();
                            existing.status = normalize_issue_status(&dep.status.clone().unwrap_or_else(|| "open".to_string()));
                            existing.priority = priority_to_string(dep.priority.unwrap_or(3));
                            existing.direction = "dependent".to_string();
                        }
                    }
                } else {
                    seen_relations.insert(key);
                    relations.push(Relation {
                        id,
                        title: dep.title.clone().unwrap_or_default(),
                        status: normalize_issue_status(&dep.status.clone().unwrap_or_else(|| "open".to_string())),
                        priority: priority_to_string(dep.priority.unwrap_or(3)),
                        relation_type: dep_type.clone(),
                        direction: "dependent".to_string(),
                    });
                }
            }
        }
    }

    // Compute comment_count before consuming raw.comments
    let comment_count = raw.comment_count.or_else(|| {
        raw.comments.as_ref().map(|c| c.len() as i32)
    });

    Issue {
        id: raw.id,
        title: raw.title,
        description: raw.description.unwrap_or_default(),
        issue_type: normalize_issue_type(&raw.issue_type),
        status: normalize_issue_status(&raw.status),
        priority: priority_to_string(raw.priority),
        assignee: raw.assignee,
        labels: raw.labels.unwrap_or_default(),
        created_at: raw.created_at,
        updated_at: raw.updated_at,
        closed_at: raw.closed_at,
        comments: raw.comments.unwrap_or_default().into_iter().map(|c| {
            Comment {
                id: match c.id {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s,
                    _ => "0".to_string(),
                },
                author: c.author,
                content: c.text.or(c.content).unwrap_or_default(),
                created_at: c.created_at,
            }
        }).collect(),
        blocked_by: {
            // Try raw.blocked_by first (if the CLI ever populates it directly)
            let mut bb = raw.blocked_by.unwrap_or_default();
            // Extract from dependencies array (show: objects with dependency_type "blocks" = blockers)
            if let Some(ref deps) = raw.dependencies {
                // show format: [{id, dependency_type: "blocks"}] — these block the current issue
                for dep in deps {
                    if let (Some(ref dep_type), Some(ref id)) = (&dep.dependency_type, &dep.id) {
                        if dep_type == "blocks" && !bb.contains(id) {
                            bb.push(id.clone());
                        }
                    }
                    // list format: [{issue_id, depends_on_id, type: "blocks"}]
                    if let (Some(ref dep_type), Some(ref depends_on_id), Some(ref _issue_id)) = (&dep.dependency_type, &dep.depends_on_id, &dep.issue_id) {
                        if dep_type == "blocks" && !bb.contains(depends_on_id) {
                            bb.push(depends_on_id.clone());
                        }
                    }
                }
            }
            if bb.is_empty() { None } else { Some(bb) }
        },
        blocks: {
            let mut bl = raw.blocks.unwrap_or_default();
            // Extract from dependents array (show: objects with dependency_type "blocks" = issues blocked by current)
            // Filter to only "blocks" type — exclude "parent-child" which are children, not dependencies
            if let Some(ref dependents) = raw.dependents {
                for dep in dependents {
                    if let (Some(ref dep_type), Some(ref id)) = (&dep.dependency_type, &dep.id) {
                        if dep_type == "blocks" && !bl.contains(id) {
                            bl.push(id.clone());
                        }
                    }
                }
            }
            if bl.is_empty() { None } else { Some(bl) }
        },
        external_ref: raw.external_ref,
        estimate_minutes: raw.estimate,
        design_notes: raw.design,
        acceptance_criteria: raw.acceptance_criteria,
        working_notes: raw.notes,
        parent,
        children,
        relations: if relations.is_empty() { None } else { Some(relations) },
        metadata: raw.metadata,
        spec_id: raw.spec_id,
        comment_count,
        dependency_count: raw.dependency_count.or_else(|| {
            raw.dependencies.as_ref().map(|d| d.len() as i32)
        }),
        dependent_count: raw.dependent_count.or_else(|| {
            raw.dependents.as_ref().map(|d| d.len() as i32)
        }),
    }
}

/// Parse issues with tolerance for malformed entries
/// Returns all successfully parsed issues and logs failures
fn parse_issues_tolerant(output: &str, context: &str) -> Result<Vec<BrRawIssue>, String> {
    // First try strict parsing
    if let Ok(issues) = serde_json::from_str::<Vec<BrRawIssue>>(output) {
        return Ok(issues);
    }

    // If strict parsing fails, try tolerant parsing
    log_warn!("[{}] Strict parsing failed, attempting tolerant parsing", context);

    let value: serde_json::Value = serde_json::from_str(output)
        .map_err(|e| {
            log_error!("[{}] JSON is completely invalid: {}", context, e);
            format!("Invalid JSON: {}", e)
        })?;

    // br >= 0.1.30 wraps `list` output in a paginated envelope:
    // {"issues": [...], "total": N, "offset": N, "limit": N, "has_more": bool}
    // Unwrap the envelope if present, otherwise expect a flat array.
    let arr_value;
    let arr = if let Some(obj) = value.as_object() {
        if let Some(issues) = obj.get("issues").and_then(|v| v.as_array()) {
            log_info!("[{}] Unwrapped paginated envelope ({} issues)", context, issues.len());
            arr_value = issues.clone();
            &arr_value
        } else {
            log_error!("[{}] Expected array or envelope with 'issues' key, got object: {:?}", context, obj.keys().collect::<Vec<_>>());
            return Err("Expected JSON array or paginated envelope".to_string());
        }
    } else {
        value.as_array().ok_or_else(|| {
            log_error!("[{}] Expected array, got: {:?}", context, value);
            "Expected JSON array".to_string()
        })?
    };

    let mut issues = Vec::new();
    let mut failed_count = 0;

    for (i, obj) in arr.iter().enumerate() {
        let obj_str = serde_json::to_string(obj).unwrap_or_default();
        match serde_json::from_str::<BrRawIssue>(&obj_str) {
            Ok(issue) => issues.push(issue),
            Err(e) => {
                failed_count += 1;
                let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                log_error!("[{}] Skipping issue {} (id={}): {}", context, i, id, e);

                // Log which fields are present/missing
                if let Some(obj_map) = obj.as_object() {
                    let keys: Vec<&str> = obj_map.keys().map(|s| s.as_str()).collect();
                    log_error!("[{}] Issue {} has keys: {:?}", context, i, keys);

                    // Check for common missing required fields
                    let required = ["id", "title", "status", "priority", "issue_type", "created_at", "updated_at"];
                    let missing: Vec<&&str> = required.iter().filter(|k| !keys.contains(*k)).collect();
                    if !missing.is_empty() {
                        log_error!("[{}] Issue {} missing required fields: {:?}", context, i, missing);
                    }
                }
            }
        }
    }

    if failed_count > 0 {
        log_warn!("[{}] Parsed {} issues, skipped {} malformed entries", context, issues.len(), failed_count);
    }

    Ok(issues)
}

fn get_extended_path() -> String {
    let current_path = env::var("PATH").unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        let userprofile = env::var("USERPROFILE").unwrap_or_default();
        let localappdata = env::var("LOCALAPPDATA").unwrap_or_default();
        let mut extra_paths = vec![
            format!(r"{}\AppData\Local\bin", userprofile),
            format!(r"{}\.local\bin", userprofile),
            format!(r"{}\Programs", localappdata),
        ];
        extra_paths.extend(current_path.split(';').map(String::from));
        extra_paths.join(";")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = env::var("HOME").unwrap_or_default();
        let mut extra_paths = vec![
            "/opt/homebrew/bin".to_string(),
            "/usr/local/bin".to_string(),
            "/usr/bin".to_string(),
            "/bin".to_string(),
            format!("{}/.local/bin", home),
            format!("{}/bin", home),
        ];
        extra_paths.extend(current_path.split(':').map(String::from));
        extra_paths.join(":")
    }
}

/// Creates a Command with platform-specific flags.
/// On Windows, sets CREATE_NO_WINDOW to prevent console popups.
fn new_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

// ============================================================================
// CLI Binary Configuration
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    #[serde(default = "default_cli_binary")]
    cli_binary: String,
}

fn default_cli_binary() -> String {
    // br is the only supported client. Resolving it here happens from Tauri's
    // setup hook, so never spawn `--version` to validate it: that would block
    // the window from becoming responsive.
    "br".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cli_binary: default_cli_binary(),
        }
    }
}

fn get_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.beads.manager")
        .join("settings.json")
}

fn load_config() -> AppConfig {
    let path = get_config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(config) => return config,
                Err(e) => log::warn!("[config] Failed to parse settings.json: {}", e),
            },
            Err(e) => log::warn!("[config] Failed to read settings.json: {}", e),
        }
    }
    AppConfig::default()
}

fn get_cli_binary() -> String {
    CLI_BINARY.lock().unwrap().clone()
}

/// Normalize the configured CLI binary. `br` is the only supported client, so a
/// stale bare `"bd"` value from an older settings.json coerces to `br` (custom
/// binary paths are respected as-is).
fn normalize_cli_binary(configured: &str) -> String {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return "br".to_string();
    }
    if trimmed == "bd" {
        log::warn!("[config] cli_binary \"bd\" is no longer supported; using \"br\"");
        return "br".to_string();
    }
    trimmed.to_string()
}

// ============================================================================
// CLI Version Parsing
// ============================================================================

/// Parse a version string into (major, minor, patch).
/// Works for `br 0.1.13 (rustc 1.85.0-nightly)`.
fn parse_version(version_str: &str) -> Option<(u32, u32, u32)> {
    // Look for a semver-like pattern: digits.digits.digits
    let re_like = version_str
        .split_whitespace()
        .find(|word| word.contains('.') && word.chars().next().map_or(false, |c| c.is_ascii_digit()));

    let version_part = re_like?;
    let parts: Vec<&str> = version_part.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        // Patch may have trailing non-numeric chars (e.g. "6-beta")
        let patch_str: String = parts[2].chars().take_while(|c| c.is_ascii_digit()).collect();
        let patch = patch_str.parse::<u32>().ok()?;
        Some((major, minor, patch))
    } else {
        None
    }
}

fn execute_br(command: &str, args: &[String], cwd: Option<&str>) -> Result<String, String> {
    let working_dir = cwd
        .map(String::from)
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    // Split command by spaces to handle subcommands like "comments add"
    let mut full_args: Vec<&str> = command.split_whitespace().collect();
    for arg in args {
        full_args.push(arg);
    }
    full_args.push("--json");

    let binary = get_cli_binary();
    log_info!("[br] {} {} | cwd: {}", binary, full_args.join(" "), working_dir);

    // Acquire the per-project lock: the CLI store is single-writer.
    let project_lock = {
        let mut locks = PROJECT_LOCKS.lock().unwrap();
        locks.entry(working_dir.clone())
            .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = project_lock.lock().unwrap();

    let output = new_command(&binary)
        .args(&full_args)
        .current_dir(&working_dir)
        .env("PATH", get_extended_path())
        .env("BEADS_PATH", &working_dir)
        .output()
        .map_err(|e| {
            log_error!("[br] Failed to execute {}: {}", binary, e);
            format!("Failed to execute {}: {}", binary, e)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log_error!("[br] Command failed | status: {} | stderr: {}", output.status, stderr);

        // Detect schema incompatibility so the frontend can offer a repair.
        if stderr.contains("no such column: spec_id") {
            log_error!("[br] Schema migration failure detected - database needs repair");
            return Err("SCHEMA_MIGRATION_ERROR: Database schema is incompatible. Please use the repair function to fix this issue.".to_string());
        }

        if !stderr.is_empty() {
            return Err(stderr.to_string());
        }
        return Err(format!("CLI command failed with status: {}", output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    log_info!("[br] OK | {} bytes", stdout.len());

    // Log output preview only if verbose mode is enabled
    if VERBOSE_LOGGING.load(Ordering::Relaxed) {
        let preview: String = stdout.chars().take(500).collect();
        log_debug!("[br] Output: {}", preview);
    }

    Ok(stdout)
}

/// Auto-run refs migration v3 (filesystem-only attachments) if needed.
/// Called synchronously before br sync to prevent UNIQUE constraint errors.
fn ensure_refs_migrated_v3(beads_dir: &std::path::Path, working_dir: &str) {
    if beads_dir.join(".migrated-attachments").exists() {
        return;
    }
    let jsonl_path = beads_dir.join("issues.jsonl");
    if !jsonl_path.exists() {
        let _ = std::fs::write(beads_dir.join(".migrated-attachments"), "");
        return;
    }

    let content = match std::fs::read_to_string(&jsonl_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Quick scan: does any line have non-real external refs?
    let mut needs_migration = false;
    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ext_ref = v.get("external_ref").and_then(|r| r.as_str()).unwrap_or("");
        // Non-real ref (att:, paths, cleared: sentinels, etc.)
        if ext_ref.is_empty() { continue; }
        for r in ext_ref.split(|c: char| c == '\n' || c == '|') {
            let trimmed = r.trim();
            if !trimmed.is_empty() && !is_real_external_ref(trimmed) {
                needs_migration = true;
                break;
            }
        }
        if needs_migration { break; }
    }

    // Also check if attachment folders need renaming
    let attachments_dir_check = beads_dir.join("attachments");
    let mut needs_folder_work = false;
    if attachments_dir_check.exists() {
        if let Ok(entries) = std::fs::read_dir(&attachments_dir_check) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() { continue; }
                let name = entry.file_name().to_string_lossy().to_string();
                if issue_short_id(&name) != name {
                    needs_folder_work = true;
                    break;
                }
            }
        }
    }

    if !needs_migration && !needs_folder_work {
        let _ = std::fs::write(beads_dir.join(".migrated-attachments"), "");
        return;
    }

    log_info!("[sync] Auto-migrating v3 (refs={}, folders={}) for: {}", needs_migration, needs_folder_work, working_dir);

    // Backup
    let backup_path = beads_dir.join("issues.jsonl.bak-refs-v3-migration");
    if std::fs::copy(&jsonl_path, &backup_path).is_err() {
        log_error!("[sync] Failed to backup JSONL for v3 migration, skipping");
        return;
    }

    // Migrate: strip non-real refs, deduplicate
    let mut refs_updated: u32 = 0;
    let mut output_lines: Vec<String> = Vec::new();
    let mut seen_refs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            output_lines.push(line.to_string());
            continue;
        }
        let mut v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => { output_lines.push(line.to_string()); continue; }
        };

        let issue_id = v.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();

        let ext_ref = v.get("external_ref").and_then(|r| r.as_str()).unwrap_or("").to_string();

        // Parse existing refs, keep only real external ones
        let real_refs: Vec<String> = if ext_ref.is_empty() {
            vec![]
        } else {
            ext_ref.split(|c: char| c == '\n' || c == '|')
                .map(|r| r.trim())
                .filter(|r| is_real_external_ref(r))
                .map(String::from)
                .collect()
        };

        let mut new_ref = if real_refs.is_empty() {
            String::new()
        } else {
            real_refs.join("|")
        };

        // Deduplicate: if another issue already has this exact ref, clear it
        if !new_ref.is_empty() && seen_refs.contains(&new_ref) {
            log_info!("[sync] Duplicate external_ref '{}' for issue {}, clearing", new_ref, issue_id);
            new_ref = String::new();
        }
        if !new_ref.is_empty() {
            seen_refs.insert(new_ref.clone());
        }

        if new_ref != ext_ref {
            v["external_ref"] = serde_json::Value::String(new_ref);
            refs_updated += 1;
            output_lines.push(serde_json::to_string(&v).unwrap_or_else(|_| line.to_string()));
            continue;
        }

        // Track existing refs that weren't modified too
        if let Some(ext_ref) = v.get("external_ref").and_then(|r| r.as_str()) {
            seen_refs.insert(ext_ref.to_string());
        }

        output_lines.push(line.to_string());
    }

    if refs_updated > 0 {
        let new_content = output_lines.join("\n");
        if std::fs::write(&jsonl_path, &new_content).is_err() {
            log_error!("[sync] Failed to write migrated JSONL");
            return;
        }
        log_info!("[sync] Refs v3 migration: {} ref(s) cleaned", refs_updated);
    }

    // Rename attachment folders: {full-id}/ → {short-id}/
    let attachments_dir = beads_dir.join("attachments");
    if attachments_dir.exists() {
        let mut renamed = 0u32;
        if let Ok(entries) = std::fs::read_dir(&attachments_dir) {
            let dirs: Vec<_> = entries.flatten()
                .filter(|e| e.path().is_dir())
                .collect();
            for entry in dirs {
                let folder_name = entry.file_name().to_string_lossy().to_string();
                let short = issue_short_id(&folder_name);
                if short != folder_name {
                    let target = attachments_dir.join(short);
                    if target.exists() {
                        log_warn!("[sync] Cannot rename '{}' → '{}': target already exists", folder_name, short);
                        continue;
                    }
                    if std::fs::rename(entry.path(), &target).is_ok() {
                        renamed += 1;
                    } else {
                        log_warn!("[sync] Failed to rename '{}' → '{}'", folder_name, short);
                    }
                }
            }
        }
        if renamed > 0 {
            log_info!("[sync] Renamed {} attachment folder(s) to short IDs", renamed);
        }
    }

    let _ = std::fs::write(beads_dir.join(".migrated-attachments"), "");
    // Signal for the frontend to show a notification
    let _ = std::fs::write(beads_dir.join(".migrated-attachments-notify"), "");
    log_info!("[sync] Migration v3 complete (refs cleaned + folders renamed)");
}

/// Sync the beads database before read operations to ensure data is up-to-date
/// Uses bidirectional sync to preserve local changes while getting remote updates
/// Has a cooldown to avoid redundant syncs within the same poll cycle
fn sync_br_database(cwd: Option<&str>) {
    let working_dir = cwd
        .map(String::from)
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    // Check cooldown — skip if synced recently
    let beads_dir = std::path::Path::new(&working_dir).join(".beads");
    {
        let last = LAST_SYNC_TIME.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed().as_secs() < SYNC_COOLDOWN_SECS {
                log_info!("[sync] Skipping — cooldown active ({:.1}s ago)", t.elapsed().as_secs_f32());
                return;
            }
        }
    }

    log_info!("[sync] Starting bidirectional sync for: {}", working_dir);

    // Auto-migrate refs v3 before sync if needed (prevents UNIQUE constraint errors)
    ensure_refs_migrated_v3(&beads_dir, &working_dir);

    // Run br sync (bidirectional - exports local changes AND imports remote changes)
    let binary = get_cli_binary();
    match new_command(&binary)
        .arg("sync")
        .current_dir(&working_dir)
        .env("PATH", get_extended_path())
        .env("BEADS_PATH", &working_dir)
        .output()
    {
        Ok(output) if output.status.success() => {
            log_info!("[sync] Sync completed successfully");
            // Update cooldown timestamp
            let mut last = LAST_SYNC_TIME.lock().unwrap();
            *last = Some(Instant::now());
        }
        Ok(output) => {
            log_warn!(
                "[sync] {} sync failed: {}",
                binary,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            log_error!("[sync] Failed to run {} sync: {}", binary, e);
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
async fn br_sync(cwd: Option<String>) -> Result<(), String> {
    let working_dir = cwd
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    let binary = get_cli_binary();
    log_info!("[br_sync] Manual sync requested for: {}", working_dir);

    let output = new_command(&binary)
        .arg("sync")
        .current_dir(&working_dir)
        .env("PATH", get_extended_path())
        .env("BEADS_PATH", &working_dir)
        .output()
        .map_err(|e| format!("Failed to run {} sync: {}", binary, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log_error!("[br_sync] Sync failed: {}", stderr.trim());
        return Err(format!("Sync failed: {}", stderr.trim()));
    }

    log_info!("[br_sync] Sync completed successfully");
    // Reset cooldown so subsequent reads pick up the fresh sync
    let mut last = LAST_SYNC_TIME.lock().unwrap();
    *last = Some(Instant::now());
    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct RepairResult {
    success: bool,
    message: String,
    backup_path: Option<String>,
}

#[tauri::command]
async fn br_repair_database(cwd: Option<String>) -> Result<RepairResult, String> {
    let working_dir = cwd
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    log_info!("[br_repair] Starting database repair for: {}", working_dir);

    let beads_dir = std::path::Path::new(&working_dir).join(".beads");

    // Check if .beads directory exists
    if !beads_dir.exists() {
        return Err("No .beads directory found in this project".to_string());
    }

    // br stores SQLite + JSONL: the database is rebuilt from issues.jsonl,
    // so a corrupt database is repaired by replacing it and re-importing.
    let db_path = beads_dir.join("beads.db");
    let jsonl_path = beads_dir.join("issues.jsonl");
    let backup_path = beads_dir.join("beads.db.backup");

    // Check if database exists
    if !db_path.exists() {
        return Ok(RepairResult {
            success: true,
            message: "No database to repair - it will be created on next operation".to_string(),
            backup_path: None,
        });
    }

    // Require issues.jsonl for repair (the db is rebuilt from JSONL)
    let jsonl_size = std::fs::metadata(&jsonl_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if !jsonl_path.exists() || jsonl_size == 0 {
        return Err("Cannot repair: issues.jsonl is missing or empty. Your data would be lost.".to_string());
    }
    log_info!("[br_repair] Using JSONL-based repair strategy");

    // Create backup of current database
    if let Err(e) = std::fs::copy(&db_path, &backup_path) {
        log_error!("[br_repair] Failed to create backup: {}", e);
        return Err(format!("Failed to create backup: {}", e));
    }
    log_info!("[br_repair] Backup created at: {:?}", backup_path);

    // Remove database files
    std::fs::remove_file(&db_path).ok();
    std::fs::remove_file(beads_dir.join("beads.db-shm")).ok();
    std::fs::remove_file(beads_dir.join("beads.db-wal")).ok();
    log_info!("[br_repair] Removed old database files");

    // Verify the CLI can now work (it recreates the database from JSONL).
    // br 0.7.6 may rebuild the database and still crash (stack overflow) on that
    // first invocation, so a single failed probe is not a failed repair: retry once
    // and accept success = probe ok, or the db exists and the retry probe ok.
    let probe = || {
        new_command(&get_cli_binary())
            .args(&["list", "--limit=1", "--json"])
            .current_dir(&working_dir)
            .env("PATH", get_extended_path())
            .env("BEADS_PATH", &working_dir)
            .output()
    };

    let mut last_error = String::new();
    for attempt in 1..=2 {
        match probe() {
            Ok(output) if output.status.success() => {
                log_info!("[br_repair] Repair successful - database recreated (probe attempt {})", attempt);
                return Ok(RepairResult {
                    success: true,
                    message: "Database repaired successfully. Your issues have been restored from the backup file.".to_string(),
                    backup_path: Some(backup_path.to_string_lossy().to_string()),
                });
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                last_error = if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                };
                log_warn!("[br_repair] Probe attempt {} failed: {}", attempt, last_error);
            }
            Err(e) => {
                last_error = e.to_string();
                log_warn!("[br_repair] Probe attempt {} failed: {}", attempt, last_error);
            }
        }
    }

    log_error!("[br_repair] Repair verification failed: {}", last_error);
    Err(format!("Repair failed during verification: {}", last_error))
}

// ============================================================================
// Batched Poll Data
// ============================================================================

/// All data needed for a single poll cycle, fetched in one IPC call.
#[derive(Debug, Serialize)]
pub struct PollData {
    #[serde(rename = "openIssues")]
    pub open_issues: Vec<Issue>,
    #[serde(rename = "closedIssues")]
    pub closed_issues: Vec<Issue>,
    #[serde(rename = "readyIssues")]
    pub ready_issues: Vec<Issue>,
}

/// Batched poll: sync once, then fetch all issues + ready in 2 commands (was 3).
/// Replaces 3 separate IPC calls (br_list + br_list(closed) + br_ready) with one.
#[tauri::command]
async fn br_poll_data(cwd: Option<String>) -> Result<PollData, String> {
    log_info!("[br_poll_data] Batched poll starting");

    let cwd_ref = cwd.as_deref();

    // Single sync for the entire poll cycle
    sync_br_database(cwd_ref);

    // Fetch issues with a single --all call (br supports --all)
    let all_output = execute_br("list", &["--all".to_string(), "--limit=0".to_string()], cwd_ref)?;
    let raw_all = parse_issues_tolerant(&all_output, "br_poll_data_all")?;
    let (raw_open, raw_closed): (Vec<_>, Vec<_>) = raw_all.into_iter()
        .partition(|issue: &BrRawIssue| issue.status != "closed");

    // Fetch ready issues
    let ready_output = execute_br("ready", &[], cwd_ref)?;
    let raw_ready = parse_issues_tolerant(&ready_output, "br_poll_data_ready")?;

    log_info!("[br_poll_data] Batched poll done: {} open, {} closed, {} ready",
        raw_open.len(), raw_closed.len(), raw_ready.len());

    // Update mtime AFTER our commands ran, so the next br_check_changed
    // only detects EXTERNAL changes (not our own poll's side effects)
    {
        let working_dir = cwd_ref
            .map(String::from)
            .or_else(|| env::var("BEADS_PATH").ok())
            .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });
        let beads_dir = std::path::Path::new(&working_dir).join(".beads");

        if let Some(mtime) = get_beads_mtime(&beads_dir) {
            let mut map = LAST_KNOWN_MTIME.lock().unwrap();
            map.insert(working_dir, mtime);
        }
    }

    Ok(PollData {
        open_issues: raw_open.into_iter().map(transform_issue).collect(),
        closed_issues: raw_closed.into_iter().map(transform_issue).collect(),
        ready_issues: raw_ready.into_iter().map(transform_issue).collect(),
    })
}

/// Get the latest mtime across the beads database files.
/// br stores SQLite + JSONL: check beads.db, beads.db-wal and issues.jsonl.
fn get_beads_mtime(beads_dir: &std::path::Path) -> Option<std::time::SystemTime> {
    let paths = [
        beads_dir.join("beads.db"),
        beads_dir.join("beads.db-wal"),
        beads_dir.join("issues.jsonl"),
    ];
    paths.iter()
        .filter_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
}

/// Check if the beads database has changed since last check (via filesystem mtime).
/// Returns true if changes detected or if this is the first check.
/// This is extremely cheap — just a few stat() calls, no CLI process spawns.
#[tauri::command]
async fn br_check_changed(cwd: Option<String>) -> Result<bool, String> {
    let working_dir = cwd
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    let beads_dir = std::path::Path::new(&working_dir).join(".beads");
    let current_mtime = get_beads_mtime(&beads_dir);

    let mut map = LAST_KNOWN_MTIME.lock().unwrap();
    let previous = map.get(&working_dir).copied();

    match (current_mtime, previous) {
        (Some(current), Some(prev)) => {
            if current != prev {
                log_info!("[br_check_changed] mtime changed — data may have been modified");
                map.insert(working_dir, current);
                Ok(true)
            } else {
                log_debug!("[br_check_changed] mtime unchanged — no changes");
                Ok(false)
            }
        }
        (Some(current), None) => {
            // First check — store mtime, report changed so initial load happens
            map.insert(working_dir, current);
            Ok(true)
        }
        (None, _) => {
            // No database file found
            log_warn!("[br_check_changed] No beads database found in {}", working_dir);
            Ok(true) // Report changed to let caller handle missing db
        }
    }
}

/// Reset the cached mtime for a specific project (or all projects).
/// Called from the frontend when switching projects to force a fresh poll.
#[tauri::command]
async fn br_reset_mtime(cwd: Option<String>) -> Result<(), String> {
    let mut map = LAST_KNOWN_MTIME.lock().unwrap();
    if let Some(path) = cwd {
        log_info!("[br_reset_mtime] Resetting mtime for: {}", path);
        map.remove(&path);
    } else {
        log_info!("[br_reset_mtime] Resetting all cached mtimes");
        map.clear();
    }
    Ok(())
}

#[tauri::command]
async fn br_list(options: ListOptions) -> Result<Vec<Issue>, String> {
    log_info!("[br_list] cwd: {:?}", options.cwd);

    // Sync database before reading to ensure data is up-to-date
    sync_br_database(options.cwd.as_deref());

    let mut args: Vec<String> = Vec::new();

    // br supports --all in a single call
    let use_all = options.include_all.unwrap_or(false);
    if use_all {
        args.push("--all".to_string());
    }
    if let Some(ref statuses) = options.status {
        if !statuses.is_empty() {
            args.push(format!("--status={}", statuses.join(",")));
        }
    }
    if let Some(ref types) = options.issue_type {
        if !types.is_empty() {
            args.push(format!("--type={}", types.join(",")));
        }
    }
    if let Some(ref priorities) = options.priority {
        if !priorities.is_empty() {
            let nums: Vec<String> = priorities.iter().map(|p| priority_to_number(p)).collect();
            args.push(format!("--priority={}", nums.join(",")));
        }
    }
    if let Some(ref assignee) = options.assignee {
        args.push(format!("--assignee={}", assignee));
    }

    // Always disable limit to get all issues (the CLI defaults to 50)
    args.push("--limit=0".to_string());

    let output = execute_br("list", &args, options.cwd.as_deref())?;

    let raw_issues = parse_issues_tolerant(&output, "br_list")?;

    log_info!("[br_list] Found {} issues", raw_issues.len());
    Ok(raw_issues.into_iter().map(transform_issue).collect())
}

#[tauri::command]
async fn br_count(options: CwdOptions) -> Result<CountResult, String> {
    // Sync database before reading to ensure data is up-to-date
    sync_br_database(options.cwd.as_deref());

    // Fetch all issues in a single --all call
    let all_output = execute_br("list", &["--all".to_string(), "--limit=0".to_string()], options.cwd.as_deref())?;
    let raw_issues = parse_issues_tolerant(&all_output, "br_count_all")?;

    let mut by_type: HashMap<String, usize> = HashMap::new();
    by_type.insert("bug".to_string(), 0);
    by_type.insert("task".to_string(), 0);
    by_type.insert("feature".to_string(), 0);
    by_type.insert("epic".to_string(), 0);
    by_type.insert("chore".to_string(), 0);

    let mut by_priority: HashMap<String, usize> = HashMap::new();
    by_priority.insert("p0".to_string(), 0);
    by_priority.insert("p1".to_string(), 0);
    by_priority.insert("p2".to_string(), 0);
    by_priority.insert("p3".to_string(), 0);
    by_priority.insert("p4".to_string(), 0);

    let mut last_updated: Option<String> = None;

    for issue in &raw_issues {
        let issue_type = issue.issue_type.to_lowercase();
        if by_type.contains_key(&issue_type) {
            *by_type.get_mut(&issue_type).unwrap() += 1;
        }

        let priority_key = format!("p{}", issue.priority);
        if by_priority.contains_key(&priority_key) {
            *by_priority.get_mut(&priority_key).unwrap() += 1;
        }

        if last_updated.is_none() || issue.updated_at > *last_updated.as_ref().unwrap() {
            last_updated = Some(issue.updated_at.clone());
        }
    }

    Ok(CountResult {
        count: raw_issues.len(),
        by_type,
        by_priority,
        last_updated,
    })
}

#[tauri::command]
async fn br_ready(options: CwdOptions) -> Result<Vec<Issue>, String> {
    log_info!("[br_ready] Called with cwd: {:?}", options.cwd);

    // Sync database before reading to ensure data is up-to-date
    sync_br_database(options.cwd.as_deref());

    let output = execute_br("ready", &[], options.cwd.as_deref())?;

    let raw_issues = parse_issues_tolerant(&output, "br_ready")?;

    log_info!("[br_ready] Found {} ready issues", raw_issues.len());
    Ok(raw_issues.into_iter().map(transform_issue).collect())
}

#[tauri::command]
async fn br_status(options: CwdOptions) -> Result<serde_json::Value, String> {
    let output = execute_br("status", &[], options.cwd.as_deref())?;

    serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse status: {}", e))
}

#[tauri::command]
async fn br_show(id: String, options: CwdOptions) -> Result<Option<Issue>, String> {
    log_info!("[br_show] Called for issue: {} with cwd: {:?}", id, options.cwd);

    // Sync database before reading to ensure data is up-to-date
    sync_br_database(options.cwd.as_deref());

    let output = match execute_br("show", std::slice::from_ref(&id), options.cwd.as_deref()) {
        Ok(output) => output,
        Err(e) => {
            // Handle "not found" errors gracefully (future CLI versions may use non-zero exit)
            let err_lower = e.to_lowercase();
            if err_lower.contains("no issue found") || err_lower.contains("not found") {
                log_info!("[br_show] Issue {} not found (error from CLI): {}", id, e);
                return Ok(None);
            }
            return Err(e);
        }
    };

    // Handle empty output (current CLI behavior for missing issues: exit 0, empty stdout)
    let trimmed = output.trim();
    if trimmed.is_empty() {
        log_info!("[br_show] Issue {} not found (empty output from CLI)", id);
        return Ok(None);
    }

    // show can return either a single object or an array
    let result: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| {
            log_error!("[br_show] Failed to parse JSON for {}: {}", id, e);
            format!("Failed to parse issue: {}", e)
        })?;

    let raw_issue: Option<BrRawIssue> = if result.is_array() {
        result.as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    } else {
        serde_json::from_value(result).ok()
    };

    log_info!("[br_show] Issue {} found: {}", id, raw_issue.is_some());
    Ok(raw_issue.map(transform_issue))
}

#[tauri::command]
async fn br_create(payload: CreatePayload) -> Result<Option<Issue>, String> {
    log_info!("[br_create] Creating issue: {:?}", payload.title);
    let mut args: Vec<String> = vec![payload.title.clone()];

    if let Some(ref desc) = payload.description {
        args.push("--description".to_string());
        args.push(desc.clone());
    }
    if let Some(ref t) = payload.issue_type {
        args.push("--type".to_string());
        args.push(t.clone());
    }
    if let Some(ref p) = payload.priority {
        args.push("--priority".to_string());
        args.push(priority_to_number(p));
    }
    if let Some(ref a) = payload.assignee {
        args.push("--assignee".to_string());
        args.push(a.clone());
    }
    if let Some(ref labels) = payload.labels {
        if !labels.is_empty() {
            args.push("--labels".to_string());
            args.push(labels.join(","));
        }
    }
    if let Some(ref ext) = payload.external_ref {
        args.push("--external-ref".to_string());
        args.push(ext.clone());
    }
    if let Some(est) = payload.estimate_minutes {
        args.push("--estimate".to_string());
        args.push(est.to_string());
    }
    if let Some(ref design) = payload.design_notes {
        args.push("--design".to_string());
        args.push(design.clone());
    }
    if let Some(ref acc) = payload.acceptance_criteria {
        args.push("--acceptance".to_string());
        args.push(acc.clone());
    }
    if let Some(ref notes) = payload.working_notes {
        args.push("--notes".to_string());
        args.push(notes.clone());
    }
    if let Some(ref parent) = payload.parent {
        if !parent.is_empty() {
            args.push("--parent".to_string());
            args.push(parent.clone());
        }
    }
    if let Some(ref spec_id) = payload.spec_id {
        if !spec_id.is_empty() {
            args.push("--spec-id".to_string());
            args.push(spec_id.clone());
        }
    }

    let output = execute_br("create", &args, payload.cwd.as_deref())?;

    let raw_issue: BrRawIssue = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse created issue: {}", e))?;

    Ok(Some(transform_issue(raw_issue)))
}

#[tauri::command]
async fn br_update(id: String, updates: UpdatePayload) -> Result<Option<Issue>, String> {
    // Always log update calls for debugging (regardless of LOGGING_ENABLED)
    log::info!("[br_update] Updating issue: {} with cwd: {:?}", id, updates.cwd);
    log::info!("[br_update] Updates: status={:?}, title={:?}, type={:?}", updates.status, updates.title, updates.issue_type);

    let mut args: Vec<String> = vec![id.clone()];

    if let Some(ref title) = updates.title {
        args.push("--title".to_string());
        args.push(title.clone());
    }
    if let Some(ref desc) = updates.description {
        args.push("--description".to_string());
        args.push(desc.clone());
    }
    if let Some(ref t) = updates.issue_type {
        args.push("--type".to_string());
        args.push(t.clone());
    }
    if let Some(ref s) = updates.status {
        args.push("--status".to_string());
        args.push(s.clone());
    }
    if let Some(ref p) = updates.priority {
        args.push("--priority".to_string());
        args.push(priority_to_number(p));
    }
    if let Some(ref a) = updates.assignee {
        args.push("--assignee".to_string());
        args.push(a.clone());
    }
    if let Some(ref labels) = updates.labels {
        args.push("--set-labels".to_string());
        args.push(labels.join(","));
    }
    if let Some(ref ext) = updates.external_ref {
        args.push("--external-ref".to_string());
        args.push(ext.clone());
    }
    if let Some(est) = updates.estimate_minutes {
        args.push("--estimate".to_string());
        args.push(est.to_string());
    }
    if let Some(ref design) = updates.design_notes {
        args.push("--design".to_string());
        args.push(design.clone());
    }
    if let Some(ref acc) = updates.acceptance_criteria {
        args.push("--acceptance".to_string());
        args.push(acc.clone());
    }
    if let Some(ref notes) = updates.working_notes {
        args.push("--notes".to_string());
        args.push(notes.clone());
    }
    if let Some(ref metadata) = updates.metadata {
        args.push("--metadata".to_string());
        args.push(metadata.clone());
    }
    if let Some(ref spec_id) = updates.spec_id {
        args.push("--spec-id".to_string());
        args.push(spec_id.clone());
    }
    if let Some(ref parent) = updates.parent {
        args.push("--parent".to_string());
        args.push(parent.clone());
    }

    log::info!("[br_update] Executing: br update {}", args.join(" "));
    let output = execute_br("update", &args, updates.cwd.as_deref())?;

    log::info!("[br_update] Raw output: {}", output.chars().take(500).collect::<String>());

    // Handle empty output from the CLI (some updates return empty response)
    let trimmed_output = output.trim();
    if trimmed_output.is_empty() {
        log::info!("[br_update] Empty response from CLI, fetching issue {} to get updated data", id);
        // Fetch the updated issue directly
        let show_output = execute_br("show", std::slice::from_ref(&id), updates.cwd.as_deref())?;
        let show_result: serde_json::Value = serde_json::from_str(&show_output)
            .map_err(|e| {
                log::error!("[br_update] Failed to parse show JSON: {}", e);
                format!("Failed to fetch updated issue: {}", e)
            })?;

        let raw_issue: Option<BrRawIssue> = if show_result.is_array() {
            show_result.as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| serde_json::from_value(v.clone()).ok())
        } else {
            serde_json::from_value(show_result).ok()
        };

        return Ok(raw_issue.map(transform_issue));
    }

    // update can return either a single object or an array
    let result: serde_json::Value = serde_json::from_str(trimmed_output)
        .map_err(|e| {
            log::error!("[br_update] Failed to parse JSON: {}", e);
            format!("Failed to parse updated issue: {}", e)
        })?;

    let raw_issue: Option<BrRawIssue> = if result.is_array() {
        log::info!("[br_update] Result is array");
        result.as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    } else {
        log::info!("[br_update] Result is object");
        serde_json::from_value(result.clone()).map_err(|e| {
            log::error!("[br_update] Failed to parse issue from result: {}", e);
            e
        }).ok()
    };

    if let Some(ref issue) = raw_issue {
        log::info!("[br_update] Updated issue {} - new status: {}", id, issue.status);
    } else {
        log::warn!("[br_update] Could not parse updated issue from response");
    }

    Ok(raw_issue.map(transform_issue))
}

#[tauri::command]
async fn br_close(id: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    log_info!("[br_close] Closing issue: {} with cwd: {:?}", id, options.cwd);

    let args = vec![id.clone(), "--suggest-next".to_string()];

    let output = execute_br("close", &args, options.cwd.as_deref())?;

    log_info!("[br_close] Raw output: {}", output.chars().take(500).collect::<String>());

    let result: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| {
            log_error!("[br_close] Failed to parse JSON: {}", e);
            format!("Failed to parse close result: {}", e)
        })?;

    log_info!("[br_close] Issue {} closed successfully", id);
    Ok(result)
}

#[tauri::command]
async fn br_search(query: String, options: CwdOptions) -> Result<Vec<Issue>, String> {
    log_info!("[br_search] Searching for: {} with cwd: {:?}", query, options.cwd);

    let args = vec![query];
    let output = execute_br("search", &args, options.cwd.as_deref())?;

    log_info!("[br_search] Raw output: {}", output.chars().take(500).collect::<String>());

    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return Ok(vec![]);
    }

    let raw: Vec<BrRawIssue> = serde_json::from_str(trimmed)
        .map_err(|e| {
            log_error!("[br_search] Failed to parse JSON: {}", e);
            format!("Failed to parse search results: {}", e)
        })?;

    Ok(raw.into_iter().map(transform_issue).collect())
}

#[tauri::command]
async fn br_label_add(id: String, label: String, options: CwdOptions) -> Result<(), String> {
    log_info!("[br_label_add] Adding label '{}' to issue {}", label, id);
    let args = vec![id, label];
    execute_br("label add", &args, options.cwd.as_deref())?;
    Ok(())
}

#[tauri::command]
async fn br_label_remove(id: String, label: String, options: CwdOptions) -> Result<(), String> {
    log_info!("[br_label_remove] Removing label '{}' from issue {}", label, id);
    let args = vec![id, label];
    execute_br("label remove", &args, options.cwd.as_deref())?;
    Ok(())
}

#[tauri::command]
async fn br_delete(id: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    let args = vec![id.clone(), "--force".to_string()];
    log::info!("[br_delete] Deleting issue: {} with args: {:?}", id, args);
    execute_br("delete", &args, options.cwd.as_deref())?;

    // Sync after delete to push deletion to remote and prevent resurrection
    sync_br_database(options.cwd.as_deref());

    // Clean up attachments folder for this issue
    let project_path = options.cwd.as_deref().unwrap_or(".");
    let abs_project_path = if project_path == "." || project_path.is_empty() {
        env::current_dir().ok()
    } else {
        let p = PathBuf::from(project_path);
        if p.is_relative() {
            env::current_dir().ok().map(|cwd| cwd.join(&p))
        } else {
            Some(p)
        }
    };

    if let Some(path) = abs_project_path {
        if let Ok(abs_path) = path.canonicalize() {
            let att_dir = abs_path.join(".beads").join("attachments").join(issue_short_id(&id));
            if att_dir.exists() && att_dir.is_dir() {
                if let Err(e) = fs::remove_dir_all(&att_dir) {
                    log::warn!("[br_delete] Failed to remove attachments folder: {}", e);
                } else {
                    log::info!("[br_delete] Removed attachments folder: {:?}", att_dir);
                }
            }
        }
    }

    Ok(serde_json::json!({ "success": true, "id": id }))
}

#[tauri::command]
async fn br_comments_add(id: String, content: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    let args = vec![id, content];

    execute_br("comments add", &args, options.cwd.as_deref())?;

    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn br_dep_add(issue_id: String, blocker_id: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    let args = vec![issue_id, blocker_id];

    execute_br("dep add", &args, options.cwd.as_deref())?;

    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn br_dep_remove(issue_id: String, blocker_id: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    let args = vec![issue_id, blocker_id];

    execute_br("dep remove", &args, options.cwd.as_deref())?;

    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn br_dep_add_relation(id1: String, id2: String, relation_type: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    let args = vec![id1, id2, "--type".to_string(), relation_type];

    execute_br("dep add", &args, options.cwd.as_deref())?;

    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn br_dep_remove_relation(id1: String, id2: String, options: CwdOptions) -> Result<serde_json::Value, String> {
    let args = vec![id1, id2];

    execute_br("dep remove", &args, options.cwd.as_deref())?;

    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
async fn br_available_relation_types() -> Vec<serde_json::Value> {
    let types: Vec<(&str, &str)> = vec![
        ("relates-to", "Relates To"),
        ("related", "Related"),
        ("discovered-from", "Discovered From"),
        ("duplicates", "Duplicates"),
        ("supersedes", "Supersedes"),
        ("caused-by", "Caused By"),
        ("replies-to", "Replies To"),
    ];

    types.into_iter().map(|(v, l)| serde_json::json!({ "value": v, "label": l })).collect()
}

/// Strip the Windows verbatim (extended-length) prefix that `canonicalize()` adds.
///
/// `std::fs::canonicalize` returns paths like `\\?\C:\Users\me` on Windows. That
/// prefix is meaningless to users, breaks path joining in the UI, and renders as
/// garbage in the folder picker. Convert back to the familiar form:
///   - `\\?\C:\Users\me` becomes `C:\Users\me`
///   - `\\?\UNC\srv\share` becomes `\\srv\share`
///
/// No-op on any other path, so it is safe (and unit-testable) on Unix too.
fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", rest);
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        // Only unwrap real drive paths (`C:\...`); leave exotic device paths alone.
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return rest.to_string();
        }
    }
    path.to_string()
}

/// Normalize user-typed input before resolving it.
///
/// On Windows a bare drive letter (`D:`) is *drive-relative*: `canonicalize()`
/// resolves it against the process' current directory on that drive rather than
/// the drive root. Anchor it explicitly. Also trims surrounding whitespace and
/// quotes, which Explorer's "Copy as path" adds.
fn normalize_input_path(path: &str) -> String {
    let trimmed = path.trim().trim_matches('"');
    let bytes = trimmed.as_bytes();
    if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return format!(r"{}\", trimmed);
    }
    trimmed.to_string()
}

/// Expand a leading `~` to the user's home directory (`~`, `~/foo`, `~\foo`).
fn expand_home(path: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if path == "~" {
        return home;
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix(r"~\")) {
        return home.join(rest);
    }
    PathBuf::from(path)
}

#[tauri::command]
async fn fs_exists(path: String) -> Result<bool, String> {
    Ok(std::path::Path::new(&path).exists())
}

#[tauri::command]
async fn fs_roots() -> Vec<DirectoryEntry> {
    quick_access_roots()
}

/// Quick-access roots for the folder picker sidebar: known user folders plus,
/// on Windows, every mounted drive. Without this there is no way to reach a
/// second drive except by typing its letter.
fn quick_access_roots() -> Vec<DirectoryEntry> {
    let mut roots: Vec<(String, PathBuf)> = Vec::new();

    if let Some(home) = dirs::home_dir() {
        roots.push(("Home".to_string(), home));
    }
    if let Some(desktop) = dirs::desktop_dir() {
        roots.push(("Desktop".to_string(), desktop));
    }
    if let Some(documents) = dirs::document_dir() {
        roots.push(("Documents".to_string(), documents));
    }

    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if std::path::Path::new(&drive).is_dir() {
            roots.push((drive.clone(), PathBuf::from(&drive)));
        }
    }

    #[cfg(not(windows))]
    roots.push(("/".to_string(), PathBuf::from("/")));

    roots
        .into_iter()
        .filter(|(_, path)| path.is_dir())
        .map(|(name, path)| {
            let beads_path = path.join(".beads");
            DirectoryEntry {
                name,
                path: strip_verbatim_prefix(&path.to_string_lossy()),
                is_directory: true,
                has_beads: beads_path.is_dir(),
                has_grace: path.join(".grace5").is_dir(),
            }
        })
        .collect()
}

#[tauri::command]
async fn fs_list(path: Option<String>) -> Result<FsListResult, String> {
    list_directory(path)
}

fn list_directory(path: Option<String>) -> Result<FsListResult, String> {
    use std::fs;

    let target_path = match path {
        Some(p) if !p.trim().is_empty() => expand_home(&normalize_input_path(&p)),
        _ => dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
    };

    let target_path = target_path.canonicalize()
        .map_err(|e| format!("Cannot resolve path: {}", e))?;

    let entries = fs::read_dir(&target_path)
        .map_err(|e| format!("Cannot read directory: {}", e))?;

    let mut directories: Vec<DirectoryEntry> = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            let full_path = entry.path();
            let has_beads = full_path.join(".beads").is_dir();

            directories.push(DirectoryEntry {
                name,
                path: strip_verbatim_prefix(&full_path.to_string_lossy()),
                is_directory: true,
                has_beads,
                has_grace: full_path.join(".grace5").is_dir(),
            });
        }
    }

    // Sort: beads projects first, then alphabetically
    directories.sort_by(|a, b| {
        match (a.has_beads, b.has_beads) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    let current_beads_path = target_path.join(".beads");

    Ok(FsListResult {
        current_path: strip_verbatim_prefix(&target_path.to_string_lossy()),
        has_beads: current_beads_path.is_dir(),
        has_grace: target_path.join(".grace5").is_dir(),
        entries: directories,
    })
}

// File watcher commands removed - replaced by frontend polling for lower CPU usage

// ============================================================================
// Update Checker
// ============================================================================

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/w3dev33/beads-task-issue-tracker/releases/latest";

/// Get a GitHub token from `gh auth token` (if gh CLI is installed and authenticated).
/// Raises the API rate limit from 60/hour (anonymous) to 5,000/hour (authenticated).
fn get_github_token() -> Option<String> {
    // Check GITHUB_TOKEN env var first
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }
    // Fall back to gh CLI
    let output = new_command("gh")
        .args(&["auth", "token"])
        .output()
        .ok()?;
    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}

/// Build a reqwest client with GitHub auth if available.
fn github_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("beads-task-issue-tracker")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

/// Add GitHub auth header to a request if a token is available.
fn with_github_auth(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match get_github_token() {
        Some(token) => req.bearer_auth(token),
        None => req,
    }
}

fn get_platform_string() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn find_platform_asset(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
    let suffix = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "_macOS-ARM64.dmg"
        } else {
            "_macOS-Intel.dmg"
        }
    } else if cfg!(target_os = "windows") {
        "_Windows.msi"
    } else {
        "_Linux-amd64.AppImage"
    };

    assets.iter().find(|a| a.name.ends_with(suffix))
}

fn compare_versions(current: &str, latest: &str) -> bool {
    // Remove 'v' prefix if present
    let current = current.trim_start_matches('v');
    let latest = latest.trim_start_matches('v');

    let parse_version = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };

    let current_parts = parse_version(current);
    let latest_parts = parse_version(latest);

    for i in 0..3 {
        let c = current_parts.get(i).copied().unwrap_or(0);
        let l = latest_parts.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if c > l {
            return false;
        }
    }
    false
}

#[tauri::command]
async fn check_for_updates() -> Result<UpdateInfo, String> {
    let client = github_client()?;

    let response = with_github_auth(client.get(GITHUB_RELEASES_URL))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch releases: {}", e))?;

    // Handle 404 (no published releases yet)
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: CURRENT_VERSION.to_string(),
            has_update: false,
            release_url: "https://github.com/w3dev33/beads-task-issue-tracker/releases".to_string(),
            download_url: None,
            platform: get_platform_string().to_string(),
            release_notes: None,
        });
    }

    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let has_update = compare_versions(CURRENT_VERSION, &latest_version);

    let download_url = find_platform_asset(&release.assets)
        .map(|a| a.browser_download_url.clone());

    // Fetch CHANGELOG.md via GitHub API (raw.githubusercontent CDN ignores query params for caching)
    let changelog = with_github_auth(
        client
            .get("https://api.github.com/repos/w3dev33/beads-task-issue-tracker/contents/CHANGELOG.md")
            .header("Accept", "application/vnd.github.raw+json")
    )
        .send()
        .await
        .ok()
        .and_then(|r| if r.status().is_success() { Some(r) } else { None });
    let changelog_text = match changelog {
        Some(r) => r.text().await.ok(),
        None => None,
    };

    Ok(UpdateInfo {
        current_version: CURRENT_VERSION.to_string(),
        latest_version,
        has_update,
        release_url: release.html_url,
        download_url,
        platform: get_platform_string().to_string(),
        release_notes: changelog_text.or(release.body),
    })
}

#[tauri::command]
async fn check_for_updates_demo() -> Result<UpdateInfo, String> {
    let client = github_client()?;

    let response = with_github_auth(client.get(GITHUB_RELEASES_URL))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch releases: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();

    let download_url = find_platform_asset(&release.assets)
        .map(|a| a.browser_download_url.clone());

    // Fetch CHANGELOG.md via GitHub API (raw.githubusercontent CDN ignores query params for caching)
    let changelog = with_github_auth(
        client
            .get("https://api.github.com/repos/w3dev33/beads-task-issue-tracker/contents/CHANGELOG.md")
            .header("Accept", "application/vnd.github.raw+json")
    )
        .send()
        .await
        .ok()
        .and_then(|r| if r.status().is_success() { Some(r) } else { None });
    let changelog_text = match changelog {
        Some(r) => r.text().await.ok(),
        None => None,
    };

    // Demo mode: force has_update = true, fake current version as 0.0.0
    Ok(UpdateInfo {
        current_version: "0.0.0".to_string(),
        latest_version,
        has_update: true,
        release_url: release.html_url,
        download_url,
        platform: get_platform_string().to_string(),
        release_notes: changelog_text.or(release.body),
    })
}

#[tauri::command]
async fn check_br_cli_update() -> Result<BrCliUpdateInfo, String> {
    // Get current br version
    let version_str = get_br_version().await;
    if version_str.contains("not found") {
        return Err("br CLI not found".to_string());
    }

    // Parse semver from version string
    let current_tuple = parse_version(&version_str)
        .ok_or_else(|| format!("Could not parse version from: {}", version_str))?;
    let current_version = format!("{}.{}.{}", current_tuple.0, current_tuple.1, current_tuple.2);

    let api_url = "https://api.github.com/repos/Dicklesworthstone/beads_rust/releases/latest";
    let releases_url = "https://github.com/Dicklesworthstone/beads_rust/releases";

    let client = github_client()?;

    let response = with_github_auth(client.get(api_url))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch releases: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let has_update = compare_versions(&current_version, &latest_version);

    Ok(BrCliUpdateInfo {
        current_version,
        latest_version,
        has_update,
        release_url: releases_url.to_string(),
    })
}

#[tauri::command]
async fn download_and_install_update(download_url: String) -> Result<String, String> {
    log::info!("[download_update] Starting download from: {}", download_url);

    // Extract filename from URL
    let filename = download_url
        .rsplit('/')
        .next()
        .unwrap_or("update-download")
        .to_string();
    log::info!("[download_update] Target filename: {}", filename);

    // Download the file
    let client = reqwest::Client::builder()
        .user_agent("beads-task-issue-tracker")
        .build()
        .map_err(|e| {
            log::error!("[download_update] Failed to create HTTP client: {}", e);
            format!("Failed to create HTTP client: {}", e)
        })?;

    log::info!("[download_update] Sending GET request...");
    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| {
            log::error!("[download_update] HTTP request failed: {} (url: {})", e, download_url);
            format!("Failed to download update: {}", e)
        })?;

    let status = response.status();
    let final_url = response.url().to_string();
    log::info!("[download_update] Response status: {} (final URL: {})", status, final_url);

    if !status.is_success() {
        log::error!("[download_update] Download failed with status: {} (url: {})", status, final_url);
        return Err(format!("Download failed with status: {}", status));
    }

    log::info!("[download_update] Reading response bytes...");
    let bytes = response
        .bytes()
        .await
        .map_err(|e| {
            log::error!("[download_update] Failed to read response bytes: {}", e);
            format!("Failed to read download bytes: {}", e)
        })?;
    log::info!("[download_update] Downloaded {} bytes", bytes.len());

    // Save to ~/Downloads
    let download_dir = dirs::download_dir()
        .ok_or_else(|| {
            log::error!("[download_update] Could not find Downloads directory");
            "Could not find Downloads directory".to_string()
        })?;

    let dest_path = download_dir.join(&filename);
    log::info!("[download_update] Saving to: {}", dest_path.display());
    fs::write(&dest_path, &bytes)
        .map_err(|e| {
            log::error!("[download_update] Failed to save file to {}: {}", dest_path.display(), e);
            format!("Failed to save file: {}", e)
        })?;

    let dest_str = dest_path.to_string_lossy().to_string();
    log::info!("[download_update] Saved successfully: {} ({} bytes)", dest_str, bytes.len());

    // On macOS, mount the DMG
    #[cfg(target_os = "macos")]
    {
        if filename.ends_with(".dmg") {
            log::info!("[download_update] Mounting DMG: {}", dest_str);
            Command::new("open")
                .arg(&dest_path)
                .spawn()
                .map_err(|e| {
                    log::error!("[download_update] Failed to open DMG: {}", e);
                    format!("Failed to open DMG: {}", e)
                })?;
        }
    }

    Ok(dest_str)
}

// ============================================================================
// Debug / Logging Commands
// ============================================================================

fn get_log_path() -> PathBuf {
    // Match tauri-plugin-log's LogDir resolution per platform:
    //   macOS:   ~/Library/Logs/com.beads.manager/
    //   Linux:   ~/.local/share/com.beads.manager/logs/  (XDG_DATA_HOME)
    //   Windows: %APPDATA%/com.beads.manager/logs/
    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_default();
        PathBuf::from(home)
            .join("Library/Logs/com.beads.manager/beads.log")
    }
    #[cfg(target_os = "linux")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.beads.manager")
            .join("logs")
            .join("beads.log")
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.beads.manager")
            .join("logs")
            .join("beads.log")
    }
}

#[tauri::command]
async fn get_logging_enabled() -> bool {
    LOGGING_ENABLED.load(Ordering::Relaxed)
}

#[tauri::command]
async fn set_logging_enabled(enabled: bool) {
    LOGGING_ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        log_info!("[debug] Logging enabled");
    }
}

#[tauri::command]
async fn get_verbose_logging() -> bool {
    VERBOSE_LOGGING.load(Ordering::Relaxed)
}

#[tauri::command]
async fn set_verbose_logging(enabled: bool) {
    VERBOSE_LOGGING.store(enabled, Ordering::Relaxed);
    log_info!("[debug] Verbose logging: {}", if enabled { "ON" } else { "OFF" });
}

#[tauri::command]
async fn clear_logs() -> Result<(), String> {
    let log_path = get_log_path();
    if log_path.exists() {
        fs::write(&log_path, "").map_err(|e| format!("Failed to clear logs: {}", e))?;
        log_info!("[debug] Logs cleared");
    }
    Ok(())
}

#[tauri::command]
async fn export_logs() -> Result<String, String> {
    let log_path = get_log_path();
    if !log_path.exists() {
        return Err("No logs to export".to_string());
    }

    // Get export folder: Downloads > Documents > Home
    let export_dir = dirs::download_dir()
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Could not find a folder to export logs".to_string())?;

    // Generate filename with timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let export_filename = format!("beads-logs-{}.log", now);
    let export_path = export_dir.join(&export_filename);

    // Copy log file
    fs::copy(&log_path, &export_path)
        .map_err(|e| format!("Failed to export logs: {}", e))?;

    Ok(export_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn read_logs(tail_lines: Option<usize>) -> Result<String, String> {
    let log_path = get_log_path();
    if !log_path.exists() {
        return Ok(String::new());
    }

    let content = fs::read_to_string(&log_path)
        .map_err(|e| format!("Failed to read logs: {}", e))?;

    // If tail_lines is specified, return only the last N lines
    if let Some(n) = tail_lines {
        let lines: Vec<&str> = content.lines().collect();
        let start = if lines.len() > n { lines.len() - n } else { 0 };
        Ok(lines[start..].join("\n"))
    } else {
        Ok(content)
    }
}

#[tauri::command]
async fn get_log_path_string() -> String {
    get_log_path().to_string_lossy().to_string()
}

#[tauri::command]
async fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!("[frontend] {}", message),
        "warn" => log::warn!("[frontend] {}", message),
        _ => log::info!("[frontend] {}", message),
    }
}

#[tauri::command]
async fn get_br_version() -> String {
    let binary = get_cli_binary();
    match new_command(&binary)
        .arg("--version")
        .env("PATH", get_extended_path())
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if binary != "br" {
                format!("{} ({})", version, binary)
            } else {
                version
            }
        }
        _ => format!("{} not found", binary),
    }
}

#[tauri::command]
async fn open_image_file(path: String) -> Result<(), String> {
    log_info!("[open_image_file] Opening: {}", path);

    // Security: Only allow image file extensions
    let allowed_extensions = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "tiff", "tif"];
    let path_lower = path.to_lowercase();
    let is_image = allowed_extensions.iter().any(|ext| path_lower.ends_with(&format!(".{}", ext)));

    if !is_image {
        return Err("Only image files are allowed".to_string());
    }

    // Verify file exists
    if !std::path::Path::new(&path).exists() {
        return Err(format!("File not found: {}", path));
    }

    // Security: Canonicalize to resolve symlinks/.. and verify inside .beads/attachments/
    let canonical = std::path::Path::new(&path).canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;
    let canonical_str = canonical.to_string_lossy();
    if !canonical_str.contains("/.beads/attachments/") {
        log_warn!("[open_image_file] Refusing to open file outside attachments: {} (resolved: {})", path, canonical_str);
        return Err("Can only open files inside .beads/attachments/".to_string());
    }

    // Use platform-specific command to open file with default application
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        new_command("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ImageData {
    pub base64: String,
    pub mime_type: String,
}

#[tauri::command]
async fn read_image_file(path: String) -> Result<ImageData, String> {
    log_info!("[read_image_file] Reading: {}", path);

    // Security: Only allow image file extensions
    let allowed_extensions: &[(&str, &str)] = &[
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("webp", "image/webp"),
        ("bmp", "image/bmp"),
        ("svg", "image/svg+xml"),
        ("ico", "image/x-icon"),
        ("tiff", "image/tiff"),
        ("tif", "image/tiff"),
    ];

    let path_lower = path.to_lowercase();
    let mime_type = allowed_extensions
        .iter()
        .find(|(ext, _)| path_lower.ends_with(&format!(".{}", ext)))
        .map(|(_, mime)| *mime);

    let mime_type = match mime_type {
        Some(m) => m.to_string(),
        None => return Err("Only image files are allowed".to_string()),
    };

    // Verify file exists
    if !std::path::Path::new(&path).exists() {
        return Err(format!("File not found: {}", path));
    }

    // Security: Canonicalize to resolve symlinks/.. and verify inside .beads/attachments/
    let canonical = std::path::Path::new(&path).canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;
    let canonical_str = canonical.to_string_lossy();
    if !canonical_str.contains("/.beads/attachments/") {
        log_warn!("[read_image_file] Refusing to read file outside attachments: {} (resolved: {})", path, canonical_str);
        return Err("Can only read files inside .beads/attachments/".to_string());
    }

    // Read file and encode as base64
    let data = fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let base64 = base64_encode(&data);

    Ok(ImageData { base64, mime_type })
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let mut buf = [0u8; 3];
        buf[..chunk.len()].copy_from_slice(chunk);

        let n = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);

        result.push(ALPHABET[(n >> 18) as usize & 0x3F] as char);
        result.push(ALPHABET[(n >> 12) as usize & 0x3F] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[(n >> 6) as usize & 0x3F] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[n as usize & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[tauri::command]
async fn purge_orphan_attachments(project_path: String) -> Result<PurgeResult, String> {
    log::info!("[purge_orphan_attachments] project: {}", project_path);

    // Calculate absolute project path (reusing pattern from br_delete)
    let abs_project_path = if project_path == "." || project_path.is_empty() {
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
    } else {
        let p = PathBuf::from(&project_path);
        if p.is_relative() {
            let cwd = env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            cwd.join(&p)
        } else {
            p
        }
    };

    let abs_project_path = abs_project_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;

    // Build attachments directory path
    let attachments_dir = abs_project_path.join(".beads").join("attachments");

    // If attachments directory doesn't exist, nothing to purge
    if !attachments_dir.exists() || !attachments_dir.is_dir() {
        log::info!("[purge_orphan_attachments] No attachments directory found");
        return Ok(PurgeResult {
            deleted_count: 0,
            deleted_folders: vec![],
        });
    }

    // Get list of all existing issue IDs via br list --all
    let existing_ids: std::collections::HashSet<String> = {
        let output = execute_br("list", &["--all".to_string(), "--limit=0".to_string()], Some(&abs_project_path.to_string_lossy()))?;
        let issues = parse_issues_tolerant(&output, "purge_orphan_attachments")?;
        issues.into_iter().map(|i| i.id).collect()
    };

    log::info!("[purge_orphan_attachments] Found {} existing issues", existing_ids.len());

    // List all subdirectories in attachments folder
    let entries = fs::read_dir(&attachments_dir)
        .map_err(|e| format!("Failed to read attachments directory: {}", e))?;

    let mut deleted_folders: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Check if this folder corresponds to an existing issue (folders use short IDs)
        let is_owned = existing_ids.iter().any(|id| issue_short_id(id) == folder_name);
        if !is_owned {
            log::info!("[purge_orphan_attachments] Deleting orphan folder: {}", folder_name);
            if let Err(e) = fs::remove_dir_all(&path) {
                log::warn!("[purge_orphan_attachments] Failed to delete {}: {}", folder_name, e);
            } else {
                deleted_folders.push(folder_name);
            }
        }
    }

    let deleted_count = deleted_folders.len();
    log::info!("[purge_orphan_attachments] Purged {} orphan folders", deleted_count);

    Ok(PurgeResult {
        deleted_count,
        deleted_folders,
    })
}

/// Sanitize a filename for safe storage and br JSONL compatibility.
/// Converts to kebab-case, strips diacritics, removes unsafe chars.
/// Example: "Screenshot 2026-02-24 à 10.30.png" → "screenshot-2026-02-24-a-10-30.png"
fn sanitize_filename(filename: &str) -> String {
    // Split into stem and extension
    let (stem, ext) = match filename.rfind('.') {
        Some(pos) => (&filename[..pos], &filename[pos..]),
        None => (filename, ""),
    };

    // Strip diacritics by replacing common accented chars, then lowercase + kebab-case
    let mut sanitized = String::with_capacity(stem.len());
    for c in stem.chars() {
        let replacement = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "a",
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => "e",
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => "i",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => "o",
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => "u",
            'ñ' | 'Ñ' => "n",
            'ç' | 'Ç' => "c",
            'ß' => "ss",
            'æ' | 'Æ' => "ae",
            'œ' | 'Œ' => "oe",
            'ý' | 'ÿ' | 'Ý' => "y",
            'A'..='Z' => { sanitized.push((c as u8 + 32) as char); continue; },
            'a'..='z' | '0'..='9' => { sanitized.push(c); continue; },
            '-' => { sanitized.push('-'); continue; },
            ' ' | '_' | '.' => { sanitized.push('-'); continue; },
            _ => "-",
        };
        sanitized.push_str(replacement);
    }

    // Collapse multiple consecutive dashes and trim
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_dash = false;
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    let result = result.trim_matches('-');

    let ext_lower = ext.to_lowercase();
    if result.is_empty() {
        format!("file{}", ext_lower)
    } else {
        format!("{}{}", result, ext_lower)
    }
}

// ============================================================================
// Attachment helpers
// ============================================================================

/// Image file extensions supported for attachment preview
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "tiff", "tif"];

/// Markdown file extensions supported for attachment preview
const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown"];

/// Extract the short ID from a full issue ID by stripping the project prefix.
/// e.g. "beads-manager-2qk" → "2qk", "kybio-1pxe" → "1pxe",
///      "kybio-front-nuxt-4-466d" → "466d", "beads-manager-02e.1" → "02e.1"
/// Falls back to the full ID if no prefix separator is found.
fn issue_short_id(full_id: &str) -> &str {
    // The short ID is after the last '-' that isn't followed by another segment
    // containing only digits (which would be part of the prefix like "nuxt-4").
    // Strategy: find the last '-' where everything after it matches [a-z0-9.]+ (the short ID).
    // But "kybio-front-nuxt-4-466d": after last '-' is "466d" ✓
    // "kybio-front-nuxt-4": after last '-' is "4" which could be a short ID or prefix part.
    // Since we only call this with real issue IDs (not project names), the last segment is always the short ID.
    match full_id.rfind('-') {
        Some(pos) => &full_id[pos + 1..],
        None => full_id,
    }
}

/// Resolve the attachment directory for an issue.
/// Always uses short ID: .beads/attachments/{short_id}/
fn resolve_attachment_dir(attachments_dir: &std::path::Path, issue_id: &str) -> PathBuf {
    attachments_dir.join(issue_short_id(issue_id))
}

/// Classify a filename as "image", "markdown", or "other"
fn classify_attachment(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(&format!(".{}", ext))) {
        "image"
    } else if MARKDOWN_EXTENSIONS.iter().any(|ext| lower.ends_with(&format!(".{}", ext))) {
        "markdown"
    } else {
        "other"
    }
}

/// Resolve a duplicate filename: image.png → image-1.png → image-2.png
fn resolve_duplicate_filename(dir: &std::path::Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(pos) => (&name[..pos], &name[pos..]),
        None => (name, ""),
    };
    for i in 1..1000 {
        let candidate = format!("{}-{}{}", stem, i, ext);
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    // Fallback with timestamp
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}-{}{}", stem, ts, ext)
}

// ============================================================================
// Attachment Refs Migration v3 — filesystem-only
// ============================================================================

/// Check if a ref is a "real" external reference (Redmine, GitHub, or other URL/ID).
/// Returns false for att: refs, local file paths, cleared: sentinels.
fn is_real_external_ref(r: &str) -> bool {
    let trimmed = r.trim();
    if trimmed.is_empty() { return false; }
    if trimmed.starts_with("cleared:") { return false; }
    if trimmed.starts_with("att:") { return false; }
    // Local file paths (absolute or relative .beads/)
    if trimmed.starts_with('/') { return false; }
    if trimmed.starts_with(".beads/") { return false; }
    // Anything with path separators inside .beads or attachments is local
    if trimmed.contains("/attachments/") || trimmed.contains("/.beads/") { return false; }
    true
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RefsMigrationStatus {
    needs_migration: bool,
    ref_count: u32,
    just_migrated: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrateRefsResult {
    success: bool,
    refs_updated: u32,
}

/// Check if a project needs attachment refs migration v3.
/// v3 strips all attachment refs (att:, local paths) from external_ref,
/// keeping only real external refs (Redmine, GitHub, URLs).
/// Returns quickly if the .migrated-attachments marker file exists.
#[tauri::command]
async fn check_refs_migration(cwd: Option<String>) -> Result<RefsMigrationStatus, String> {
    let working_dir = cwd
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    let beads_dir = PathBuf::from(&working_dir).join(".beads");
    if !beads_dir.exists() {
        return Ok(RefsMigrationStatus { needs_migration: false, ref_count: 0, just_migrated: false });
    }

    // Already migrated to v3?
    if beads_dir.join(".migrated-attachments").exists() {
        // Check if auto-migration just ran (notify signal)
        let notify_path = beads_dir.join(".migrated-attachments-notify");
        let just_migrated = notify_path.exists();
        if just_migrated {
            let _ = std::fs::remove_file(&notify_path);
        }
        return Ok(RefsMigrationStatus { needs_migration: false, ref_count: 0, just_migrated });
    }

    let jsonl_path = beads_dir.join("issues.jsonl");
    if !jsonl_path.exists() {
        let _ = std::fs::write(beads_dir.join(".migrated-attachments"), "");
        return Ok(RefsMigrationStatus { needs_migration: false, ref_count: 0, just_migrated: false });
    }

    // Scan JSONL for refs that need cleanup (non-real external refs)
    let content = std::fs::read_to_string(&jsonl_path)
        .map_err(|e| format!("Failed to read issues.jsonl: {}", e))?;

    let mut ref_count: u32 = 0;

    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(ext_ref) = v.get("external_ref").and_then(|r| r.as_str()) {
            if ext_ref.is_empty() { continue; }
            let refs: Vec<&str> = ext_ref.split(|c: char| c == '\n' || c == '|').collect();
            for r in &refs {
                let trimmed = r.trim();
                if !trimmed.is_empty() && !is_real_external_ref(trimmed) {
                    ref_count += 1;
                    break; // One bad ref per issue is enough to flag it
                }
            }
        }
    }

    // Also check if attachment folders need renaming (full-id → short-id)
    let mut folder_work_count: u32 = 0;
    let attachments_dir = beads_dir.join("attachments");
    if attachments_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&attachments_dir) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() { continue; }
                let name = entry.file_name().to_string_lossy().to_string();
                if issue_short_id(&name) != name {
                    folder_work_count += 1;
                }
            }
        }
    }

    let total = ref_count + folder_work_count;
    if total == 0 {
        let _ = std::fs::write(beads_dir.join(".migrated-attachments"), "");
        return Ok(RefsMigrationStatus { needs_migration: false, ref_count: 0, just_migrated: false });
    }

    log_info!("[refs_migration_v3] Project needs migration: {} ref(s) to clean, {} folder(s) to update", ref_count, folder_work_count);
    Ok(RefsMigrationStatus { needs_migration: true, ref_count: total, just_migrated: false })
}

/// Perform the attachment refs migration v3 (filesystem-only).
/// Delegates to ensure_refs_migrated_v3 which handles backup, cleanup, dedup, and marker.
/// The br sync is NOT called here — it will happen naturally after via sync_br_database.
#[tauri::command]
async fn migrate_attachment_refs(cwd: Option<String>) -> Result<MigrateRefsResult, String> {
    let working_dir = cwd
        .or_else(|| env::var("BEADS_PATH").ok())
        .unwrap_or_else(|| {
            env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    let beads_dir = PathBuf::from(&working_dir).join(".beads");
    ensure_refs_migrated_v3(&beads_dir, &working_dir);
    Ok(MigrateRefsResult { success: true, refs_updated: 0 })
}

#[tauri::command]
async fn copy_file_to_attachments(
    project_path: String,
    source_path: String,
    issue_id: String,
) -> Result<String, String> {
    log::info!(
        "[copy_file_to_attachments] project: {}, source: {}, issue: {}",
        project_path,
        source_path,
        issue_id
    );

    // Validate file extension (images + markdown)
    let source_lower = source_path.to_lowercase();
    let is_allowed = IMAGE_EXTENSIONS.iter().chain(MARKDOWN_EXTENSIONS.iter())
        .any(|ext| source_lower.ends_with(&format!(".{}", ext)));

    if !is_allowed {
        return Err("Only image and markdown files are allowed".to_string());
    }

    // Verify source file exists
    let source = PathBuf::from(&source_path);
    if !source.exists() {
        return Err(format!("Source file not found: {}", source_path));
    }

    // Calculate absolute project path
    let abs_project_path = if project_path == "." || project_path.is_empty() {
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
    } else {
        let p = PathBuf::from(&project_path);
        if p.is_relative() {
            let cwd = env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            cwd.join(&p)
        } else {
            p
        }
    };

    let abs_project_path = abs_project_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;

    // Build destination directory: {project}/.beads/attachments/{short_id}/
    let attachments_dir = abs_project_path.join(".beads").join("attachments");
    let dest_dir = resolve_attachment_dir(&attachments_dir, &issue_id);

    // Create directory if needed
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create attachments directory: {}", e))?;

    // Sanitize the original filename and handle duplicates
    let raw_filename = source
        .file_name()
        .ok_or_else(|| "Invalid source filename".to_string())?
        .to_string_lossy()
        .to_string();
    let sanitized = sanitize_filename(&raw_filename);
    let dest_filename = resolve_duplicate_filename(&dest_dir, &sanitized);
    let dest_path = dest_dir.join(&dest_filename);

    // Copy the file
    fs::copy(&source, &dest_path).map_err(|e| format!("Failed to copy file: {}", e))?;

    log::info!("[copy_file_to_attachments] Copied to: {}", dest_path.display());

    // Return just the filename (frontend doesn't need to store it in external_ref)
    Ok(dest_filename)
}

// ============================================================================
// Filesystem-based Attachment Commands
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentFile {
    pub filename: String,
    pub file_type: String,   // "image" or "markdown"
    pub path: String,        // absolute path
    pub modified: u64,       // mtime in epoch seconds (for sorting)
}

/// List all attachments for an issue by reading the filesystem directly.
/// Returns images and markdown files sorted by modification time (newest first).
#[tauri::command]
async fn list_attachments(project_path: String, issue_id: String) -> Result<Vec<AttachmentFile>, String> {
    let abs_project_path = if project_path == "." || project_path.is_empty() {
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
    } else {
        let p = PathBuf::from(&project_path);
        if p.is_relative() {
            let cwd = env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            cwd.join(&p)
        } else {
            p
        }
    };

    let attachments_dir = abs_project_path.join(".beads").join("attachments");
    let issue_dir = resolve_attachment_dir(&attachments_dir, &issue_id);

    if !issue_dir.exists() || !issue_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut files: Vec<AttachmentFile> = Vec::new();

    let entries = fs::read_dir(&issue_dir)
        .map_err(|e| format!("Failed to read attachment directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() { continue; }

        let name = entry.file_name().to_string_lossy().to_string();
        // Skip legacy index.json files
        if name == "index.json" { continue; }

        let file_type = classify_attachment(&name);
        // Only return images and markdown
        if file_type == "other" { continue; }

        let modified = entry.metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        files.push(AttachmentFile {
            filename: name,
            file_type: file_type.to_string(),
            path: path.to_string_lossy().to_string(),
            modified,
        });
    }

    // Sort by mtime descending (newest first)
    files.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(files)
}

/// Delete an attachment file by filename within an issue's attachment directory.
#[tauri::command]
async fn delete_attachment(project_path: String, issue_id: String, filename: String) -> Result<(), String> {
    log::info!("[delete_attachment] project: {}, issue: {}, file: {}", project_path, issue_id, filename);

    // Security: reject path traversal
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("Invalid filename".to_string());
    }

    let abs_project_path = if project_path == "." || project_path.is_empty() {
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
    } else {
        let p = PathBuf::from(&project_path);
        if p.is_relative() {
            let cwd = env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            cwd.join(&p)
        } else {
            p
        }
    };

    let abs_project_path = abs_project_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;

    let attachments_dir = abs_project_path.join(".beads").join("attachments");
    let issue_dir = resolve_attachment_dir(&attachments_dir, &issue_id);
    let file_path = issue_dir.join(&filename);

    if !file_path.exists() {
        log::info!("[delete_attachment] File does not exist: {:?}", file_path);
        return Ok(());
    }

    // Security: verify file is inside .beads/attachments/
    let canonical = file_path.canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;
    let canonical_str = canonical.to_string_lossy();
    if !canonical_str.contains("/.beads/attachments/") {
        return Err("Can only delete files inside .beads/attachments/".to_string());
    }

    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete file: {}", e))?;

    log::info!("[delete_attachment] Deleted: {:?}", file_path);

    // Cleanup empty folder (issue_dir already resolved above via resolve_attachment_dir)
    if issue_dir.exists() {
        if let Ok(entries) = fs::read_dir(&issue_dir) {
            // Count non-index.json entries
            let count = entries.flatten()
                .filter(|e| e.file_name().to_string_lossy() != "index.json")
                .count();
            if count == 0 {
                // Remove index.json if present, then the directory
                let _ = fs::remove_file(issue_dir.join("index.json"));
                let _ = fs::remove_dir(&issue_dir);
                log::info!("[delete_attachment] Cleaned up empty folder: {:?}", issue_dir);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct TextData {
    pub content: String,
}

#[tauri::command]
async fn read_text_file(path: String) -> Result<TextData, String> {
    log_info!("[read_text_file] Reading: {}", path);

    // Security: Only allow markdown file extensions
    let path_lower = path.to_lowercase();
    let is_markdown = path_lower.ends_with(".md") || path_lower.ends_with(".markdown");

    if !is_markdown {
        return Err("Only markdown files are allowed".to_string());
    }

    // Verify file exists
    if !std::path::Path::new(&path).exists() {
        return Err(format!("File not found: {}", path));
    }

    // Security: Canonicalize to resolve symlinks/.. and verify inside .beads/attachments/
    let canonical = std::path::Path::new(&path).canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;
    let canonical_str = canonical.to_string_lossy();
    if !canonical_str.contains("/.beads/attachments/") {
        log_warn!("[read_text_file] Refusing to read file outside attachments: {} (resolved: {})", path, canonical_str);
        return Err("Can only read files inside .beads/attachments/".to_string());
    }

    // Read file as UTF-8
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    Ok(TextData { content })
}

#[tauri::command]
async fn write_text_file(path: String, content: String) -> Result<(), String> {
    log_info!("[write_text_file] Writing: {}", path);

    // Security: Only allow markdown file extensions
    let path_lower = path.to_lowercase();
    let is_markdown = path_lower.ends_with(".md") || path_lower.ends_with(".markdown");

    if !is_markdown {
        return Err("Only markdown files are allowed".to_string());
    }

    // Verify file exists (no creation of new files)
    if !std::path::Path::new(&path).exists() {
        return Err(format!("File not found: {}", path));
    }

    // Security: Canonicalize to resolve symlinks/.. and verify inside .beads/attachments/
    let canonical = std::path::Path::new(&path).canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;
    let canonical_str = canonical.to_string_lossy();
    if !canonical_str.contains("/.beads/attachments/") {
        log_warn!("[write_text_file] Refusing to write file outside attachments: {} (resolved: {})", path, canonical_str);
        return Err("Can only write files inside .beads/attachments/".to_string());
    }

    // Write content to file
    fs::write(&path, &content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    log_info!("[write_text_file] Written {} bytes to {}", content.len(), path);
    Ok(())
}

// ============================================================================
// File Watcher Commands
// ============================================================================

#[tauri::command]
fn start_watching(
    path: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<WatcherState>>,
) -> Result<(), String> {
    let mut watcher_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
    watcher_state.watch_session_id = watcher_state.watch_session_id.wrapping_add(1);
    let watch_session_id = watcher_state.watch_session_id;

    // Stop existing watcher if any
    if watcher_state.debouncer.is_some() {
        log::info!("[watcher] Stopping previous watcher for: {:?}", watcher_state.watched_path);
        if let Some(previous_path) = watcher_state.watched_path.clone() {
            if let Some(previous_emit_state) = watcher_state.project_emit_state.get(&previous_path) {
                if let Ok(mut emit_state) = previous_emit_state.lock() {
                    emit_state.session_id = watch_session_id;
                    emit_state.pending = false;
                    emit_state.flush_scheduled = false;
                }
            }
            watcher_state.project_emit_state.remove(&previous_path);
        }
        watcher_state.debouncer = None;
        watcher_state.watched_path = None;
    }

    let project_dir = PathBuf::from(&path);
    let targets = grace::fs::watch_targets(&project_dir);
    if targets.is_empty() {
        return Err(format!(
            "no .beads or .grace5 directory found at: {}",
            project_dir.display()
        ));
    }

    let project_path = path.clone();
    let app_handle = app.clone();
    let project_emit_state = watcher_state
        .project_emit_state
        .entry(path.clone())
        .or_insert_with(|| Arc::new(Mutex::new(ProjectWatcherEmitState::new(watch_session_id))))
        .clone();
    {
        let mut emit_state = project_emit_state
            .lock()
            .map_err(|e| format!("Watcher emit-state lock error: {}", e))?;
        *emit_state = ProjectWatcherEmitState::new(watch_session_id);
    }

    let mut debouncer = new_debouncer(
        Duration::from_millis(WATCHER_DEBOUNCE_INTERVAL_MS),
        move |res: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            match res {
                Ok(events) => {
                    let relevant_count = events
                        .iter()
                        .filter(|event| should_process_watcher_event(event))
                        .count();
                    if relevant_count == 0 {
                        return;
                    }

                    let min_interval = watcher_min_emit_interval();

                    let (emit_now, schedule_delayed_emit, emitted, suppressed) =
                        match project_emit_state.lock() {
                            Ok(mut emit_state) => {
                                if emit_state.session_id != watch_session_id {
                                    return;
                                }

                                let can_emit_now = emit_state
                                    .last_emit
                                    .map(|last| last.elapsed() >= min_interval)
                                    .unwrap_or(true);

                                if can_emit_now {
                                    emit_state.pending = false;
                                    emit_state.last_emit = Some(Instant::now());
                                    emit_state.emitted_batches += 1;
                                    (
                                        true,
                                        false,
                                        emit_state.emitted_batches,
                                        emit_state.suppressed_batches,
                                    )
                                } else {
                                    emit_state.pending = true;
                                    emit_state.suppressed_batches += 1;
                                    let schedule_delayed_emit = if !emit_state.flush_scheduled {
                                        emit_state.flush_scheduled = true;
                                        true
                                    } else {
                                        false
                                    };
                                    (
                                        false,
                                        schedule_delayed_emit,
                                        emit_state.emitted_batches,
                                        emit_state.suppressed_batches,
                                    )
                                }
                            }
                            Err(err) => {
                                log::error!("[watcher] Failed to lock emit state for {}: {}", project_path, err);
                                return;
                            }
                        };

                    if emit_now {
                        emit_beads_changed(&app_handle, &project_path);
                        log::info!(
                            "[watcher] Emitted coalesced update for {} (events={}, relevant={}, emitted={}, suppressed={})",
                            project_path,
                            events.len(),
                            relevant_count,
                            emitted,
                            suppressed
                        );
                    } else {
                        log::info!(
                            "[watcher] Suppressed watcher batch for {} (events={}, relevant={}, emitted={}, suppressed={})",
                            project_path,
                            events.len(),
                            relevant_count,
                            emitted,
                            suppressed
                        );
                    }

                    if schedule_delayed_emit {
                        schedule_pending_watcher_emit(
                            app_handle.clone(),
                            project_path.clone(),
                            project_emit_state.clone(),
                            watch_session_id,
                        );
                    }
                }
                Err(e) => {
                    log::error!("[watcher] Error: {:?}", e);
                }
            }
        },
    ).map_err(|e| format!("Failed to create watcher: {}", e))?;

    // Watch each registry present. Grace artifacts nest (changes/<id>/records/...),
    // so .grace5 is recursive; br's .beads store is flat, so it is non-recursive.
    for target in &targets {
        let is_grace = target.file_name().and_then(|name| name.to_str()) == Some(".grace5");
        let watch_mode = if is_grace {
            notify::RecursiveMode::Recursive
        } else {
            notify::RecursiveMode::NonRecursive
        };
        debouncer
            .watcher()
            .watch(target.as_path(), watch_mode)
            .map_err(|e| format!("Failed to watch {}: {}", target.display(), e))?;
        log::info!("[watcher] Started watching: {}", target.display());
    }
    watcher_state.debouncer = Some(debouncer);
    watcher_state.watched_path = Some(path);

    Ok(())
}

#[tauri::command]
fn stop_watching(
    state: tauri::State<'_, Mutex<WatcherState>>,
) -> Result<(), String> {
    let mut watcher_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
    watcher_state.watch_session_id = watcher_state.watch_session_id.wrapping_add(1);
    let watch_session_id = watcher_state.watch_session_id;

    if watcher_state.debouncer.is_some() {
        log::info!("[watcher] Stopped watching: {:?}", watcher_state.watched_path);
        if let Some(previous_path) = watcher_state.watched_path.clone() {
            if let Some(previous_emit_state) = watcher_state.project_emit_state.get(&previous_path) {
                if let Ok(mut emit_state) = previous_emit_state.lock() {
                    emit_state.session_id = watch_session_id;
                    emit_state.pending = false;
                    emit_state.flush_scheduled = false;
                }
            }
            watcher_state.project_emit_state.remove(&previous_path);
        }
        watcher_state.debouncer = None;
        watcher_state.watched_path = None;
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct WatcherStatusInfo {
    active: bool,
    #[serde(rename = "watchedPath")]
    watched_path: Option<String>,
}

#[tauri::command]
fn get_watcher_status(
    state: tauri::State<'_, Mutex<WatcherState>>,
) -> Result<WatcherStatusInfo, String> {
    let watcher_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    Ok(WatcherStatusInfo {
        active: watcher_state.debouncer.is_some(),
        watched_path: watcher_state.watched_path.clone(),
    })
}

// ============================================================================
// External Data Source Commands
// ============================================================================

#[tauri::command]
async fn fetch_external_data(url: String) -> Result<String, String> {
    log_info!("[probe] GET {}", url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let err = format!("HTTP {}: {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown"));
        log_error!("[probe] GET failed: {}", err);
        return Err(err);
    }

    response.text().await.map_err(|e| format!("Failed to read response: {}", e))
}

#[tauri::command]
async fn check_external_health(url: String) -> Result<bool, String> {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    log_info!("[probe] Health check: {}", health_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    match client.get(&health_url).send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
async fn post_external_data(url: String, body: String) -> Result<String, String> {
    log_info!("[probe] POST {}", url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }

    response.text().await.map_err(|e| format!("Failed to read response: {}", e))
}

#[tauri::command]
async fn delete_external_data(url: String) -> Result<String, String> {
    log_info!("[probe] DELETE {}", url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .delete(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }

    response.text().await.map_err(|e| format!("Failed to read response: {}", e))
}

#[tauri::command]
async fn patch_external_data(url: String, body: String) -> Result<String, String> {
    log_info!("[probe] PATCH {}", url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .patch(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }

    response.text().await.map_err(|e| format!("Failed to read response: {}", e))
}

// ============================================================================
// Probe Launcher
// ============================================================================

#[tauri::command]
async fn launch_probe(port: u16) -> Result<String, String> {
    use std::process::Stdio;

    let health_url = format!("http://127.0.0.1:{}/health", port);

    // Check if probe is already reachable via HTTP health endpoint
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    if let Ok(resp) = client.get(&health_url).send().await {
        if resp.status().is_success() {
            log_info!("[probe] Already running on port {}", port);
            return Ok("already running".to_string());
        }
    }

    // Determine binary: BEADS_PROBE_BIN env var, fallback to "beads-probe"
    let bin = env::var("BEADS_PROBE_BIN").unwrap_or_else(|_| "beads-probe".to_string());
    log_info!("[probe] Launching: {} --port {}", bin, port);

    let child = Command::new(&bin)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", bin, e))?;

    // Store child handle so it lives as long as the app
    if let Ok(mut guard) = PROBE_CHILD.lock() {
        *guard = Some(child);
    }

    log_info!("[probe] Launched on port {}", port);
    Ok("launched".to_string())
}

// ============================================================================
// App Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env file (dev only — in prod there's no .env, env vars come from the system)
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .manage(Mutex::new(WatcherState::default()))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Enable logging in both debug and release builds
            let log_level = if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            };
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log_level)
                    .max_file_size(5_000_000) // 5 MB max per log file
                    .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne) // Keep only one backup
                    .target(tauri_plugin_log::Target::new(
                        tauri_plugin_log::TargetKind::LogDir { file_name: Some("beads.log".into()) },
                    ))
                    .target(tauri_plugin_log::Target::new(
                        tauri_plugin_log::TargetKind::Stdout,
                    ))
                    .build(),
            )?;

            // Create the main window from config, applying titleBarStyle only on macOS
            let window_config = &app.config().app.windows[0];
            #[allow(unused_mut)]
            let mut builder = WebviewWindowBuilder::from_config(app.handle(), window_config)?;
            #[cfg(target_os = "macos")]
            {
                builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay);
            }
            // Windows: no native title bar and no native menu row — AppHeader renders
            // the title bar, the File/Edit/Window menus and the window controls.
            // Linux keeps its native decorations (window manager behaviour varies).
            #[cfg(target_os = "windows")]
            {
                builder = builder.decorations(false).shadow(true);
            }
            builder.build()?;

            // Log startup info
            log::info!("=== Beads Task-Issue Tracker starting ===");
            log::info!("[startup] Extended PATH: {}", get_extended_path());

            // Load config and set the CLI binary (br by default; a stale bd value coerces to br)
            let config = load_config();
            let binary = normalize_cli_binary(&config.cli_binary);
            log::info!("[startup] CLI binary: {}", binary);
            *CLI_BINARY.lock().unwrap() = binary;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            br_sync,
            br_repair_database,
            br_check_changed,
            br_reset_mtime,
            br_poll_data,
            br_list,
            br_count,
            br_ready,
            br_status,
            br_show,
            br_create,
            get_logging_enabled,
            set_logging_enabled,
            get_verbose_logging,
            set_verbose_logging,
            clear_logs,
            export_logs,
            read_logs,
            get_log_path_string,
            log_frontend,
            get_br_version,
            br_update,
            br_close,
            br_search,
            br_label_add,
            br_label_remove,
            br_delete,
            br_comments_add,
            br_dep_add,
            br_dep_remove,
            br_dep_add_relation,
            br_dep_remove_relation,
            br_available_relation_types,
            fs_exists,
            fs_list,
            fs_roots,
            check_for_updates,
            check_for_updates_demo,
            check_br_cli_update,
            download_and_install_update,
            open_image_file,
            read_image_file,
            copy_file_to_attachments,
            list_attachments,
            delete_attachment,
            read_text_file,
            write_text_file,
            purge_orphan_attachments,
            check_refs_migration,
            migrate_attachment_refs,
            start_watching,
            stop_watching,
            get_watcher_status,
            fetch_external_data,
            check_external_health,
            post_external_data,
            delete_external_data,
            patch_external_data,
            launch_probe,
            grace::grace_list,
            grace::grace_show,
            grace::grace_read_file,
            grace::grace_open_file,
            grace::grace_check_changed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_issue_json(id: &str, title: &str) -> String {
        format!(
            r#"{{"id":"{}","title":"{}","description":null,"status":"open","priority":3,"issue_type":"task","owner":null,"assignee":null,"labels":[],"created_at":"2025-01-01T00:00:00Z","created_by":null,"updated_at":"2025-01-01T00:00:00Z","closed_at":null,"close_reason":null,"blocked_by":null,"blocks":null,"comments":null,"external_ref":null,"estimate":null,"design":null,"acceptance_criteria":null,"notes":null,"parent":null,"dependents":null,"dependencies":null,"dependency_count":null,"dependent_count":null,"metadata":null,"spec_id":null,"comment_count":null}}"#,
            id, title
        )
    }

    #[test]
    fn strip_verbatim_prefix_unwraps_drive_paths() {
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\Users\me"), r"C:\Users\me");
        assert_eq!(strip_verbatim_prefix(r"\\?\d:\repos"), r"d:\repos");
    }

    #[test]
    fn strip_verbatim_prefix_unwraps_unc_paths() {
        assert_eq!(strip_verbatim_prefix(r"\\?\UNC\srv\share\dir"), r"\\srv\share\dir");
    }

    #[test]
    fn strip_verbatim_prefix_leaves_other_paths_alone() {
        assert_eq!(strip_verbatim_prefix("/home/dev/project"), "/home/dev/project");
        assert_eq!(strip_verbatim_prefix(r"C:\already\plain"), r"C:\already\plain");
        // Device paths are not drive paths - leave them untouched.
        assert_eq!(strip_verbatim_prefix(r"\\?\Volume{abc}\x"), r"\\?\Volume{abc}\x");
    }

    /// End-to-end guard for the actual bug: `canonicalize()` is what introduced
    /// the `\\?\` prefix that leaked into the folder picker.
    #[test]
    fn canonicalized_home_has_no_verbatim_prefix() {
        let home = dirs::home_dir().expect("home dir");
        let canonical = home.canonicalize().expect("canonicalize home");
        let displayed = strip_verbatim_prefix(&canonical.to_string_lossy());
        assert!(
            !displayed.starts_with(r"\\?\"),
            "verbatim prefix leaked into displayed path: {displayed}"
        );
    }

    /// Exercises the whole `fs_list` path on the real filesystem: neither the
    /// current path nor any child entry may carry the verbatim prefix, because
    /// the children are what the picker navigates into.
    #[test]
    fn list_directory_returns_plain_paths() {
        let result = list_directory(Some("~".to_string())).expect("list home");
        assert!(
            !result.current_path.starts_with(r"\\?\"),
            "current_path carries verbatim prefix: {}",
            result.current_path
        );
        for entry in &result.entries {
            assert!(
                !entry.path.starts_with(r"\\?\"),
                "entry carries verbatim prefix: {}",
                entry.path
            );
        }
    }

    /// A directory counts as a grace project when it holds `.grace5/`, both for
    /// its parent's entry and for the listed current path.
    #[test]
    fn list_directory_reports_grace_projects() {
        let root = std::env::temp_dir().join(format!(
            "grace-fs-list-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let with_grace = root.join("with-grace");
        let without = root.join("without-grace");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(with_grace.join(".grace5")).unwrap();
        std::fs::create_dir_all(&without).unwrap();

        let listing =
            list_directory(Some(root.to_string_lossy().to_string())).expect("list temp root");
        let entry = |name: &str| {
            listing
                .entries
                .iter()
                .find(|entry| entry.name == name)
                .unwrap_or_else(|| panic!("missing entry {name}"))
        };
        assert!(entry("with-grace").has_grace);
        assert!(!entry("without-grace").has_grace);

        let inside = list_directory(Some(with_grace.to_string_lossy().to_string()))
            .expect("list grace project");
        assert!(inside.has_grace);
        let inside_without = list_directory(Some(without.to_string_lossy().to_string()))
            .expect("list plain project");
        assert!(!inside_without.has_grace);

        let _ = std::fs::remove_dir_all(&root);
    }

    fn watched_path(path: &str) -> bool {
        is_watched_path(std::path::Path::new(path))
    }

    #[test]
    fn watcher_accepts_beads_events() {
        assert!(watched_path("C:/repo/.beads/issues.jsonl"));
        assert!(watched_path(r"C:\repo\.beads\issues.jsonl"));
    }

    #[test]
    fn watcher_accepts_grace_events() {
        assert!(watched_path("C:/repo/.grace5/changes/C-0019/contract.xml"));
        assert!(watched_path("C:/repo/.grace5/kernel-version"));
    }

    #[test]
    fn watcher_rejects_unrelated_events() {
        assert!(!watched_path("C:/repo/src/main.rs"));
        assert!(!watched_path("C:/repo/.grace5x/contract.xml"));
        assert!(!watched_path("C:/repo/.beads/issues.jsonl.lock"));
        assert!(!watched_path("C:/repo/notes.md"));
    }

    /// A bare drive letter must resolve to the drive root, not to the process'
    /// current directory on that drive.
    #[test]
    #[cfg(windows)]
    fn list_directory_resolves_bare_drive_letter_to_root() {
        let result = list_directory(Some("C:".to_string())).expect("list C:");
        assert_eq!(result.current_path, r"C:\");
    }

    #[test]
    fn quick_access_roots_are_navigable() {
        let roots = quick_access_roots();
        assert!(!roots.is_empty(), "expected at least one quick-access root");
        for root in &roots {
            assert!(
                !root.path.starts_with(r"\\?\"),
                "root carries verbatim prefix: {}",
                root.path
            );
            // The grace flag must reflect the filesystem for every root.
            assert_eq!(
                root.has_grace,
                std::path::Path::new(&root.path).join(".grace5").is_dir(),
                "has_grace disagrees with the filesystem for {}",
                root.path
            );
            // Every sidebar entry must be something fs_list can open.
            list_directory(Some(root.path.clone()))
                .unwrap_or_else(|e| panic!("root {} is not listable: {e}", root.path));
        }
    }

    #[test]
    #[cfg(windows)]
    fn quick_access_roots_include_the_system_drive() {
        let roots = quick_access_roots();
        assert!(
            roots.iter().any(|r| r.path == r"C:\"),
            "expected C:\\ among quick-access roots, got {:?}",
            roots.iter().map(|r| &r.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn normalize_input_path_anchors_bare_drive_letters() {
        // `D:` is drive-relative on Windows and must become the drive root.
        assert_eq!(normalize_input_path("D:"), r"D:\");
        assert_eq!(normalize_input_path(" c: "), r"c:\");
        assert_eq!(normalize_input_path(r"D:\repos"), r"D:\repos");
    }

    #[test]
    fn normalize_input_path_strips_explorer_quotes() {
        assert_eq!(normalize_input_path("\"C:\\Program Files\""), r"C:\Program Files");
        assert_eq!(normalize_input_path("  /home/dev  "), "/home/dev");
    }

    #[test]
    fn expand_home_handles_tilde_forms() {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/repos"), home.join("repos"));
        assert_eq!(expand_home(r"~\repos"), home.join("repos"));
        // Not a tilde path - passed through verbatim.
        assert_eq!(expand_home("/tmp/x"), PathBuf::from("/tmp/x"));
    }

    #[test]
    fn parse_flat_array() {
        let json = format!("[{}]", minimal_issue_json("abc-123", "Bug fix"));
        let result = parse_issues_tolerant(&json, "test_flat");
        assert!(result.is_ok());
        let issues = result.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "abc-123");
        assert_eq!(issues[0].title, "Bug fix");
    }

    #[test]
    fn parse_paginated_envelope() {
        let json = format!(
            r#"{{"issues":[{},{}],"total":2,"offset":0,"limit":50,"has_more":false}}"#,
            minimal_issue_json("abc-123", "First"),
            minimal_issue_json("def-456", "Second")
        );
        let result = parse_issues_tolerant(&json, "test_envelope");
        assert!(result.is_ok());
        let issues = result.unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].id, "abc-123");
        assert_eq!(issues[1].id, "def-456");
    }

    #[test]
    fn parse_empty_flat_array() {
        let result = parse_issues_tolerant("[]", "test_empty_flat");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_empty_envelope() {
        let json = r#"{"issues":[],"total":0,"offset":0,"limit":50,"has_more":false}"#;
        let result = parse_issues_tolerant(json, "test_empty_envelope");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_object_without_issues_key_fails() {
        let json = r#"{"error":"something went wrong"}"#;
        let result = parse_issues_tolerant(json, "test_bad_object");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_json_fails() {
        let result = parse_issues_tolerant("not json at all", "test_invalid");
        assert!(result.is_err());
    }

    #[test]
    fn parse_real_br_envelope() {
        // Matches the shape from br 0.1.30+ (`br list --json --limit 1`):
        // br omits many optional fields (owner, assignee, labels, etc.) and includes extra
        // fields (source_repo, compaction_level). serde_json defaults missing Option<T> to None
        // and ignores unknown fields, so this parses correctly.
        let json = r#"{"issues":[{"id":"proj-abc","title":"Example bug report","description":"A test description","status":"open","priority":2,"issue_type":"bug","created_at":"2025-06-15T09:30:00.000000000Z","updated_at":"2025-06-15T10:45:00.000000000Z","source_repo":".","compaction_level":0,"dependency_count":0,"dependent_count":0}],"total":1,"limit":1,"offset":0,"has_more":true}"#;
        let result = parse_issues_tolerant(json, "test_real_br");
        assert!(result.is_ok());
        let issues = result.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "proj-abc");
        assert_eq!(issues[0].issue_type, "bug");
        assert_eq!(issues[0].priority, 2);
    }

    #[test]
    fn parse_envelope_skips_malformed_entries() {
        let good = minimal_issue_json("abc-123", "Good");
        let bad = r#"{"id":"bad-456","title":"Bad"}"#; // missing required fields
        let json = format!(r#"{{"issues":[{},{}],"total":2,"offset":0,"limit":50,"has_more":false}}"#, good, bad);
        let result = parse_issues_tolerant(&json, "test_tolerant_envelope");
        assert!(result.is_ok());
        let issues = result.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "abc-123");
    }
}
