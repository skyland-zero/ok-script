//! Application state, request bookkeeping and preference persistence.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;

use crate::i18n;
use crate::model::*;

/// Local UI preferences. The web frontend keeps these in `localStorage`
/// (`ok-script-theme` / `ok-script-language`); the native shell keeps them in
/// a JSON file next to the user profile.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Prefs {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_theme() -> String {
    "Auto".to_owned()
}

fn default_language() -> String {
    "Auto".to_owned()
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
        }
    }
}

pub fn prefs_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| dirs_home())
        .unwrap_or_else(std::env::temp_dir);
    base.join("ok-script").join("gpui-ui.json")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

impl Prefs {
    pub fn load() -> Self {
        let path = prefs_path();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = prefs_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub created: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Capture,
    Triggers,
    Tasks,
    Group(String),
    Script,
    Templates,
    Schedule,
    Settings,
    SettingsGroup(String),
    Notifications,
    About,
    TaskTab(String),
}

impl Page {
    pub fn key(&self) -> String {
        match self {
            Self::Capture => "Capture".into(),
            Self::Triggers => "Triggers".into(),
            Self::Tasks => "Tasks".into(),
            Self::Group(name) => format!("group:{name}"),
            Self::Script => "Script".into(),
            Self::Templates => "Templates".into(),
            Self::Schedule => "Schedule".into(),
            Self::Settings => "Settings".into(),
            Self::SettingsGroup(name) => name.clone(),
            Self::Notifications => "Notifications".into(),
            Self::About => "About".into(),
            Self::TaskTab(id) => format!("task-tab:{id}"),
        }
    }
}

/// One scheduled repeating read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PollKey(pub String);

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub capture: CaptureUiState,
    pub theme: ThemeUiState,
    pub tasks: Vec<AutomationTask>,
    pub settings: Vec<SettingsGroup>,
    pub navigation: NavigationCapabilities,
    pub about: AboutInfo,
    pub scripts: Vec<ScriptSummary>,
    pub script_templates: Vec<ScriptTemplate>,
    pub templates: Vec<TemplateImage>,
    pub schedule: ScheduleData,
    pub logs: Option<LogResponse>,
    pub script_document: Option<ScriptDocument>,
    pub script_code: String,
    pub script_dirty: bool,
    pub script_error: Option<String>,
    pub annotations: Option<TemplateAnnotations>,
    pub updates: Option<UpdateCheckResult>,
    pub updates_checked: bool,
    pub templates_selected: Option<String>,
    pub loaded: bool,
    /// Action keys currently in flight (`pending` in the web frontend).
    pub pending: HashSet<String>,
    pub expanded_tasks: HashSet<String>,
    pub expanded_groups: HashSet<String>,
    pub event_log: Vec<String>,
    pub task_tab_dirty: bool,
}

impl AppState {
    pub fn is_busy(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn pending(&self, key: &str) -> bool {
        self.pending.contains(key)
    }

    pub fn visible_tasks(&self) -> Vec<&AutomationTask> {
        self.tasks.iter().filter(|task| task.visible).collect()
    }

    pub fn task(&self, name: &str) -> Option<&AutomationTask> {
        self.tasks.iter().find(|task| task.name == name)
    }

    pub fn task_groups(&self) -> Vec<String> {
        let mut groups = Vec::new();
        for task in self.visible_tasks() {
            if task.trigger {
                continue;
            }
            if let Some(group) = &task.group_name {
                if !group.is_empty() && !groups.contains(group) {
                    groups.push(group.clone());
                }
            }
        }
        groups
    }

    pub fn top_level_group(&self, name: &str) -> Option<&SettingsGroup> {
        self.settings.iter().find(|group| {
            group.top_level
                && (group.name == name
                    || (group.name == "Notification" && name == "Notifications"))
        })
    }

    pub fn setting_group(&self, name: &str) -> Option<&SettingsGroup> {
        self.settings.iter().find(|group| group.name == name)
    }

    pub fn system_notifications(&self) -> bool {
        self.settings
            .iter()
            .filter(|group| group.name == "Notification")
            .flat_map(|group| group.fields.iter())
            .any(|field| {
                field.key == "System Notification"
                    && field.value.as_bool().unwrap_or(false)
            })
    }
}

/// Split a `/api/...` path into `(kind, tail)` so responses can be routed to
/// the matching slice of state.
pub fn route(path: &str) -> (&str, &str) {
    let rest = path.strip_prefix("/api").unwrap_or(path);
    let rest = rest.trim_start_matches('/');
    match rest.split_once('/') {
        Some((head, tail)) => (head, tail),
        None => (rest, ""),
    }
}

/// Path classifier used by the shell to merge a response entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Capture,
    Theme,
    Tasks,
    Task(String),
    ExecutorStatus,
    Settings,
    SettingsGroup(String),
    Navigation,
    About,
    Logs,
    Scripts,
    ScriptTemplates,
    Script(String),
    ScriptRecord,
    ScriptExportOptions,
    Templates,
    TemplateAnnotations(String),
    TemplateSave,
    Schedule,
    ScheduleItem(String),
    Updates,
    UpdateApply,
    Tool(String),
    Binary(String),
    Unknown,
}

