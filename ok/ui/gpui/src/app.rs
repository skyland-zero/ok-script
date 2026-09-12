//! The native shell: sidebar navigation, page routing, event pump, toasts and
//! the modal host. Structure mirrors the web frontend's `App.tsx`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, px, AnyElement, App, Context, Div, ElementId, FontWeight, Hsla, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, WindowAppearance,
};
use gpui_component::{ActiveTheme as _, StyledExt as _, ThemeMode};
use serde_json::{json, Value};

use crate::api::{self, ApiClient};
use crate::components as ui;
use crate::i18n;
use crate::icons::OkIcon;
use crate::model::*;
use crate::state::{
    apply_value, classify, elapsed_text, AppState, Inputs, Page, Prefs, Route, Toast,
};
use crate::theme;

/// Delay between queue drains; the web frontend re-renders immediately, the
/// native shell polls at 4 Hz which is also the input latency budget.
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const TASK_POLL_INTERVAL: Duration = Duration::from_millis(1000);
const LOG_POLL_INTERVAL: Duration = Duration::from_millis(750);
const SCRIPT_POLL_INTERVAL: Duration = Duration::from_millis(2000);
const TOAST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    DeleteScript(String),
    DeleteTemplate(String),
    DeleteAnnotation(String, Option<i64>),
    DeleteSchedule(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Modal {
    CapturePreview,
    Instructions {
        title: String,
        text: String,
    },
    Logs,
    Confirm {
        title: String,
        message: String,
        confirm_label: String,
        action: ConfirmAction,
    },
    UnsavedScript,
    ExternalScriptChange,
    CreateScript,
    RecordScript,
    ExportScript,
    ImportScript {
        file_name: String,
        bytes: Vec<u8>,
        accepted: bool,
        seconds: i64,
    },
    TemplateSaveTo,
    Markup,
    ScheduleEditor {
        name: Option<String>,
    },
    ListEditor {
        target: ConfigTarget,
        field: TaskConfigField,
        selected: Vec<Value>,
        active: Option<usize>,
        draft: String,
    },
    BboxEditor {
        image: String,
        index: Option<i64>,
        category: String,
        x: String,
        y: String,
        width: String,
        height: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigTarget {
    Task(String),
    Settings(String),
}

pub struct OkApp {
    pub client: Arc<ApiClient>,
    pub queue: Arc<Mutex<VecDeque<Update>>>,
    pub state: AppState,
    pub prefs: Prefs,
    pub page: Page,
    pub pending_page: Option<Page>,
    pub collapsed: bool,
    pub toasts: Vec<Toast>,
    pub toasts_enabled: bool,
    pub modal: Option<Modal>,
    pub inputs: Inputs,
    pub logs_level: String,
    pub logs_query: String,
    pub logs_paused: bool,
    pub capture_preview: Option<String>,
    pub system_dark: bool,
    pub system_accent: Option<SystemAccent>,
    pub min_width: f32,
    pub min_height: f32,
    pub window_title: String,
    pub debug: bool,
    pub script_recording: bool,
    pub script_open: Option<String>,
    pub script_cursor: usize,
    pub template_index: usize,
    pub markup_mode: MarkupMode,
    pub markup_selected: Option<usize>,
    pub markup_view: Option<[f64; 4]>,
    pub update_notice_at: Option<Instant>,
    pub device_query: String,
    pub template_query: String,
    pub script_query: String,
    pub open_select: Option<String>,
    pub input_values: std::collections::HashMap<String, String>,
    pub images: std::collections::HashMap<String, Arc<gpui::RenderImage>>,
    /// Raw RGBA pixels per image URL (markup editor sampling).
    pub pixels: std::collections::HashMap<String, Arc<image::RgbaImage>>,
    pub markup: crate::markup::MarkupState,
    pub script_ui: crate::script::ScriptUiState,
    pub task_tab: crate::task_tab::TaskTabState,
    pub schedule_form: crate::schedule_dialog::ScheduleForm,
    pub save_to: crate::modals::SaveToState,
    pub close_guard_installed: bool,
    /// Template image to open in the markup editor right after the first snapshot.
    pub start_markup: Option<String>,
    /// Script to open in the editor right after the first snapshot.
    pub start_script: Option<String>,
    pub last_task_poll: Instant,
    pub last_log_poll: Instant,
    pub last_script_poll: Instant,
    pub last_theme_poll: Instant,
    snapshot_ready: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkupMode {
    Draw,
    Delete,
    Modify,
}

impl OkApp {
    pub fn new(
        client: Arc<ApiClient>,
        queue: Arc<Mutex<VecDeque<Update>>>,
        prefs: Prefs,
        min_width: f32,
        min_height: f32,
        debug: bool,
        start_page: Option<Page>,
        start_markup: Option<String>,
        start_script: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let language = prefs.language.clone();
        i18n::set_locale(&language);
        let mut view = Self {
            client,
            queue,
            state: AppState::default(),
            prefs,
            page: start_page.unwrap_or(Page::Capture),
            pending_page: None,
            collapsed: false,
            toasts: Vec::new(),
            toasts_enabled: true,
            modal: None,
            inputs: Inputs::default(),
            logs_level: "ALL".to_owned(),
            logs_query: String::new(),
            logs_paused: false,
            capture_preview: None,
            system_dark: false,
            system_accent: None,
            min_width,
            min_height,
            window_title: String::new(),
            debug,
            script_recording: false,
            script_open: None,
            script_cursor: 0,
            template_index: 0,
            markup_mode: MarkupMode::Draw,
            markup_selected: None,
            markup_view: None,
            update_notice_at: None,
            device_query: String::new(),
            template_query: String::new(),
            script_query: String::new(),
            open_select: None,
            input_values: std::collections::HashMap::new(),
            images: std::collections::HashMap::new(),
            pixels: std::collections::HashMap::new(),
            markup: crate::markup::MarkupState::default(),
            script_ui: crate::script::ScriptUiState::default(),
            task_tab: crate::task_tab::TaskTabState::default(),
            schedule_form: crate::schedule_dialog::ScheduleForm::default(),
            save_to: crate::modals::SaveToState::default(),
            close_guard_installed: false,
            start_markup: start_markup.clone(),
            start_script: start_script.clone(),
            last_task_poll: Instant::now(),
            last_log_poll: Instant::now(),
            last_script_poll: Instant::now(),
            last_theme_poll: Instant::now(),
            snapshot_ready: false,
        };
        view.apply_local_theme(cx);
        view.poll_loop(cx);
        view
    }

    fn poll_loop(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |weak, cx| loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            if weak
                .update(cx, |view, cx| {
                    view.tick(cx);
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let changed = self.drain_updates(cx);
        if let Some(name) = self.start_markup.clone() {
            if self.state.templates.iter().any(|item| item.name == name) {
                self.start_markup = None;
                self.open_markup(name, cx);
            }
        }
        if let Some(name) = self.start_script.clone() {
            if self.state.scripts.iter().any(|item| item.name == name) {
                self.start_script = None;
                self.open_script(&name, cx);
            }
        }
        let polled = self.poll_endpoints(cx);
        let expired = self.expire_toasts();
        if changed || polled || expired {
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- updates

    fn drain_updates(&mut self, cx: &mut Context<Self>) -> bool {
        let updates = self
            .queue
            .lock()
            .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        if updates.is_empty() {
            return false;
        }
        for update in updates {
            match update {
                Update::Snapshot(snapshot) => {
                    self.snapshot_ready = true;
                    self.state.capture = snapshot.capture;
                    self.state.theme = snapshot.theme;
                    self.state.tasks = snapshot.tasks;
                    self.state.settings = snapshot.settings;
                    self.state.navigation = snapshot.navigation;
                    self.state.about = snapshot.about;
                    self.state.scripts = snapshot.scripts;
                    self.state.script_templates = snapshot.script_templates;
                    self.state.templates = snapshot.templates;
                    self.state.schedule = snapshot.schedule;
                    self.state.loaded = true;
                    self.system_accent = self.state.theme.system_accent.clone();
                    self.apply_local_theme(cx);
                    self.schedule_update_check();
                }
                Update::Event(event) => self.apply_event(event, cx),
                Update::Value {
                    path,
                    value,
                    message,
                    kind,
                } => {
                    self.state.pending.remove(&path);
                    if path.ends_with("/api/scripts-record/start") {
                        self.script_recording = true;
                    } else if path.ends_with("/api/scripts-record/stop") {
                        self.script_recording = false;
                    }
                    let applied = apply_value(&mut self.state, &path, value.clone());
                    if !applied {
                        self.refresh_for(&path);
                    }
                    if let Some(message) = message {
                        self.toast(ToastKind::Success, i18n::t(&message));
                    }
                    if kind.as_deref() == Some("capture") {
                        if let Some(url) = value
                            .get("resource_url")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                        {
                            self.capture_preview = Some(url);
                            self.modal = Some(Modal::CapturePreview);
                        }
                    }
                }
                Update::Error { path, message } => {
                    if let Some(path) = &path {
                        self.state.pending.remove(path);
                    }
                    self.toast(ToastKind::Error, message);
                }
                Update::Binary { path, bytes } => {
                    self.state.pending.remove(&path);
                    self.handle_binary(&path, bytes);
                }
                Update::Image { key, bytes } => {
                    if let Some(decoded) = crate::images::decode(&bytes) {
                        self.pixels
                            .insert(key.clone(), Arc::new(decoded.rgba));
                        self.images.insert(key, decoded.render);
                    }
                }
            }
        }
        true
    }

    fn apply_event(&mut self, event: RuntimeEvent, cx: &mut Context<Self>) {
        if let Some(ui) = event.ui.clone() {
            self.state.capture = ui;
        }
        let name = event.event.clone();
        match name.as_str() {
            "notification" => {
                if let Some((message, title, error, _tray, params)) = event.notification() {
                    let text = i18n::interpolate(&i18n::t(&message), &params);
                    let text = match title {
                        Some(title) if !title.is_empty() => {
                            format!("{} · {}", i18n::t(&title), text)
                        }
                        _ => text,
                    };
                    self.toast(
                        if error {
                            ToastKind::Error
                        } else {
                            ToastKind::Info
                        },
                        text,
                    );
                }
            }
            "task" | "task_done" | "executor_paused" => {
                if event.ui.is_none() {
                    self.background_get("/api/ui/capture");
                }
                self.background_get("/api/tasks");
            }
            "task_list_updated" => {
                self.background_get("/api/tasks");
                self.background_get("/api/navigation");
            }
            "adb_devices" => {
                self.state.pending.remove("/api/devices/refresh");
                if event.ui.is_none() {
                    self.background_get("/api/ui/capture");
                }
            }
            "task_tab" => {
                if let Some((tab_id, name, payload)) = event.task_tab() {
                    self.push_task_tab_event(&tab_id, &name, payload);
                }
            }
            _ => {}
        }
        if !name.is_empty() {
            self.state.event_log.push(name);
            if self.state.event_log.len() > 200 {
                self.state.event_log.remove(0);
            }
        }
        let _ = cx;
    }

    fn handle_binary(&mut self, path: &str, bytes: Vec<u8>) {
        if path.contains("/api/scripts-export") {
            let directory = std::env::var_os("USERPROFILE")
                .map(std::path::PathBuf::from)
                .map(|home| home.join("Downloads"))
                .unwrap_or_else(std::env::temp_dir);
            let file_name = self
                .input_values
                .get("export-file-name")
                .map(|name| name.trim())
                .filter(|name| !name.is_empty())
                .map(|name| {
                    if name.ends_with(".okscript") {
                        name.to_owned()
                    } else {
                        format!("{name}.okscript")
                    }
                })
                .unwrap_or_else(|| "ok-script-tasks.okscript".to_owned());
            let file = directory.join(file_name);
            match std::fs::write(&file, bytes) {
                Ok(()) => self.toast(
                    ToastKind::Success,
                    format!("{}: {}", i18n::t("Export Script"), file.display()),
                ),
                Err(error) => self.toast(ToastKind::Error, error.to_string()),
            }
        }
    }

    fn refresh_for(&mut self, path: &str) {
        match classify(path) {
            Route::Capture | Route::Tool(_) => self.background_get("/api/ui/capture"),
            Route::Templates => self.background_get("/api/templates"),
            Route::Scripts => self.background_get("/api/scripts"),
            Route::Script(_) => {
                if let Some(name) = self.script_open.clone() {
                    self.background_get(&format!("/api/scripts/{}", api::url_encode(&name)));
                }
            }
            _ => {}
        }
    }

    fn expire_toasts(&mut self) -> bool {
        let before = self.toasts.len();
        self.toasts.retain(|toast| {
            toast.kind == ToastKind::Success || toast.created.elapsed() < TOAST_TIMEOUT
        });
        before != self.toasts.len()
    }

    // ---------------------------------------------------------------- polling

    fn poll_endpoints(&mut self, cx: &mut Context<Self>) -> bool {
        let mut busy = false;
        let stateful_page = matches!(
            self.page,
            Page::Tasks | Page::Triggers | Page::Group(_) | Page::TaskTab(_)
        );
        if stateful_page && self.last_task_poll.elapsed() >= TASK_POLL_INTERVAL {
            self.last_task_poll = Instant::now();
            self.background_get("/api/tasks");
            busy = true;
        }
        if self.modal == Some(Modal::Logs)
            && !self.logs_paused
            && self.last_log_poll.elapsed() >= LOG_POLL_INTERVAL
        {
            self.last_log_poll = Instant::now();
            self.background_get(&self.logs_path());
            busy = true;
        }
        if self.page == Page::Script
            && self.script_open.is_some()
            && self.last_script_poll.elapsed() >= SCRIPT_POLL_INTERVAL
        {
            self.last_script_poll = Instant::now();
            if let Some(name) = self.script_open.clone() {
                self.background_get(&format!("/api/scripts/{}", api::url_encode(&name)));
            }
            busy = true;
        }
        if self.prefs.theme == "Auto" && self.last_theme_poll.elapsed() >= Duration::from_secs(30) {
            self.last_theme_poll = Instant::now();
            self.background_get("/api/ui/theme");
            busy = true;
        }
        if self.state.about.update_supported && !self.state.updates_checked {
            self.schedule_update_check();
        }
        if let Some(at) = self.update_notice_at {
            if at.elapsed() >= Duration::from_millis(self.state.about.update_check_delay_ms) {
                self.update_notice_at = None;
                self.background_get("/api/updates?release_only=true");
                busy = true;
            }
        }
        let _ = cx;
        busy
    }

    fn schedule_update_check(&mut self) {
        if self.state.about.update_supported && !self.state.updates_checked {
            if self.update_notice_at.is_none() {
                self.update_notice_at = Some(Instant::now());
            }
        }
    }

    fn logs_path(&self) -> String {
        let query = format!(
            "level={}&query={}",
            self.logs_level,
            api::url_encode(&self.logs_query)
        );
        format!("/api/logs?{query}")
    }

    // ------------------------------------------------------------- dispatching

    // ---------------------------------------------------------------- images

    /// Fetch and decode a local API image once, then reuse the cached frame.
    pub fn ensure_image(&mut self, url: &str) {
        if self.images.contains_key(url) {
            return;
        }
        let path = url.to_owned();
        api::run_image(self.client.clone(), self.queue.clone(), path.clone(), path);
    }

    // ------------------------------------------------------------ text inputs

    /// Lazily create (and subscribe) an [`InputState`] for the given key.
    pub fn ensure_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        key: &str,
        placeholder: &str,
        default: &str,
        multi_line: bool,
    ) -> gpui::Entity<gpui_component::input::InputState> {
        if let Some(state) = self.inputs.get(key) {
            return state;
        }
        let value = self
            .input_values
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_owned());
        let placeholder = placeholder.to_owned();
        let state = cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(value)
                .multi_line(multi_line)
        });
        let key_owned = key.to_owned();
        cx.subscribe(
            &state,
            move |view, state, event, cx| match event {
                gpui_component::input::InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    view.input_values.insert(key_owned.clone(), value);
                }
                gpui_component::input::InputEvent::PressEnter { .. }
                | gpui_component::input::InputEvent::Blur => {
                    let value = state.read(cx).value().to_string();
                    view.input_values.insert(key_owned.clone(), value.clone());
                    view.commit_input(&key_owned, value, cx);
                }
                _ => {}
            },
        )
        .detach();
        self.inputs.insert(key, state.clone());
        state
    }

    /// Commit a typed value; the key encodes its destination.
    pub fn commit_input(&mut self, key: &str, value: String, cx: &mut Context<Self>) {
        match key {
            "device-search" => {
                self.device_query = value;
                cx.notify();
                return;
            }
            "template-search" => {
                self.template_query = value;
                cx.notify();
                return;
            }
            "script-search" => {
                self.script_query = value;
                cx.notify();
                return;
            }
            "logs-query" => {
                self.logs_query = value;
                self.last_log_poll = Instant::now() - LOG_POLL_INTERVAL;
                cx.notify();
                return;
            }
            _ => {}
        }
        if let Some(rest) = key.strip_prefix("config:") {
            let mut parts = rest.splitn(3, ':');
            let kind = parts.next().unwrap_or_default();
            let name = parts.next().unwrap_or_default();
            let field = parts.next().unwrap_or_default();
            let target = if kind == "task" {
                ConfigTarget::Task(name.to_owned())
            } else {
                ConfigTarget::Settings(name.to_owned())
            };
            let parsed = match value.trim() {
                text if text.parse::<f64>().is_ok() && !text.contains('.') => {
                    Value::from(text.parse::<i64>().unwrap_or_default())
                }
                text if text.parse::<f64>().is_ok() => {
                    Value::from(text.parse::<f64>().unwrap_or_default())
                }
                text => Value::from(text),
            };
            self.set_config(&target, field, parsed, cx);
            return;
        }
        cx.notify();
    }

    fn background_get(&mut self, path: &str) {
        let (path, query) = match path.split_once('?') {
            Some((path, query)) => (path.to_owned(), Some(query.to_owned())),
            None => (path.to_owned(), None),
        };
        api::run_get(self.client.clone(), self.queue.clone(), path, query);
    }

    /// User-triggered read: shows the pending state on the matching controls.
    pub fn get(&mut self, path: &str, cx: &mut Context<Self>) {
        self.state.pending.insert(path.to_owned());
        self.background_get(path);
        cx.notify();
    }

    /// User-triggered write; `path` doubles as the pending key.
    pub fn post(&mut self, path: &str, body: Option<Value>, cx: &mut Context<Self>) {
        if self.state.pending.contains(path) {
            return;
        }
        self.state.pending.insert(path.to_owned());
        api::run_post(self.client.clone(), self.queue.clone(), path.to_owned(), body);
        cx.notify();
    }

    pub fn post_action(&mut self, path: &str, action: &str, cx: &mut Context<Self>) {
        self.post(path, Some(json!({ "action": action })), cx);
    }

    pub fn set_config(
        &mut self,
        target: &ConfigTarget,
        key: &str,
        value: Value,
        cx: &mut Context<Self>,
    ) {
        let path = match target {
            ConfigTarget::Task(name) => {
                format!("/api/tasks/{}/config", api::url_encode(name))
            }
            ConfigTarget::Settings(name) => {
                format!("/api/settings/{}/config", api::url_encode(name))
            }
        };
        self.post(&path, Some(json!({ "key": key, "value": value })), cx);
    }

    pub fn reset_config(&mut self, target: &ConfigTarget, cx: &mut Context<Self>) {
        let path = match target {
            ConfigTarget::Task(name) => {
                format!("/api/tasks/{}/config/reset", api::url_encode(name))
            }
            ConfigTarget::Settings(name) => {
                format!("/api/settings/{}/reset", api::url_encode(name))
            }
        };
        self.post(&path, None, cx);
    }

    // ------------------------------------------------------------------ toast

    pub fn toast(&mut self, kind: ToastKind, message: impl Into<String>) {
        let message = message.into();
        if message.trim().is_empty() {
            return;
        }
        if self
            .toasts
            .iter()
            .any(|toast| toast.message.eq_ignore_ascii_case(&message))
        {
            return;
        }
        self.toasts.insert(
            0,
            Toast {
                kind,
                message,
                created: Instant::now(),
            },
        );
        while self.toasts.len() > 6 {
            self.toasts.pop();
        }
    }

    pub fn dismiss_toast(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.toasts.len() {
            self.toasts.remove(index);
            cx.notify();
        }
    }

    // -------------------------------------------------------- theme and prefs

    pub fn apply_local_theme(&mut self, cx: &mut Context<Self>) {
        let mode = match self.prefs.theme.as_str() {
            "Light" => ThemeMode::Light,
            "Dark" => ThemeMode::Dark,
            _ => {
                if self.system_dark {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                }
            }
        };
        let dark = matches!(mode, ThemeMode::Dark);
        let accent = if self.prefs.theme == "Auto" {
            theme::accent_for(self.system_accent.as_ref(), dark)
        } else {
            theme::parse_hex(&format!("#{:06x}", theme::WINDOWS_STANDARD_BLUE))
                .unwrap_or_else(|| gpui::rgb(theme::WINDOWS_STANDARD_BLUE).into())
        };
        theme::apply(cx, mode, accent);
    }

    pub fn set_theme(&mut self, theme_name: &str, cx: &mut Context<Self>) {
        self.prefs.theme = theme_name.to_owned();
        self.prefs.save();
        self.apply_local_theme(cx);
        cx.notify();
    }

    pub fn set_language(&mut self, language: &str, cx: &mut Context<Self>) {
        self.prefs.language = language.to_owned();
        self.prefs.save();
        i18n::set_locale(language);
        cx.notify();
    }

    // ------------------------------------------------------------- navigation

    pub fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        let leaving_script = self.page == Page::Script && page != Page::Script;
        let leaving_tab = matches!(self.page, Page::TaskTab(_)) && page != self.page;
        if (leaving_script && self.state.script_dirty)
            || (leaving_tab && self.task_tab_dirty())
        {
            self.pending_page = Some(page);
            self.modal = Some(Modal::UnsavedScript);
            cx.notify();
            return;
        }
        self.set_page(page, cx);
    }

    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        // A task tab owns an OS child window: it has to be hidden explicitly,
        // GPUI does not clip it to the page area.
        if matches!(self.page, Page::TaskTab(_)) && !matches!(page, Page::TaskTab(_)) {
            self.hide_task_tab(cx);
        }
        self.page = page;
        self.modal = None;
        cx.notify();
    }

    pub fn nav_items(&self) -> (Vec<NavEntry>, Vec<NavEntry>) {
        let mut primary = Vec::new();
        let mut secondary = Vec::new();
        let tabs = &self.state.navigation.task_tabs;

        primary.push(NavEntry::page(Page::Capture, "Capture", OkIcon::Capture));
        for tab in tabs.iter().filter(|tab| {
            tab.position == "scroll" && !tab.add_after_default_tabs
        }) {
            primary.push(NavEntry::tab(tab));
        }
        if self.state.navigation.triggers {
            primary.push(NavEntry::page(Page::Triggers, "Triggers", OkIcon::Timer));
        }
        if self.state.navigation.tasks {
            primary.push(NavEntry::page(Page::Tasks, "Tasks", OkIcon::TaskList));
        }
        for group in self.state.task_groups() {
            primary.push(NavEntry::label(
                Page::Group(group.clone()),
                group,
                OkIcon::TaskList,
            ));
        }
        for tab in tabs.iter().filter(|tab| {
            tab.position == "scroll" && tab.add_after_default_tabs
        }) {
            primary.push(NavEntry::tab(tab));
        }
        if self.state.navigation.script {
            primary.push(NavEntry::page(Page::Script, "Script", OkIcon::Edit));
        }
        if self.state.navigation.templates {
            primary.push(NavEntry::page(Page::Templates, "Templates", OkIcon::Image));
        }
        if self.state.navigation.schedule {
            primary.push(NavEntry::page(Page::Schedule, "Schedule", OkIcon::Calendar));
        }
        for group in self.state.settings.iter().filter(|group| {
            group.top_level && group.name != "Notification" && !group.name.is_empty()
        }) {
            primary.push(NavEntry::label(
                Page::SettingsGroup(group.name.clone()),
                group.name.clone(),
                OkIcon::Settings,
            ));
        }

        for tab in tabs.iter().filter(|tab| tab.position == "bottom") {
            secondary.push(NavEntry::tab(tab));
        }
        secondary.push(NavEntry::page(
            Page::Notifications,
            "Notifications",
            OkIcon::Alert,
        ));
        secondary.push(NavEntry::page(Page::Settings, "Settings", OkIcon::Settings));
        secondary.push(NavEntry::page(Page::About, "About", OkIcon::QuestionCircle));
        (primary, secondary)
    }

    // --------------------------------------------------------------- rendering

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let width = if self.collapsed {
            ui::SIDEBAR_COLLAPSED_WIDTH
        } else {
            ui::SIDEBAR_WIDTH
        };
        let (primary, secondary) = self.nav_items();
        let collapsed = self.collapsed;
        let toggle = div()
            .id("nav-toggle")
            .grid()
            .items_center().justify_center()
            .size(px(34.0))
            .ml(px(3.0))
            .mb(px(1.0))
            .rounded(px(ui::BUTTON_RADIUS))
            .cursor_pointer()
            .hover(|style| style.bg(gpui::rgba(0xffffff0e)))
            .on_click(cx.listener(|view, _, _, cx| {
                view.collapsed = !view.collapsed;
                cx.notify();
            }))
            .child(
                OkIcon::Navigation
                    .icon()
                    .size(px(17.0))
                    .text_color(cx.theme().muted_foreground),
            );

        let render_group = |entries: Vec<NavEntry>, cx: &mut Context<Self>| -> Vec<AnyElement> {
            entries
                .into_iter()
                .map(|entry| {
                    let key = entry.page.key();
                    let active = self.page == entry.page;
                    let show_dot = matches!(entry.page, Page::About)
                        && self
                            .state
                            .updates
                            .as_ref()
                            .map(|updates| updates.update_available)
                            .unwrap_or(false);
                    let page = entry.page.clone();
                    ui::nav_item(
                        &key,
                        entry.label,
                        entry.icon,
                        active,
                        collapsed,
                        show_dot,
                        cx,
                        cx.listener(move |view, _, _, cx| {
                            view.navigate(page.clone(), cx);
                        }),
                    )
                })
                .collect()
        };

        div()
            .v_flex()
            .h_full()
            .w(px(width))
            .flex_none()
            .bg(cx.theme().sidebar)
            .overflow_hidden()
            .child(toggle)
            .child(
                div()
                    .v_flex()
                    .gap(px(1.0))
                    .px(px(4.0))
                    .children(render_group(primary, cx)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .v_flex()
                    .gap(px(1.0))
                    .px(px(4.0))
                    .pb(px(6.0))
                    .children(render_group(secondary, cx)),
            )
            .into_any_element()
    }

    fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let page = self.page.clone();
        let content = match page {
            Page::Capture => self.render_capture(window, cx),
            Page::Triggers => self.render_task_list(TaskFilter::Triggers, window, cx),
            Page::Tasks => self.render_task_list(TaskFilter::Tasks, window, cx),
            Page::Group(name) => self.render_task_list(TaskFilter::Group(name), window, cx),
            Page::Script => {
                self.poll_script(window, cx);
                self.render_script(window, cx)
            }
            Page::Templates => self.render_templates(window, cx),
            Page::Schedule => self.render_schedule(window, cx),
            Page::Settings => self.render_settings(window, cx),
            Page::SettingsGroup(name) => self.render_top_level_group(&name, window, cx),
            Page::Notifications => self.render_notifications(window, cx),
            Page::About => self.render_about(window, cx),
            Page::TaskTab(id) => self.render_task_tab(&id, window, cx),
        };
        let background = if ui::is_dark(cx) {
            gpui::rgb(0x2e1f26)
        } else {
            gpui::rgb(0xf7f0f4)
        };
        div()
            .v_flex()
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .overflow_hidden()
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .m(px(3.0))
                    .mr(px(3.0))
                    .ml(px(0.0))
                    .rounded_tl(px(12.0))
                    .bg(background)
                    .overflow_hidden()
                    .child(content),
            )
            .into_any_element()
    }

    fn render_toasts(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.toasts.is_empty() {
            return div().into_any_element();
        }
        let cards: Vec<AnyElement> = self
            .toasts
            .iter()
            .enumerate()
            .map(|(index, toast)| {
                let card = div()
                    .relative()
                    .w_full()
                    .child(ui::toast_card(toast.kind, toast.message.clone(), cx))
                    .child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "toast-close-{index}"
                            ))))
                            .absolute()
                            .top(px(12.0))
                            .right(px(8.0))
                            .cursor_pointer()
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.dismiss_toast(index, cx);
                            }))
                            .child(
                                OkIcon::Close
                                    .icon()
                                    .size(px(18.0))
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    );
                card.into_any_element()
            })
            .collect();
        div()
            .absolute()
            .top(px(16.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .justify_center()
            .child(
                div()
                    .v_flex()
                    .w(px(560.0))
                    .gap_2()
                    .children(cards),
            )
            .into_any_element()
    }

    fn render_page_header(&self, title: impl Into<SharedString>, cx: &App) -> Div {
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .w_full()
            .child(ui::page_title(title.into(), cx))
    }

    fn status_line(&self, cx: &App) -> String {
        let status = &self.state.capture.status;
        if status.starting {
            i18n::t("Starting")
        } else if status.paused {
            i18n::t("Paused")
        } else if status.running {
            i18n::t("Running")
        } else {
            i18n::t("Idle")
        }
    }

    fn task_state_text(task: &AutomationTask) -> String {
        if task.paused {
            i18n::t("Paused")
        } else if task.running {
            i18n::t("Running")
        } else if task.enabled {
            i18n::t("Enabled")
        } else {
            i18n::t("Disabled")
        }
    }
}

