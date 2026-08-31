use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use gpui::{div, prelude::*, px, rgb, AnyElement, Context, Render, Window};
use serde_json::{json, Value};

use crate::{
    api::{run_action, ApiClient},
    model::{
        AboutInfo, ApiSnapshot, AutomationTask, CaptureUiState, NavigationCapabilities,
        RuntimeEvent, ScheduleData, ScriptSummary, ScriptTemplate, SettingsGroup, TaskConfigField,
        TaskTabManifest, TemplateImage, Update,
    },
};

const BACKGROUND: u32 = 0x111318;
const SIDEBAR: u32 = 0x171a20;
const PANEL: u32 = 0x1d2129;
const PANEL_ALT: u32 = 0x242a34;
const BORDER: u32 = 0x343b48;
const TEXT: u32 = 0xf2f4f8;
const MUTED: u32 = 0xa9b1bf;
const ACCENT: u32 = 0x60cdff;
const SUCCESS: u32 = 0x75d48b;
const DANGER: u32 = 0xff7b86;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Page {
    Capture,
    Triggers,
    Tasks,
    Script,
    Templates,
    Schedule,
    Settings,
    About,
    Custom(String),
}

impl Page {
    fn key(&self) -> String {
        match self {
            Self::Capture => "capture".into(),
            Self::Triggers => "triggers".into(),
            Self::Tasks => "tasks".into(),
            Self::Script => "script".into(),
            Self::Templates => "templates".into(),
            Self::Schedule => "schedule".into(),
            Self::Settings => "settings".into(),
            Self::About => "about".into(),
            Self::Custom(id) => format!("custom-{id}"),
        }
    }

    fn title(&self, tabs: &[TaskTabManifest]) -> String {
        match self {
            Self::Capture => "Capture".into(),
            Self::Triggers => "Triggers".into(),
            Self::Tasks => "Tasks".into(),
            Self::Script => "Script".into(),
            Self::Templates => "Templates".into(),
            Self::Schedule => "Schedule".into(),
            Self::Settings => "Settings".into(),
            Self::About => "About".into(),
            Self::Custom(id) => tabs
                .iter()
                .find(|tab| &tab.id == id)
                .map(|tab| tab.name.clone())
                .unwrap_or_else(|| id.clone()),
        }
    }
}

pub struct GpuiView {
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    capture: CaptureUiState,
    tasks: Vec<AutomationTask>,
    settings: Vec<SettingsGroup>,
    navigation: NavigationCapabilities,
    about: AboutInfo,
    scripts: Vec<ScriptSummary>,
    script_templates: Vec<ScriptTemplate>,
    templates: Vec<TemplateImage>,
    schedule: ScheduleData,
    page: Page,
    notice: Option<(bool, String)>,
    busy: bool,
    min_width: f32,
    min_height: f32,
    window_title: String,
    event_history: Vec<String>,
}