pub fn classify(path: &str) -> Route {
    let (head, tail) = route(path);
    match head {
        "ui" => match tail {
            "capture" => Route::Capture,
            "theme" => Route::Theme,
            "ready" => Route::Unknown,
            _ => Route::Unknown,
        },
        "tasks" => {
            if tail.is_empty() {
                Route::Tasks
            } else {
                Route::Task(tail.to_owned())
            }
        }
        "executor" => Route::ExecutorStatus,
        "settings" => {
            if tail.is_empty() {
                Route::Settings
            } else {
                Route::SettingsGroup(tail.split('/').next().unwrap_or("").to_owned())
            }
        }
        "navigation" => Route::Navigation,
        "about" => Route::About,
        "logs" => Route::Logs,
        "scripts" => {
            if tail.is_empty() {
                Route::Scripts
            } else {
                Route::Script(tail.trim_end_matches('/').to_owned())
            }
        }
        "script-templates" => Route::ScriptTemplates,
        "scripts-record" => Route::ScriptRecord,
        "scripts-export" => Route::ScriptExportOptions,
        "scripts-import" => Route::ScriptExportOptions,
        "templates" => {
            if tail.is_empty() {
                Route::Templates
            } else if tail == "save" {
                Route::TemplateSave
            } else if let Some(name) = tail.strip_suffix("/annotations") {
                Route::TemplateAnnotations(name.to_owned())
            } else if tail == "capture" || tail.ends_with("/delete") || tail.starts_with("image/")
            {
                Route::Templates
            } else {
                Route::Templates
            }
        }
        "schedule" => {
            if tail.is_empty() {
                Route::Schedule
            } else {
                Route::ScheduleItem(tail.trim_end_matches('/').to_owned())
            }
        }
        "updates" => {
            if tail == "apply" {
                Route::UpdateApply
            } else {
                Route::Updates
            }
        }
        "tools" => Route::Tool(tail.to_owned()),
        "devices" | "capture-methods" | "interaction-methods" | "overlay" => Route::Capture,
        _ => Route::Unknown,
    }
}

