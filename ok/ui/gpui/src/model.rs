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
pub struct ScriptTemplate {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub template_name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub class_name: String,
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
    pub description: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScheduleData {
    #[serde(default)]
    pub available_tasks: Vec<Value>,
    #[serde(default)]
    pub tasks: Vec<ScheduledTask>,
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

#[derive(Clone, Debug, Default)]
pub struct ApiSnapshot {
    pub capture: CaptureUiState,
    pub tasks: Vec<AutomationTask>,
    pub settings: Vec<SettingsGroup>,
    pub navigation: NavigationCapabilities,
    pub about: AboutInfo,
    pub logs: Option<LogResponse>,
    pub scripts: Vec<ScriptSummary>,
    pub script_templates: Vec<ScriptTemplate>,
    pub templates: Vec<TemplateImage>,
    pub schedule: ScheduleData,
}

#[derive(Debug)]
pub enum Update {
    Snapshot(ApiSnapshot),
    Event(RuntimeEvent),
    ActionFinished {
        message: String,
        snapshot: Option<ApiSnapshot>,
    },
    Error(String),
}