impl GpuiView {
    pub fn new(
        client: Arc<ApiClient>,
        queue: Arc<Mutex<VecDeque<Update>>>,
        min_width: f32,
        min_height: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = Self {
            client,
            queue,
            capture: CaptureUiState::default(),
            tasks: Vec::new(),
            settings: Vec::new(),
            navigation: NavigationCapabilities::default(),
            about: AboutInfo::default(),
            scripts: Vec::new(),
            script_templates: Vec::new(),
            templates: Vec::new(),
            schedule: ScheduleData::default(),
            page: Page::Capture,
            notice: None,
            busy: true,
            min_width,
            min_height,
            window_title: String::new(),
            event_history: Vec::new(),
        };
        cx.spawn(async move |weak_view, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            if weak_view
                .update(cx, |view, cx| {
                    if view.drain_updates() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
        view
    }

    fn apply_snapshot(&mut self, snapshot: ApiSnapshot) {
        self.capture = snapshot.capture;
        self.tasks = snapshot.tasks;
        self.settings = snapshot.settings;
        self.navigation = snapshot.navigation;
        self.about = snapshot.about;
        self.scripts = snapshot.scripts;
        self.script_templates = snapshot.script_templates;
        self.templates = snapshot.templates;
        self.schedule = snapshot.schedule;
        self.busy = false;
        if self.capture.title.is_empty() {
            self.capture.title = "ok-script".into();
        }
    }

    fn drain_updates(&mut self) -> bool {
        let updates = self
            .queue
            .lock()
            .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        let changed = !updates.is_empty();
        for update in updates {
            match update {
                Update::Snapshot(snapshot) => self.apply_snapshot(snapshot),
                Update::Event(event) => self.apply_event(event),
                Update::ActionFinished { message, snapshot } => {
                    self.busy = false;
                    self.notice = Some((true, message));
                    if let Some(snapshot) = snapshot {
                        self.apply_snapshot(snapshot);
                    }
                }
                Update::Error(error) => {
                    self.busy = false;
                    self.notice = Some((false, error));
                }
            }
        }
        changed
    }

    fn apply_event(&mut self, event: RuntimeEvent) {
        if let Some(ui) = event.ui {
            self.capture = ui;
        }
        if !event.event.is_empty() {
            self.event_history.push(event.event);
            if self.event_history.len() > 80 {
                self.event_history.remove(0);
            }
        }
    }

    fn dispatch(&mut self, path: String, body: Option<Value>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.notice = None;
        run_action(self.client.clone(), self.queue.clone(), path, body);
    }

    fn action_button(
        &self,
        id: &str,
        label: &str,
        path: &str,
        body: Option<Value>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let path = path.to_owned();
        let id = gpui::SharedString::from(id.to_owned());
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL_ALT))
            .text_color(rgb(TEXT))
            .cursor_pointer()
            .active(|style| style.bg(rgb(ACCENT)).text_color(rgb(BACKGROUND)))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.dispatch(path.clone(), body.clone());
                cx.notify();
            }))
            .child(label.to_owned())
            .into_any_element()
    }

    fn nav_button(&self, page: Page, label: &str, cx: &Context<Self>) -> AnyElement {
        let active = self.page == page;
        let key = gpui::SharedString::from(page.key());
        let selected_page = page.clone();
        let background = if active { ACCENT } else { SIDEBAR };
        let foreground = if active { BACKGROUND } else { TEXT };
        div()
            .id(gpui::SharedString::from(format!("nav-{key}")))
            .w_full()
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(background))
            .text_color(rgb(foreground))
            .cursor_pointer()
            .on_click(cx.listener(move |view, _, _, cx| {
                view.page = selected_page.clone();
                view.notice = None;
                cx.notify();
            }))
            .child(label.to_owned())
            .into_any_element()
    }

    fn nav_items(&self) -> Vec<(Page, String)> {
        let mut items = vec![(Page::Capture, "Capture".to_owned())];
        if self.navigation.triggers {
            items.push((Page::Triggers, "Triggers".into()));
        }
        if self.navigation.tasks {
            items.push((Page::Tasks, "Tasks".into()));
        }
        if self.navigation.script {
            items.push((Page::Script, "Script".into()));
        }
        if self.navigation.templates {
            items.push((Page::Templates, "Templates".into()));
        }
        if self.navigation.schedule {
            items.push((Page::Schedule, "Schedule".into()));
        }
        for tab in &self.navigation.task_tabs {
            items.push((Page::Custom(tab.id.clone()), tab.name.clone()));
        }
        items.extend([
            (Page::Settings, "Settings".into()),
            (Page::About, "About".into()),
        ]);
        items
    }

    fn render_header(&self, cx: &Context<Self>) -> AnyElement {
        let title = self.page.title(&self.navigation.task_tabs);
        let status = status_text(&self.capture.status);
        let status_color = if self.capture.status.running {
            SUCCESS
        } else {
            MUTED
        };
        let refresh = self.action_button(
            "header-refresh",
            "Refresh",
            "/api/devices/refresh",
            None::<Value>,
            cx,
        );
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .py_4()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(title)
                    .child(div().text_sm().text_color(rgb(MUTED)).child(status)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(status_color))
                            .child(if self.busy { "syncing…" } else { "connected" }),
                    )
                    .child(refresh),
            )
            .into_any_element()
    }

    fn render_capture(&self, cx: &Context<Self>) -> AnyElement {
        let status = &self.capture.status;
        let pause_path = if status.paused {
            "/api/executor/resume"
        } else {
            "/api/executor/pause"
        };
        let pause_label = if status.paused { "Resume" } else { "Pause" };
        let mut children = vec![
            self.card_title("Runtime"),
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(vec![
                    self.action_button(
                        "executor-pause",
                        pause_label,
                        pause_path,
                        None::<Value>,
                        cx,
                    ),
                    self.action_button(
                        "executor-stop",
                        "Stop current task",
                        "/api/executor/stop-task",
                        None::<Value>,
                        cx,
                    ),
                    self.action_button(
                        "devices-refresh",
                        "Refresh devices",
                        "/api/devices/refresh",
                        None::<Value>,
                        cx,
                    ),
                    self.action_button(
                        "overlay-boxes",
                        if self.capture.overlay.boxes {
                            "Hide boxes"
                        } else {
                            "Show boxes"
                        },
                        "/api/overlay",
                        Some(json!({"name":"boxes","value":!self.capture.overlay.boxes})),
                        cx,
                    ),
                ])
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .text_sm()
                .text_color(rgb(MUTED))
                .children(vec![
                    div()
                        .child(format!("State: {}", status_text(status)))
                        .into_any_element(),
                    div()
                        .child(format!("Tasks in queue: {}", status.task_count))
                        .into_any_element(),
                    div()
                        .child(format!(
                            "Current task: {}",
                            status.current_task.as_deref().unwrap_or("—")
                        ))
                        .into_any_element(),
                ])
                .into_any_element(),
        ];
        children.push(self.card_title("Devices"));
        if self.capture.devices.is_empty() {
            children.push(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("No devices reported by the backend")
                    .into_any_element(),
            );
        } else {
            for device in &self.capture.devices {
                let selected = device.selected;
                let label = if device.resolution.is_empty() {
                    device.label.clone()
                } else {
                    format!("{} · {}", device.label, device.resolution)
                };
                let color = if selected { ACCENT } else { TEXT };
                let id = device.id.clone();
                children.push(
                    div()
                        .id(gpui::SharedString::from(format!("device-{id}")))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(if selected { ACCENT } else { BORDER }))
                        .text_color(rgb(color))
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.dispatch(
                                "/api/devices/select".into(),
                                Some(json!({"id": id.clone()})),
                            );
                            cx.notify();
                        }))
                        .child(label)
                        .into_any_element(),
                );
            }
        }
        children.push(self.card_title("Capture method"));
        children.extend(self.method_buttons(
            &self.capture.capture_methods,
            "/api/capture-methods/select",
            cx,
        ));
        children.push(self.card_title("Interaction method"));
        children.extend(self.method_buttons(
            &self.capture.interaction_methods,
            "/api/interaction-methods/select",
            cx,
        ));
        div()
            .id("capture-page")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .children(vec![
                div()
                    .w_full()
                    .bg(rgb(PANEL))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(children)
                    .into_any_element(),
                self.render_event_card(),
            ])
            .into_any_element()
    }

    fn method_buttons(
        &self,
        methods: &[crate::model::MethodOption],
        path: &str,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        if methods.is_empty() {
            return vec![div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child("No options available")
                .into_any_element()];
        }
        let path = path.to_owned();
        methods
            .iter()
            .map(|method| {
                let selected = method.selected;
                let id = method.id.clone();
                let method_path = path.clone();
                div()
                    .id(gpui::SharedString::from(format!("method-{}", method.id)))
                    .w_full()
                    .flex()
                    .items_center()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(if selected { ACCENT } else { BORDER }))
                    .bg(rgb(if selected { 0x203644 } else { PANEL_ALT }))
                    .text_color(rgb(if selected { ACCENT } else { TEXT }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.dispatch(method_path.clone(), Some(json!({"id": id.clone()})));
                        cx.notify();
                    }))
                    .child(method.label.clone())
                    .into_any_element()
            })
            .collect()
    }

    fn render_tasks(&self, cx: &Context<Self>, triggers_only: bool) -> AnyElement {
        let tasks = self
            .tasks
            .iter()
            .filter(|task| !triggers_only || task.trigger);
        let mut cards = Vec::new();
        for task in tasks {
            let name = task.name.clone();
            let encoded_name = crate::api::url_encode(&name);
            let action = if task.running {
                "stop"
            } else if task.paused {
                "resume"
            } else {
                "start"
            };
            let action_label = if task.running {
                "Stop"
            } else if task.paused {
                "Resume"
            } else {
                "Start"
            };
            let action_path = if action == "start" {
                format!("/api/tasks/{encoded_name}/start")
            } else {
                format!("/api/tasks/{encoded_name}/action")
            };
            let action_body = if action == "start" {
                None
            } else {
                Some(json!({"action": action}))
            };
            let mut controls = vec![self.action_button(
                &format!("task-{name}-{action}"),
                action_label,
                &action_path,
                action_body,
                cx,
            )];
            if task.enabled {
                controls.push(self.action_button(
                    &format!("task-{name}-disable"),
                    "Disable",
                    &format!("/api/tasks/{encoded_name}/action"),
                    Some(json!({"action":"disable"})),
                    cx,
                ));
            } else {
                controls.push(self.action_button(
                    &format!("task-{name}-enable"),
                    "Enable",
                    &format!("/api/tasks/{encoded_name}/action"),
                    Some(json!({"action":"enable"})),
                    cx,
                ));
            }
            let config = task
                .config
                .iter()
                .take(6)
                .map(|field| self.render_task_config_field(&encoded_name, field, cx))
                .collect::<Vec<_>>();
            let description = if task.description.is_empty() {
                "No description".to_owned()
            } else {
                task.description.clone()
            };
            cards.push(
                div()
                    .id(gpui::SharedString::from(format!("task-card-{name}")))
                    .w_full()
                    .bg(rgb(PANEL))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(vec![
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(task.name.clone())
                                    .child(
                                        div().text_sm().text_color(rgb(MUTED)).child(description),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(if task.running {
                                        SUCCESS
                                    } else if task.enabled {
                                        ACCENT
                                    } else {
                                        MUTED
                                    }))
                                    .child(task_state_text(task)),
                            )
                            .into_any_element(),
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(format!("Class: {}", task.class_name))
                            .into_any_element(),
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(controls)
                            .into_any_element(),
                        if config.is_empty() {
                            div()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("No configuration fields")
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .children(config)
                                .into_any_element()
                        },
                    ])
                    .into_any_element(),
            );
        }
        if cards.is_empty() {
            cards.push(
                div()
                    .text_color(rgb(MUTED))
                    .child("No tasks available")
                    .into_any_element(),
            );
        }
        div()
            .id(if triggers_only {
                "triggers-page"
            } else {
                "tasks-page"
            })
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .children(cards)
            .into_any_element()
    }

    fn render_settings(&self, cx: &Context<Self>) -> AnyElement {
        let groups = self
            .settings
            .iter()
            .map(|group| {
                let group_name = group.name.clone();
                let encoded_group_name = crate::api::url_encode(&group_name);
                let fields = group
                    .fields
                    .iter()
                    .map(|field| self.render_setting_field(&encoded_group_name, field, cx))
                    .collect::<Vec<_>>();
                div()
                    .id(gpui::SharedString::from(format!("settings-{}", group.name)))
                    .w_full()
                    .bg(rgb(PANEL))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(vec![
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(group.name.clone())
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(MUTED))
                                            .child(group.description.clone()),
                                    ),
                            )
                            .child(self.action_button(
                                &format!("reset-{}", group.name),
                                "Reset",
                                &format!("/api/settings/{encoded_group_name}/reset"),
                                None::<Value>,
                                cx,
                            ))
                            .into_any_element(),
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(fields)
                            .into_any_element(),
                    ])
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let children = if groups.is_empty() {
            vec![div()
                .text_color(rgb(MUTED))
                .child("No settings available")
                .into_any_element()]
        } else {
            groups
        };
        div()
            .id("settings-page")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .children(children)
            .into_any_element()
    }

    fn render_task_config_field(
        &self,
        task_name: &str,
        field: &TaskConfigField,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut children = vec![render_config_field(field)];
        if field.kind == "boolean" {
            let next_value = !field.value.as_bool().unwrap_or(false);
            children.push(self.action_button(
                &format!("task-config-{}-{}", task_name, field.key),
                if next_value { "Enable" } else { "Disable" },
                &format!("/api/tasks/{task_name}/config"),
                Some(json!({"key": field.key, "value": next_value})),
                cx,
            ));
        }
        div()
            .flex()
            .items_center()
            .gap_2()
            .children(children)
            .into_any_element()
    }

    fn render_setting_field(
        &self,
        group_name: &str,
        field: &TaskConfigField,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut children = vec![render_config_field(field)];
        if field.kind == "boolean" {
            let next_value = !field.value.as_bool().unwrap_or(false);
            children.push(self.action_button(
                &format!("setting-{}-{}", group_name, field.key),
                if next_value { "Enable" } else { "Disable" },
                &format!("/api/settings/{group_name}/config"),
                Some(json!({"key": field.key, "value": next_value})),
                cx,
            ));
        }
        div()
            .flex()
            .items_center()
            .gap_2()
            .children(children)
            .into_any_element()
    }

    fn render_about(&self) -> AnyElement {
        let about = &self.about;
        let about_title = if about.title.is_empty() {
            "ok-script".to_owned()
        } else {
            about.title.clone()
        };
        let about_text = if about.about.is_empty() {
            "Native GPUI shell backed by the shared ok-script Web API.".to_owned()
        } else {
            about.about.clone()
        };
        let projects = about
            .projects
            .iter()
            .map(|project| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .bg(rgb(PANEL_ALT))
                    .rounded_md()
                    .child(project.name.clone())
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(project.url.clone()),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("about-page")
            .size_full()
            .overflow_y_scroll()
            .p_5()
            .child(
                div()
                    .w_full()
                    .bg(rgb(PANEL))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(vec![
                        div().text_xl().child(about_title).into_any_element(),
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "Version {}",
                                if about.version.is_empty() {
                                    "—"
                                } else {
                                    &about.version
                                }
                            ))
                            .into_any_element(),
                        div()
                            .text_color(rgb(TEXT))
                            .child(about_text)
                            .into_any_element(),
                        if projects.is_empty() {
                            div()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("No project links")
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .children(projects)
                                .into_any_element()
                        },
                    ]),
            )
            .into_any_element()
    }

    fn render_scripts(&self, cx: &Context<Self>) -> AnyElement {
        let mut cards = vec![div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().flex().flex_col().gap_1().child("Scripts").child(
                div().text_sm().text_color(rgb(MUTED)).child(
                    "The native client uses the same script storage and execution API as Web.",
                ),
            ))
            .child(self.action_button(
                "script-record",
                "Start recording",
                "/api/scripts-record/start",
                None::<Value>,
                cx,
            ))
            .into_any_element()];
        for script in &self.scripts {
            let encoded = crate::api::url_encode(&script.name);
            cards.push(
                div()
                    .id(gpui::SharedString::from(format!("script-{}", script.name)))
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(PANEL_ALT))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .children(vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(script.name.clone())
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MUTED))
                                    .child(format!("modified {}", script.modified)),
                            )
                            .into_any_element(),
                        div()
                            .flex()
                            .gap_2()
                            .children(vec![
                                self.action_button(
                                    &format!("copy-script-{}", script.name),
                                    "Copy",
                                    &format!("/api/scripts/{encoded}/copy"),
                                    None::<Value>,
                                    cx,
                                ),
                                self.action_button(
                                    &format!("delete-script-{}", script.name),
                                    "Delete",
                                    &format!("/api/scripts/{encoded}/delete"),
                                    None::<Value>,
                                    cx,
                                ),
                            ])
                            .into_any_element(),
                    ])
                    .into_any_element(),
            );
        }
        if self.scripts.is_empty() {
            cards.push(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("No scripts available")
                    .into_any_element(),
            );
        }
        if !self.script_templates.is_empty() {
            cards.push(self.card_title("Script templates"));
            for template in &self.script_templates {
                cards.push(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(rgb(PANEL_ALT))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .children(vec![
                            div()
                                .child(if template.name.is_empty() {
                                    template.template_name.clone()
                                } else {
                                    template.name.clone()
                                })
                                .into_any_element(),
                            div()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child(format!("{} · {}", template.category, template.class_name))
                                .into_any_element(),
                            div()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child(template.doc.clone())
                                .into_any_element(),
                        ])
                        .into_any_element(),
                );
            }
        }
        div()
            .id("scripts-page")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .children(cards)
            .into_any_element()
    }

    fn render_templates(&self, cx: &Context<Self>) -> AnyElement {
        let mut cards = vec![div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div().flex().flex_col().gap_1().child("Templates").child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child("Template images and annotations are backed by the shared Web API."),
                ),
            )
            .child(self.action_button(
                "capture-template",
                "Capture template",
                "/api/templates/capture",
                None::<Value>,
                cx,
            ))
            .into_any_element()];
        for template in &self.templates {
            let encoded = crate::api::url_encode(&template.name);
            cards.push(
                div()
                    .id(gpui::SharedString::from(format!(
                        "template-{}",
                        template.name
                    )))
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(PANEL_ALT))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .children(vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(template.name.clone())
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MUTED))
                                    .child(template.categories.join(", ")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MUTED))
                                    .child(template.url.clone()),
                            )
                            .into_any_element(),
                        self.action_button(
                            &format!("delete-template-{}", template.name),
                            "Delete",
                            &format!("/api/templates/{encoded}/delete"),
                            None::<Value>,
                            cx,
                        ),
                    ])
                    .into_any_element(),
            );
        }
        if self.templates.is_empty() {
            cards.push(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("No templates available")
                    .into_any_element(),
            );
        }
        div()
            .id("templates-page")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .children(cards)
            .into_any_element()
    }

    fn render_schedule(&self, cx: &Context<Self>) -> AnyElement {
        let mut cards = vec![div()
            .flex()
            .flex_col()
            .gap_1()
            .child("Schedule")
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(format!("{} scheduled tasks", self.schedule.tasks.len())),
            )
            .into_any_element()];
        for task in &self.schedule.tasks {
            let encoded = crate::api::url_encode(&task.name);
            let toggle = if task.enabled { "disable" } else { "enable" };
            let toggle_label = if task.enabled { "Disable" } else { "Enable" };
            cards.push(
                div()
                    .id(gpui::SharedString::from(format!("schedule-{}", task.name)))
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(PANEL))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .children(vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(task.name.clone())
                            .child(div().text_sm().text_color(rgb(MUTED)).child(
                                if task.description.is_empty() {
                                    "No description".to_owned()
                                } else {
                                    task.description.clone()
                                },
                            ))
                            .child(div().text_sm().text_color(rgb(MUTED)).child(format!(
                                "{} · next {}",
                                task.trigger_type,
                                if task.next_run_time.is_empty() {
                                    "—"
                                } else {
                                    &task.next_run_time
                                }
                            )))
                            .into_any_element(),
                        div()
                            .flex()
                            .gap_2()
                            .children(vec![
                                self.action_button(
                                    &format!("schedule-toggle-{}", task.name),
                                    toggle_label,
                                    &format!("/api/schedule/{encoded}/action"),
                                    Some(json!({"action": toggle})),
                                    cx,
                                ),
                                self.action_button(
                                    &format!("schedule-delete-{}", task.name),
                                    "Delete",
                                    &format!("/api/schedule/{encoded}/action"),
                                    Some(json!({"action":"delete"})),
                                    cx,
                                ),
                            ])
                            .into_any_element(),
                    ])
                    .into_any_element(),
            );
        }
        if self.schedule.tasks.is_empty() {
            cards.push(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("No scheduled tasks")
                    .into_any_element(),
            );
        }
        div()
            .id("schedule-page")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .children(cards)
            .into_any_element()
    }

    fn render_custom_tab(&self, cx: &Context<Self>) -> AnyElement {
        let id = match &self.page {
            Page::Custom(id) => id,
            _ => return div().child("No custom tab").into_any_element(),
        };
        let tab = self.navigation.task_tabs.iter().find(|tab| &tab.id == id);
        let body = tab.and_then(|tab| tab.gpui_view.as_ref()).map(|value| self.render_control_node(id, value, cx)).unwrap_or_else(|| div().text_sm().text_color(rgb(MUTED)).child("This task tab exposes Web assets only. Add gpui_view to render a native control tree.").into_any_element());
        let tab_title = tab
            .map(|tab| tab.name.clone())
            .unwrap_or_else(|| id.clone());
        div()
            .id("custom-tab-page")
            .size_full()
            .p_5()
            .child(
                div()
                    .w_full()
                    .bg(rgb(PANEL))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(vec![
                        div().text_xl().child(tab_title).into_any_element(),
                        body,
                    ]),
            )
            .into_any_element()
    }

    fn render_control_node(&self, tab_id: &str, node: &Value, cx: &Context<Self>) -> AnyElement {
        if let Some(nodes) = node.as_array() {
            return div()
                .flex()
                .flex_col()
                .gap_2()
                .children(
                    nodes
                        .iter()
                        .map(|node| self.render_control_node(tab_id, node, cx))
                        .collect::<Vec<_>>(),
                )
                .into_any_element();
        }
        let Some(object) = node.as_object() else {
            return div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(display_value(node))
                .into_any_element();
        };
        let kind = object.get("type").and_then(Value::as_str).unwrap_or("text");
        let text = object
            .get("text")
            .or_else(|| object.get("label"))
            .or_else(|| object.get("title"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let children_value = object.get("children").or_else(|| object.get("controls"));
        let children = children_value
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .map(|node| self.render_control_node(tab_id, node, cx))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match kind {
            "button" => {
                let action = object.get("action").and_then(Value::as_object);
                let operation = action
                    .and_then(|action| action.get("operation"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let channel = action
                    .and_then(|action| action.get("kind"))
                    .and_then(Value::as_str)
                    .unwrap_or("action");
                let body = action
                    .and_then(|action| action.get("body"))
                    .cloned()
                    .or_else(|| object.get("body").cloned());
                if operation.is_empty() {
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(PANEL_ALT))
                        .text_color(rgb(MUTED))
                        .child(text)
                        .into_any_element()
                } else {
                    self.action_button(
                        &format!("tab-{tab_id}-{operation}"),
                        &text,
                        &format!(
                            "/api/task-tabs/{}/{}/{}",
                            crate::api::url_encode(tab_id),
                            channel,
                            crate::api::url_encode(operation)
                        ),
                        body,
                        cx,
                    )
                }
            }
            "heading" | "title" => div()
                .text_lg()
                .text_color(rgb(TEXT))
                .child(text)
                .into_any_element(),
            "separator" => div().h(px(1.)).w_full().bg(rgb(BORDER)).into_any_element(),
            "row" => div()
                .flex()
                .gap_2()
                .items_center()
                .children(children)
                .into_any_element(),
            "column" | "stack" | "group" | "panel" => div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(rgb(if kind == "panel" { PANEL_ALT } else { PANEL }))
                .children(if text.is_empty() {
                    children
                } else {
                    let mut result = vec![div().child(text).into_any_element()];
                    result.extend(children);
                    result
                })
                .into_any_element(),
            _ => div()
                .text_sm()
                .text_color(rgb(if kind == "muted" { MUTED } else { TEXT }))
                .child(if text.is_empty() {
                    node.to_string()
                } else {
                    text
                })
                .into_any_element(),
        }
    }

    fn render_event_card(&self) -> AnyElement {
        let events = self
            .event_history
            .iter()
            .rev()
            .take(12)
            .map(|event| {
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(event.clone())
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .w_full()
            .bg(rgb(PANEL))
            .rounded_lg()
            .border_1()
            .border_color(rgb(BORDER))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .children(if events.is_empty() {
                vec![div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("Waiting for runtime events…")
                    .into_any_element()]
            } else {
                events
            })
            .into_any_element()
    }

    fn card_title(&self, title: &str) -> AnyElement {
        div()
            .text_lg()
            .text_color(rgb(TEXT))
            .child(title.to_owned())
            .into_any_element()
    }

    fn render_content(&self, cx: &Context<Self>) -> AnyElement {
        match &self.page {
            Page::Capture => self.render_capture(cx),
            Page::Triggers => self.render_tasks(cx, true),
            Page::Tasks => self.render_tasks(cx, false),
            Page::Settings => self.render_settings(cx),
            Page::About => self.render_about(),
            Page::Custom(_) => self.render_custom_tab(cx),
            Page::Script => self.render_scripts(cx),
            Page::Templates => self.render_templates(cx),
            Page::Schedule => self.render_schedule(cx),
        }
    }
}

impl Render for GpuiView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if self.capture.title.is_empty() {
            "ok-script"
        } else {
            self.capture.title.as_str()
        };
        if self.window_title != title {
            window.set_window_title(title);
            self.window_title = title.to_owned();
        }
        let nav = self
            .nav_items()
            .into_iter()
            .map(|(page, label)| self.nav_button(page, &label, cx))
            .collect::<Vec<_>>();
        let notice = self.notice.as_ref().map(|(ok, message)| {
            div()
                .w_full()
                .px_4()
                .py_3()
                .rounded_md()
                .bg(rgb(if *ok { 0x1e3a2a } else { 0x3b2428 }))
                .text_color(rgb(if *ok { SUCCESS } else { DANGER }))
                .child(message.clone())
                .into_any_element()
        });
        let sidebar_width = if self.min_width < 1100.0 {
            196.0
        } else {
            228.0
        };
        div()
            .id("root")
            .size_full()
            .flex()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .id("sidebar")
                    .w(px(sidebar_width))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(rgb(SIDEBAR))
                    .border_r_1()
                    .border_color(rgb(BORDER))
                    .children(vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .child("ok-script")
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MUTED))
                                    .child("GPUI native UI"),
                            )
                            .into_any_element(),
                        div().h(px(1.)).w_full().bg(rgb(BORDER)).into_any_element(),
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(nav)
                            .into_any_element(),
                        div().flex_1().into_any_element(),
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "min {}×{}",
                                self.min_width as u32, self.min_height as u32
                            ))
                            .into_any_element(),
                    ]),
            )
            .child(
                div()
                    .id("main")
                    .size_full()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .children(vec![
                        self.render_header(cx),
                        if let Some(notice) = notice {
                            notice
                        } else {
                            div().h(px(0.)).into_any_element()
                        },
                        self.render_content(cx),
                    ]),
            )
    }
}

fn render_config_field(field: &TaskConfigField) -> AnyElement {
    let value = display_value(&field.value);
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .p_2()
        .rounded_md()
        .bg(rgb(PANEL_ALT))
        .children(vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .child(field.key.clone())
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child(field.description.clone()),
                )
                .into_any_element(),
            div()
                .text_sm()
                .text_color(rgb(ACCENT))
                .child(value)
                .into_any_element(),
        ])
        .into_any_element()
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => "—".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn status_text(status: &crate::model::ExecutorStatus) -> String {
    if status.starting {
        "starting".into()
    } else if status.running && status.paused {
        "paused".into()
    } else if status.running {
        "running".into()
    } else {
        "idle".into()
    }
}

fn task_state_text(task: &AutomationTask) -> String {
    if task.running {
        "running".into()
    } else if task.paused {
        "paused".into()
    } else if task.enabled {
        "enabled".into()
    } else {
        "disabled".into()
    }
}