/// Merge a returned entity into the local state, mirroring the web frontend's
/// "write endpoints return the entity" contract.
pub fn apply_value(state: &mut AppState, path: &str, value: Value) -> bool {
    match classify(path) {
        Route::Capture => {
            if let Ok(capture) = serde_json::from_value::<CaptureUiState>(value) {
                state.capture = capture;
                return true;
            }
        }
        Route::ExecutorStatus => {
            if let Ok(status) = serde_json::from_value::<ExecutorStatus>(value) {
                state.capture.status = status;
                return true;
            }
        }
        Route::Theme => {
            if let Ok(theme) = serde_json::from_value::<ThemeUiState>(value) {
                state.theme = theme;
                return true;
            }
        }
        Route::Tasks => {
            if let Ok(tasks) = serde_json::from_value::<Vec<AutomationTask>>(value) {
                state.tasks = tasks;
                return true;
            }
        }
        Route::Task(_) => {
            if let Ok(task) = serde_json::from_value::<AutomationTask>(value) {
                match state.tasks.iter().position(|item| item.name == task.name) {
                    Some(index) => state.tasks[index] = task,
                    None => state.tasks.push(task),
                }
                return true;
            }
        }
        Route::Settings => {
            if let Ok(groups) = serde_json::from_value::<Vec<SettingsGroup>>(value) {
                state.settings = groups;
                return true;
            }
        }
        Route::SettingsGroup(name) => {
            if let Ok(group) = serde_json::from_value::<SettingsGroup>(value) {
                match state.settings.iter().position(|item| item.name == group.name) {
                    Some(index) => state.settings[index] = group,
                    None => {
                        state.settings.push(group);
                        let _ = name;
                    }
                }
                return true;
            }
        }
        Route::Navigation => {
            if let Ok(navigation) = serde_json::from_value::<NavigationCapabilities>(value) {
                state.navigation = navigation;
                return true;
            }
        }
        Route::About => {
            if let Ok(about) = serde_json::from_value::<AboutInfo>(value) {
                state.about = about;
                return true;
            }
        }
        Route::Logs => {
            if let Ok(logs) = serde_json::from_value::<LogResponse>(value) {
                state.logs = Some(logs);
                return true;
            }
        }
        Route::Scripts => {
            if let Ok(scripts) = serde_json::from_value::<Vec<ScriptSummary>>(value) {
                state.scripts = scripts;
                return true;
            }
        }
        Route::ScriptTemplates => {
            if let Ok(templates) = serde_json::from_value::<Vec<ScriptTemplate>>(value) {
                state.script_templates = templates;
                return true;
            }
        }
        Route::Script(tail) => {
            if tail.ends_with("/run") || tail.ends_with("/copy") || tail.ends_with("/delete") {
                if tail.ends_with("/delete") {
                    return true;
                }
                if let Ok(document) = serde_json::from_value::<ScriptDocument>(value) {
                    state.script_document = Some(document);
                    return true;
                }
                return true;
            }
            if let Ok(document) = serde_json::from_value::<ScriptDocument>(value) {
                state.script_dirty = false;
                if let Some(error) = document.error.clone() {
                    state.script_error = Some(error);
                }
                state.script_code = document.code.clone();
                state.script_document = Some(document);
                return true;
            }
        }
        Route::ScriptRecord => {
            if let Some(code) = value.get("code").and_then(Value::as_str) {
                state.script_code = code.to_owned();
                state.script_dirty = true;
            }
            return true;
        }
        Route::ScriptExportOptions => return true,
        Route::Templates => {
            if let Ok(templates) = serde_json::from_value::<Vec<TemplateImage>>(value) {
                state.templates = templates;
                return true;
            }
        }
        Route::TemplateAnnotations(_) => {
            if let Ok(annotations) = serde_json::from_value::<TemplateAnnotations>(value) {
                state.annotations = Some(annotations);
                return true;
            }
        }
        Route::TemplateSave => return true,
        Route::Schedule | Route::ScheduleItem(_) => {
            if let Ok(schedule) = serde_json::from_value::<ScheduleData>(value) {
                state.schedule = schedule;
                return true;
            }
        }
        Route::Updates => {
            if let Ok(updates) = serde_json::from_value::<UpdateCheckResult>(value) {
                state.updates = Some(updates);
                state.updates_checked = true;
                return true;
            }
        }
        Route::UpdateApply => return true,
        Route::Tool(_) => return true,
        Route::Binary(_) => return true,
        Route::Unknown => return false,
    }
    false
}

/// Extract a human message from an entity response (`ActionResult`).
pub fn response_message(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(Value::as_str)
        .map(|message| i18n::t(message))
}

/// `Time Elapsed` formatting used by running task cards.
pub fn elapsed_text(start_time: f64) -> String {
    if start_time <= 0.0 {
        return String::new();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or(start_time);
    let total = (now - start_time).max(0.0) as u64;
    format!("{}h {}m {}s", total / 3600, (total % 3600) / 60, total % 60)
}

/// Cache of lazily created input entities.
#[derive(Default)]
pub struct Inputs {
    values: HashMap<String, gpui::Entity<gpui_component::input::InputState>>,
}

impl Inputs {
    pub fn get(&self, key: &str) -> Option<gpui::Entity<gpui_component::input::InputState>> {
        self.values.get(key).cloned()
    }

    pub fn insert(
        &mut self,
        key: impl Into<String>,
        state: gpui::Entity<gpui_component::input::InputState>,
    ) {
        self.values.insert(key.into(), state);
    }
}
