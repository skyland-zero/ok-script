//! Serde models mirroring the shared ok-script Web API contract.
//!
//! Every struct here maps 1:1 onto a JSON payload produced by
//! `ok/ui/web/app.py`; the web frontend's `web_src/src/types.ts` is the
//! reference for field names.

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExecutorStatus {
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub starting: bool,
    #[serde(default)]
    pub current_task: Option<String>,
    #[serde(default)]
    pub task_count: usize,
    #[serde(default)]
    pub hotkey: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DeviceOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub keywords: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MethodOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct OverlayState {
    #[serde(default)]
    pub boxes: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CaptureUiState {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub event_session_key: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub status: ExecutorStatus,
    #[serde(default)]
    pub devices: Vec<DeviceOption>,
    #[serde(default)]
    pub capture_methods: Vec<MethodOption>,
    #[serde(default)]
    pub interaction_methods: Vec<MethodOption>,
    #[serde(default)]
    pub overlay: OverlayState,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SystemAccent {
    #[serde(default)]
    pub light: Option<String>,
    #[serde(default)]
    pub dark: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ThemeUiState {
    #[serde(default)]
    pub system_accent: Option<SystemAccent>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TaskConfigField {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub options: Option<Vec<Value>>,
    #[serde(default)]
    pub allow_duplication: bool,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
    #[serde(default)]
    pub sub_config: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AutomationTask {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub trigger: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub waiting_for: Option<String>,
    #[serde(default)]
    pub start_time: f64,
    #[serde(default)]
    pub info: Value,
    #[serde(default)]
    pub config: Vec<TaskConfigField>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SettingsGroup {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub expanded: bool,
    #[serde(default)]
    pub top_level: bool,
    #[serde(default)]
    pub fields: Vec<TaskConfigField>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TaskTabManifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub position: String,
    #[serde(default)]
    pub add_after_default_tabs: bool,
    #[serde(default)]
    pub task_controls: bool,
    #[serde(default)]
    pub task_name: String,
    #[serde(default)]
    pub task_class_name: String,
    #[serde(default)]
    pub module_url: String,
    /// Optional native control tree. Only emitted by backends that opted in;
    /// the web contract has no equivalent and the page falls back to a
    /// WebView host when it is absent.
    #[serde(default)]
    pub gpui_view: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct NavigationCapabilities {
    #[serde(default)]
    pub triggers: bool,
    #[serde(default = "default_true")]
    pub tasks: bool,
    #[serde(default)]
    pub script: bool,
    #[serde(default)]
    pub templates: bool,
    #[serde(default)]
    pub schedule: bool,
    #[serde(default)]
    pub task_tabs: Vec<TaskTabManifest>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AboutProject {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub website: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AboutInfo {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub links: Value,
    #[serde(default)]
    pub projects: Vec<AboutProject>,
    #[serde(default)]
    pub update_supported: bool,
    #[serde(default)]
    pub update_check_delay_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LogResponse {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub line_count: usize,
    #[serde(default)]
    pub modified: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScriptSummary {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub modified: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScriptParameter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScriptTemplate {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub template_name: String,
    #[serde(default)]
    pub params: Vec<ScriptParameter>,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub full_doc: String,
    #[serde(default)]
    pub return_type: String,
    #[serde(default)]
    pub is_property: bool,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub is_static: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScriptDocument {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub modified: f64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TemplateImage {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub modified: f64,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, serde::Serialize)]
pub struct TemplateAnnotation {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub bbox: Vec<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TemplateAnnotations {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub annotations: Vec<TemplateAnnotation>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AvailableScheduleTask {
    #[serde(default)]
    pub index: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScheduledTask {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub trigger_type: String,
    #[serde(default)]
    pub next_run_time: String,
    #[serde(default)]
    pub last_run_time: String,
    #[serde(default)]
    pub last_result: String,
    #[serde(default)]
    pub actions: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub created_time: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub task_index: i64,
    #[serde(default)]
    pub task_identifier: String,
    #[serde(default)]
    pub interval_days: i64,
    #[serde(default)]
    pub interval_hours: i64,
    #[serde(default)]
    pub start_hour: Option<i64>,
    #[serde(default)]
    pub start_minute: Option<i64>,
    #[serde(default)]
    pub timeout_hours: Option<i64>,
    #[serde(default)]
    pub auto_exit: Option<bool>,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScheduleData {
    #[serde(default)]
    pub available_tasks: Vec<AvailableScheduleTask>,
    #[serde(default)]
    pub tasks: Vec<ScheduledTask>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UpdateVersion {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UpdateCheckResult {
    #[serde(default)]
    pub current_version: String,
    #[serde(default)]
    pub versions: Vec<UpdateVersion>,
    #[serde(default)]
    pub update_available: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UpdateApplyResult {
    #[serde(default)]
    pub accepted: bool,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub result: Value,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ActionResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub resource_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScriptExportOptions {
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub manifest: Value,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RuntimeEvent {
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub args: Vec<Value>,
    #[serde(default)]
    pub kwargs: Value,
    #[serde(default)]
    pub ui: Option<CaptureUiState>,
}

impl RuntimeEvent {
    /// `notification` payload: `[message, title, error, tray, show_tab, params]`.
    pub fn notification(&self) -> Option<(String, Option<String>, bool, bool, Value)> {
        if self.event != "notification" {
            return None;
        }
        let message = self
            .args
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let title = self
            .args
            .get(1)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|value| !value.is_empty());
        let error = self.args.get(2).and_then(Value::as_bool).unwrap_or(false);
        let tray = self.args.get(3).and_then(Value::as_bool).unwrap_or(false);
        let params = self.args.get(5).cloned().unwrap_or(Value::Null);
        Some((message, title, error, tray, params))
    }

    /// `task_tab` payload: `[tab_id, event_name, payload]`.
    pub fn task_tab(&self) -> Option<(String, String, Value)> {
        if self.event != "task_tab" {
            return None;
        }
        let tab_id = self.args.first().and_then(Value::as_str)?.to_owned();
        let name = self
            .args
            .get(1)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let payload = self.args.get(2).cloned().unwrap_or(Value::Null);
        Some((tab_id, name, payload))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ApiSnapshot {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Error,
}

#[derive(Debug)]
pub enum Update {
    Snapshot(ApiSnapshot),
    Event(RuntimeEvent),
    /// Successful read or write: the API returns the entity itself, so the
    /// shell re-applies it by path instead of re-fetching whole state.
    Value {
        path: String,
        value: Value,
        message: Option<String>,
        kind: Option<String>,
    },
    Error {
        path: Option<String>,
        message: String,
    },
    Binary {
        path: String,
        bytes: Vec<u8>,
    },
    /// Decoded image payload for `key` (an API image URL).
    Image {
        key: String,
        bytes: Vec<u8>,
    },
}