#[derive(Clone, Debug)]
pub struct NavEntry {
    pub page: Page,
    pub label: String,
    pub icon: OkIcon,
}

impl NavEntry {
    fn page(page: Page, label: &str, icon: OkIcon) -> Self {
        Self {
            page,
            label: i18n::t(label),
            icon,
        }
    }

    fn label(page: Page, label: String, icon: OkIcon) -> Self {
        Self {
            page,
            label,
            icon,
        }
    }

    fn tab(tab: &TaskTabManifest) -> Self {
        Self {
            page: Page::TaskTab(tab.id.clone()),
            label: match tab.name.is_empty() {
                true => tab.id.clone(),
                false => i18n::t(&tab.name),
            },
            icon: OkIcon::from_key(&tab.icon),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TaskFilter {
    Tasks,
    Triggers,
    Group(String),
}

impl Render for OkApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = matches!(window.appearance(), WindowAppearance::Dark);
        if dark != self.system_dark {
            self.system_dark = dark;
            self.apply_local_theme(cx);
        }
        let title = if self.state.capture.title.is_empty() {
            "ok-script".to_owned()
        } else {
            self.state.capture.title.clone()
        };
        if self.window_title != title {
            window.set_window_title(&title);
            self.window_title = title;
        }

        // `.beforeunload` equivalent: refuse to close while a script has
        // unsaved edits, and offer the same three-way dialog instead.
        if !self.close_guard_installed {
            self.close_guard_installed = true;
            let weak = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_window, cx| {
                let dirty = weak
                    .update(cx, |view, cx| {
                        if view.state.script_dirty {
                            view.modal = Some(Modal::UnsavedScript);
                            view.pending_page = None;
                            cx.notify();
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);
                !dirty
            });
        }

        let sidebar = self.render_sidebar(cx);
        let content = self.render_content(window, cx);
        let modal = if self.modal.is_some() {
            Some(self.render_modal(window, cx))
        } else {
            None
        };
        let toasts = self.render_toasts(cx);
        let background = cx.theme().background;
        let foreground = cx.theme().foreground;

        div()
            .relative()
            .h_flex()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .child(sidebar)
            .child(content)
            .when_some(modal, |this, modal| this.child(modal))
            .child(toasts)
    }
}

