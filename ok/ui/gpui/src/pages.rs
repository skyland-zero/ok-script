//! Page rendering. Structure and copy follow the web frontend's `App.tsx`;
//! sections are named after the web components they mirror.

use gpui::{
    div, img, prelude::*, px, AnyElement, Context, ElementId, FontWeight, IntoElement, ObjectFit,
    ParentElement, SharedString, Styled, Window,
};
use gpui_component::{
    input::Input, ActiveTheme as _, Disableable as _, StyledExt as _,
};
use serde_json::{json, Value};

use crate::api;
use crate::app::{ConfigTarget, ConfirmAction, Modal, OkApp, TaskFilter};
use crate::components as ui;
use crate::i18n::{self, t};
use crate::icons::OkIcon;
use crate::model::*;
use crate::state::elapsed_text;
use crate::theme::Tokens;

impl OkApp {
    // ------------------------------------------------------------- Capture page

    pub fn render_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.state.capture.clone();
        let busy = self.state.is_busy();
        let status = session.status.clone();

        // (a) identity card
        let identity = ui::card(cx)
            .p(px(16.0))
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_3()
                            .min_w(px(0.0))
                            .child(self.app_avatar(42.0, 15.0, cx))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.0))
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_size(px(ui::FS_IDENTITY))
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .child(if session.title.is_empty() {
                                                "ok-script".to_owned()
                                            } else {
                                                session.title.clone()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(ui::FS_SMALL))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{} · {}",
                                                if session.version.is_empty() {
                                                    "dev".to_owned()
                                                } else {
                                                    session.version.clone()
                                                },
                                                if session.debug {
                                                    t("Debug")
                                                } else {
                                                    t("Release")
                                                }
                                            )),
                                    ),
                            ),
                    )
                    .child(
                        ui::toolbar(cx).flex_none().children(vec![
                            ui::secondary_button("capture-tool", t("Capture"), cx)
                                .icon(OkIcon::Capture.icon())
                                .disabled(busy)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.post("/api/tools/capture", None, cx);
                                }))
                                .into_any_element(),
                            ui::secondary_button(
                                "capture-refresh",
                                if self.state.pending("/api/devices/refresh") {
                                    t("Refreshing")
                                } else {
                                    t("Refresh")
                                },
                                cx,
                            )
                            .icon(OkIcon::Refresh.icon())
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.post("/api/devices/refresh", None, cx);
                            }))
                            .into_any_element(),
                            self.start_pause_button(&status, busy, cx),
                            ui::secondary_button("capture-stop-task", t("Stop task"), cx)
                                .icon(OkIcon::Stop.icon())
                                .disabled(busy || !status.running)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.post("/api/executor/stop-task", None, cx);
                                }))
                                .into_any_element(),
                        ]),
                    ),
            );

        // (b) three selector columns
        let search = self.ensure_input(
            window,
            cx,
            "device-search",
            &t("Search title or exe..."),
            "",
            false,
        );
        let query = self.device_query.to_lowercase();
        let devices: Vec<DeviceOption> = session
            .devices
            .iter()
            .filter(|device| {
                query.is_empty()
                    || device.label.to_lowercase().contains(&query)
                    || device.keywords.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        let mut window_column = div()
            .v_flex()
            .flex_grow().flex_basis(gpui::relative(0.5))
            .min_w(px(0.0))
            .gap(px(9.0))
            .child(ui::section_title(t("Choose Window"), cx))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .h(px(ui::DEVICE_SEARCH_HEIGHT))
                    .px(px(14.0))
                    .rounded(px(7.0))
                    .bg(ui::button_surface(cx))
                    .child(
                        OkIcon::Search
                            .icon()
                            .size(px(17.0))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().flex_1().child(Input::new(&search))),
            );
        if devices.is_empty() {
            window_column = window_column.child(
                div()
                    .text_size(px(ui::FS_SMALL))
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.state.loaded {
                        t("No windows found")
                    } else {
                        t("Loading")
                    }),
            );
        } else {
            let rows: Vec<AnyElement> = devices
                .iter()
                .map(|device| {
                    let id = device.id.clone();
                    ui::option_row(
                        ElementId::Name(SharedString::from(format!("device-{}", device.id))),
                        device.label.clone(),
                        if device.connected {
                            None
                        } else {
                            Some(t("Disconnected").into())
                        },
                        device.selected,
                        busy,
                        cx,
                        cx.listener(move |view, _, _, cx| {
                            view.post(
                                "/api/devices/select",
                                Some(json!({ "id": id.clone() })),
                                cx,
                            );
                        }),
                    )
                })
                .collect();
            window_column = window_column.child(
                div()
                    .v_flex()
                    .id("device-list")
                    .flex_1()
                    .min_h(px(0.0))
                    .gap(px(2.0))
                    .overflow_y_scroll()
                    .children(rows),
            );
        }

        let capture_column = self.method_column(
            "Capture Method",
            &session.capture_methods,
            "/api/capture-methods/select",
            busy,
            cx,
        );
        let interaction_column = self.method_column(
            "Choose Interaction",
            &session.interaction_methods,
            "/api/interaction-methods/select",
            busy,
            cx,
        );

        // (c) debug tools
        let mut tools = ui::toolbar(cx);
        for (label, icon, action) in [
            ("Export Logs", OkIcon::DocumentText, "export-logs"),
            ("Install Folder", OkIcon::Folder, "install-folder"),
            ("Screenshot Folder", OkIcon::Folder, "screenshot-folder"),
            ("Log Folder", OkIcon::Folder, "log-folder"),
        ] {
            let path = format!("/api/tools/{action}");
            tools = tools.child(
                ui::labelled_button(
                    ElementId::Name(SharedString::from(format!("tool-{action}"))),
                    t(label),
                    icon,
                    cx,
                )
                .disabled(busy)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.post(&path, None, cx);
                })),
            );
        }
        tools = tools
            .child(
                ui::labelled_button("tool-view-log", t("View Log"), OkIcon::DeveloperBoard, cx)
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.modal = Some(Modal::Logs);
                        view.last_log_poll =
                            std::time::Instant::now() - std::time::Duration::from_secs(1);
                        cx.notify();
                    })),
            )
            .child(
                ui::labelled_button("tool-ocr", "OCR", OkIcon::Search, cx)
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.post("/api/tools/ocr", None, cx);
                    })),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(ui::switch_control(
                        "overlay-boxes",
                        session.overlay.boxes,
                        busy,
                        cx,
                        cx.listener(|view, checked, _, cx| {
                            view.post(
                                "/api/overlay",
                                Some(json!({ "name": "boxes", "value": checked })),
                                cx,
                            );
                        }),
                    ))
                    .child(div().text_size(px(ui::FS_SMALL)).child(t("Enable Boxes"))),
            );

        ui::page_root("capture-page")
            .child(identity)
            .child(
                div()
                    .h_flex()
                    .gap(px(12.0))
                    .items_start()
                    .children(vec![
                        window_column.into_any_element(),
                        capture_column,
                        interaction_column,
                    ]),
            )
            .child(ui::section_title(t("Debug"), cx))
            .child(ui::card(cx).p(px(16.0)).child(tools))
            .into_any_element()
    }

    /// `.app-avatar`: the application icon when the backend serves one,
    /// otherwise the `OK` fallback badge.
    pub fn app_avatar(&mut self, size: f32, font_size: f32, cx: &gpui::App) -> AnyElement {
        let icon_url = self
            .state
            .capture
            .icon_url
            .clone()
            .or_else(|| self.state.about.icon_url.clone());
        if let Some(url) = icon_url {
            let absolute = self.client.url(&url);
            self.ensure_image(&absolute);
            if let Some(image) = self.images.get(&absolute).cloned() {
                return div()
                    .size(px(size))
                    .flex_none()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(cx.theme().accent)
                    .child(img(image).size_full().object_fit(ObjectFit::Contain))
                    .into_any_element();
            }
        }
        div()
            .grid()
            .items_center()
            .justify_center()
            .size(px(size))
            .flex_none()
            .rounded_full()
            .bg(cx.theme().accent)
            .text_color(gpui::rgb(0x102a35))
            .text_size(px(font_size))
            .font_weight(FontWeight::BOLD)
            .child("OK")
            .into_any_element()
    }

    fn start_pause_button(
        &self,
        status: &ExecutorStatus,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hotkey = status
            .hotkey
            .clone()
            .filter(|value| !value.is_empty())
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        let (label, icon, path) = if status.starting {
            (t("Starting"), OkIcon::Refresh, None)
        } else if status.paused {
            (
                format!("{}{hotkey}", t("Start")),
                OkIcon::Play,
                Some("/api/executor/resume"),
            )
        } else {
            (t("Pause"), OkIcon::Pause, Some("/api/executor/pause"))
        };
        let mut button = ui::primary_button("capture-start-pause", label)
            .icon(icon.icon())
            .disabled(busy || status.starting);
        if let Some(path) = path {
            button = button.on_click(cx.listener(move |view, _, _, cx| {
                view.post(path, None, cx);
            }));
        }
        button.into_any_element()
    }

    fn method_column(
        &self,
        title: &str,
        methods: &[MethodOption],
        path: &str,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut column = div()
            .v_flex()
            .flex_grow().flex_basis(gpui::relative(0.25))
            .min_w(px(0.0))
            .gap(px(9.0))
            .child(ui::section_title(t(title), cx));
        if methods.is_empty() {
            column = column.child(
                div()
                    .text_size(px(ui::FS_SMALL))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("No options available")),
            );
        } else {
            let path = path.to_owned();
            let rows: Vec<AnyElement> = methods
                .iter()
                .map(|method| {
                    let id = method.id.clone();
                    let target = path.clone();
                    ui::option_row(
                        ElementId::Name(SharedString::from(format!(
                            "method-{}-{}",
                            title, method.id
                        ))),
                        method.label.clone(),
                        None,
                        method.selected,
                        busy,
                        cx,
                        cx.listener(move |view, _, _, cx| {
                            view.post(&target, Some(json!({ "id": id.clone() })), cx);
                        }),
                    )
                })
                .collect();
            column = column.child(
                div()
                    .v_flex()
                    .id(SharedString::from(format!("method-list-{title}")))
                    .flex_1()
                    .min_h(px(0.0))
                    .max_h(px(420.0))
                    .gap(px(2.0))
                    .overflow_y_scroll()
                    .children(rows),
            );
        }
        column.into_any_element()
    }

    // --------------------------------------------------------------- task list

    pub fn render_task_list(
        &mut self,
        filter: TaskFilter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tasks: Vec<AutomationTask> = self
            .state
            .visible_tasks()
            .into_iter()
            .filter(|task| match &filter {
                TaskFilter::Triggers => task.trigger,
                TaskFilter::Tasks => !task.trigger && task.group_name.is_none(),
                TaskFilter::Group(name) => {
                    !task.trigger && task.group_name.as_deref() == Some(name.as_str())
                }
            })
            .cloned()
            .collect();

        let title = match &filter {
            TaskFilter::Triggers => t("Triggers"),
            TaskFilter::Tasks => t("Tasks"),
            TaskFilter::Group(name) => name.clone(),
        };
        let _ = title;
        let mut page = ui::page_root("task-page");
        if tasks.is_empty() {
            page = page.child(ui::muted_text(
                if self.state.loaded {
                    i18n::tv("{count} available", &[("count", "0")])
                } else {
                    t("Loading")
                },
                cx,
            ));
        }
        for task in &tasks {
            page = page.child(self.render_task_card(task, window, cx));
        }
        page.into_any_element()
    }

    fn render_task_card(
        &mut self,
        task: &AutomationTask,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = task.name.clone();
        let encoded = api::url_encode(&name);
        let busy = self.state.is_busy();
        let expanded = self.state.expanded_tasks.contains(&name);
        let running = task.running || task.paused;

        let summary_secondary = if running {
            format!(
                "{} · {}: {}",
                if task.paused {
                    t("Paused")
                } else {
                    t("Running")
                },
                t("Time Elapsed"),
                elapsed_text(task.start_time)
            )
        } else {
            task.description.clone()
        };

        let summary = div()
            .id(ElementId::Name(SharedString::from(format!(
                "task-summary-{name}"
            ))))
            .h_flex()
            .flex_1()
            .min_w(px(0.0))
            .items_center()
            .gap_2()
            .cursor_pointer()
            .on_click(cx.listener({
                let name = name.clone();
                move |view, _, _, cx| {
                    if view.state.expanded_tasks.contains(&name) {
                        view.state.expanded_tasks.remove(&name);
                    } else {
                        view.state.expanded_tasks.insert(name.clone());
                    }
                    cx.notify();
                }
            }))
            .child(
                div()
                    .text_size(px(ui::FS_CARD))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(px(ui::FS_SMALL))
                    .text_color(if running {
                        cx.theme().accent
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(summary_secondary),
            );

        let mut actions = div().h_flex().items_center().gap_2().flex_none();
        if let Some(waiting) = &task.waiting_for {
            actions = actions.child(
                div()
                    .max_w(px(360.0))
                    .text_size(px(ui::FS_SMALL))
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::tv(
                        "Waiting for {task_name} task to be completed",
                        &[("task_name", waiting)],
                    )),
            );
        }
        if task.instructions.is_some() {
            actions = actions.child(
                ui::labelled_button(
                    ElementId::Name(SharedString::from(format!("task-info-{name}"))),
                    t("Instructions"),
                    OkIcon::Info,
                    cx,
                )
                .disabled(busy)
                .on_click(cx.listener({
                    let task = task.clone();
                    move |view, _, _, cx| {
                        view.modal = Some(Modal::Instructions {
                            title: task.name.clone(),
                            text: task.instructions.clone().unwrap_or_default(),
                        });
                        cx.notify();
                    }
                })),
            );
        }
        if task.trigger {
            actions = actions
                .child(ui::switch_control(
                    ElementId::Name(SharedString::from(format!("task-enable-{name}"))),
                    task.enabled,
                    busy,
                    cx,
                    cx.listener({
                        let path = format!("/api/tasks/{encoded}/action");
                        move |view, checked, _, cx| {
                            let action = if *checked { "enable" } else { "disable" };
                            view.post_action(&path, action, cx);
                        }
                    }),
                ))
                .child(
                    div()
                        .text_size(px(ui::FS_BODY))
                        .child(if task.enabled {
                            t("Enabled")
                        } else {
                            t("Disabled")
                        }),
                );
        } else {
            if task.enabled && task.running && !task.paused {
                actions = actions.child(
                    ui::secondary_button(
                        ElementId::Name(SharedString::from(format!("task-pause-{name}"))),
                        t("Pause"),
                        cx,
                    )
                    .icon(OkIcon::Pause.icon())
                    .disabled(busy)
                    .on_click(cx.listener({
                        let path = format!("/api/tasks/{encoded}/action");
                        move |view, _, _, cx| view.post_action(&path, "pause", cx)
                    })),
                );
            }
            if task.enabled {
                actions = actions.child(
                    ui::secondary_button(
                        ElementId::Name(SharedString::from(format!("task-stop-{name}"))),
                        t("Stop"),
                        cx,
                    )
                    .icon(OkIcon::Stop.icon())
                    .disabled(busy)
                    .on_click(cx.listener({
                        let path = format!("/api/tasks/{encoded}/action");
                        move |view, _, _, cx| view.post_action(&path, "stop", cx)
                    })),
                );
            }
            if !task.enabled || task.paused {
                let paused = task.paused;
                actions = actions.child(
                    ui::secondary_button(
                        ElementId::Name(SharedString::from(format!("task-start-{name}"))),
                        if paused { t("Resume") } else { t("Start") },
                        cx,
                    )
                    .icon(OkIcon::Play.icon())
                    .disabled(busy)
                    .on_click(cx.listener({
                        let name = name.clone();
                        let path = format!("/api/tasks/{encoded}");
                        move |view, _, _, cx| {
                            if paused {
                                view.post_action(&format!("{path}/action"), "resume", cx);
                            } else {
                                view.post(&format!("{path}/start"), None, cx);
                            }
                        }
                    })),
                );
            }
        }
        if !task.config.is_empty() {
            actions = actions.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "task-expand-{name}"
                    ))))
                    .grid()
                    .items_center().justify_center()
                    .size(px(32.0))
                    .rounded(px(ui::BUTTON_RADIUS))
                    .cursor_pointer()
                    .hover(|style| style.bg(gpui::rgba(0xffffff0e)))
                    .on_click(cx.listener({
                        let name = name.clone();
                        move |view, _, _, cx| {
                            if view.state.expanded_tasks.contains(&name) {
                                view.state.expanded_tasks.remove(&name);
                            } else {
                                view.state.expanded_tasks.insert(name.clone());
                            }
                            cx.notify();
                        }
                    }))
                    .child(
                        OkIcon::ChevronDown
                            .icon()
                            .size(px(17.0))
                            .text_color(cx.theme().muted_foreground),
                    ),
            );
        }

        let mut card = ui::card(cx)
            .gap(px(6.0))
            .p(px(14.0))
            .when(running, |this| {
                this.border_1().border_color(cx.theme().accent)
            })
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(summary)
                    .child(actions),
            );

        if expanded {
            let target = ConfigTarget::Task(name.clone());
            let mut rows = div().v_flex().w_full();
            for field in &task.config {
                rows = rows.child(self.render_config_row(&target, field, window, cx));
            }
            card = card.child(
                div()
                    .v_flex()
                    .w_full()
                    .gap_2()
                    .pt(px(6.0))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(rows)
                    .child(
                        div()
                            .h_flex()
                            .justify_end()
                            .pt(px(4.0))
                            .child(
                                ui::secondary_button(
                                    ElementId::Name(SharedString::from(format!(
                                        "task-reset-{name}"
                                    ))),
                                    t("Reset Config"),
                                    cx,
                                )
                                .disabled(busy)
                                .on_click(cx.listener({
                                    let target = ConfigTarget::Task(name.clone());
                                    move |view, _, _, cx| view.reset_config(&target, cx)
                                })),
                            ),
                    ),
            );
        }
        card.into_any_element()
    }

    // --------------------------------------------------------- config controls

    pub fn render_config_row(
        &mut self,
        target: &ConfigTarget,
        field: &TaskConfigField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = self.render_config_control(target, field, window, cx);
        ui::config_row(
            field.key.clone(),
            field.description.clone(),
            control,
            field.sub_config,
            cx,
        )
    }

    pub fn render_config_control(
        &mut self,
        target: &ConfigTarget,
        field: &TaskConfigField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.state.is_busy();
        let (kind_name, target_name) = match target {
            ConfigTarget::Task(name) => ("task", name.clone()),
            ConfigTarget::Settings(name) => ("settings", name.clone()),
        };
        let input_key = format!("config:{kind_name}:{target_name}:{}", field.key);
        match field.kind.as_str() {
            "boolean" => {
                let checked = field.value.as_bool().unwrap_or(false);
                let target = target.clone();
                let key = field.key.clone();
                ui::switch_control(
                    ElementId::Name(SharedString::from(format!("cfg-{input_key}"))),
                    checked,
                    busy,
                    cx,
                    cx.listener(move |view, checked, _, cx| {
                        view.set_config(&target, &key, Value::from(*checked), cx);
                    }),
                )
            }
            "select" => {
                let options: Vec<Value> = field.options.clone().unwrap_or_default();
                let current = match &field.value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                let open = self.open_select.as_deref() == Some(input_key.as_str());
                let target = target.clone();
                let key = field.key.clone();
                let mut column = div().v_flex().gap_1().items_end();
                column = column.child(
                    ui::secondary_button(
                        ElementId::Name(SharedString::from(format!("select-{input_key}"))),
                        current.clone(),
                        cx,
                    )
                    .icon(OkIcon::ChevronDown.icon())
                    .disabled(busy)
                    .on_click(cx.listener({
                        let input_key = input_key.clone();
                        move |view, _, _, cx| {
                            view.open_select = if view.open_select.as_deref()
                                == Some(input_key.as_str())
                            {
                                None
                            } else {
                                Some(input_key.clone())
                            };
                            cx.notify();
                        }
                    })),
                );
                if open {
                    let mut popup = div()
                        .v_flex()
                        .min_w(px(180.0))
                        .max_h(px(240.0))
                        .gap(px(1.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .p(px(4.0))
                        .id(SharedString::from(format!("popup-{input_key}")))
                        .overflow_y_scroll();
                    for option in options {
                        let label = match &option {
                            Value::String(text) => text.clone(),
                            other => other.to_string(),
                        };
                        let selected = label == current;
                        let target = target.clone();
                        let key = field.key.clone();
                        let value = option.clone();
                        popup = popup.child(
                            div()
                                .id(ElementId::Name(SharedString::from(format!(
                                    "opt-{input_key}-{label}"
                                ))))
                                .w_full()
                                .px(px(10.0))
                                .py(px(6.0))
                                .rounded(px(5.0))
                                .text_size(px(ui::FS_SMALL))
                                .when(selected, |this| this.bg(cx.theme().list_active))
                                .cursor_pointer()
                                .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.open_select = None;
                                    view.set_config(&target, &key, value.clone(), cx);
                                }))
                                .child(label),
                        );
                    }
                    column = column.child(popup);
                }
                column.into_any_element()
            }
            "multi_selection" => {
                let options: Vec<Value> = field.options.clone().unwrap_or_default();
                let selected: Vec<Value> = field
                    .value
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let target = target.clone();
                let key = field.key.clone();
                let mut group = div().h_flex().gap_3().flex_wrap();
                for option in options {
                    let label = match &option {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    };
                    let is_selected = selected.contains(&option);
                    let target = target.clone();
                    let key = field.key.clone();
                    let value = option.clone();
                    let current = selected.clone();
                    group = group.child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(ui::FS_SMALL))
                            .child(
                                gpui_component::checkbox::Checkbox::new(ElementId::Name(
                                    SharedString::from(format!(
                                        "multi-{input_key}-{label}"
                                    )),
                                ))
                                .checked(is_selected)
                                .disabled(busy)
                                .on_click(cx.listener(move |view, checked, _, cx| {
                                    let mut next = current.clone();
                                    if *checked {
                                        next.push(value.clone());
                                    } else {
                                        next.retain(|item| item != &value);
                                    }
                                    view.set_config(&target, &key, Value::Array(next), cx);
                                })),
                            )
                            .child(label),
                    );
                }
                group.into_any_element()
            }
            "list" => {
                let target = target.clone();
                let field = field.clone();
                let selected: Vec<Value> = field.value.as_array().cloned().unwrap_or_default();
                let summary = selected
                    .iter()
                    .map(|item| match item {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                ui::secondary_button(
                    ElementId::Name(SharedString::from(format!("list-{input_key}"))),
                    if summary.is_empty() {
                        t("None")
                    } else {
                        summary
                    },
                    cx,
                )
                .icon(OkIcon::Edit.icon())
                .disabled(busy)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.modal = Some(Modal::ListEditor {
                        target: target.clone(),
                        field: field.clone(),
                        selected: selected.clone(),
                        active: None,
                        draft: String::new(),
                    });
                    cx.notify();
                }))
                .into_any_element()
            }
            "integer" | "number" => {
                let step = if field.kind == "integer" { 1.0 } else { 0.1 };
                let value = field.value.as_f64().unwrap_or(0.0);
                let minimum = field.minimum.unwrap_or(f64::MIN);
                let maximum = field.maximum.unwrap_or(f64::MAX);
                let target = target.clone();
                let target_plus = target.clone();
                let key = field.key.clone();
                let key_plus = field.key.clone();
                let display = if field.kind == "integer" {
                    format!("{}", value as i64)
                } else {
                    format!("{value}")
                };
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        ui::icon_button(
                            ElementId::Name(SharedString::from(format!(
                                "spin-down-{input_key}"
                            ))),
                            OkIcon::Subtract,
                            cx,
                        )
                        .disabled(busy || value <= minimum)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            let next = (value - step).max(minimum);
                            let value = if target_is_integer(&target) {
                                Value::from(next as i64)
                            } else {
                                Value::from(next)
                            };
                            view.set_config(&target, &key, value, cx);
                        })),
                    )
                    .child(
                        div()
                            .min_w(px(48.0))
                            .text_size(px(ui::FS_BODY))
                            .text_color(cx.theme().accent)
                            .child(display),
                    )
                    .child(
                        ui::icon_button(
                            ElementId::Name(SharedString::from(format!(
                                "spin-up-{input_key}"
                            ))),
                            OkIcon::Add,
                            cx,
                        )
                        .disabled(busy || value >= maximum)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            let next = (value + step).min(maximum);
                            let value = if target_is_integer(&target_plus) {
                                Value::from(next as i64)
                            } else {
                                Value::from(next)
                            };
                            view.set_config(&target_plus, &key_plus, value, cx);
                        })),
                    )
                    .into_any_element()
            }
            "multiline" => {
                let state = self.ensure_input(
                    window,
                    cx,
                    &input_key,
                    &field.description,
                    &field.value.as_str().unwrap_or_default().to_owned(),
                    true,
                );
                div()
                    .w(px(360.0))
                    .min_h(px(88.0))
                    .child(Input::new(&state).disabled(busy))
                    .into_any_element()
            }
            "text" | "file" => {
                let state = self.ensure_input(
                    window,
                    cx,
                    &input_key,
                    &field.description,
                    &field.value.as_str().unwrap_or_default().to_owned(),
                    false,
                );
                div()
                    .w(px(280.0))
                    .child(Input::new(&state).disabled(busy))
                    .into_any_element()
            }
            _ => {
                let state = self.ensure_input(
                    window,
                    cx,
                    &input_key,
                    &field.description,
                    &field.value.as_str().unwrap_or_default().to_owned(),
                    false,
                );
                div()
                    .w(px(280.0))
                    .child(Input::new(&state).disabled(busy))
                    .into_any_element()
            }
        }
    }

    // ---------------------------------------------------------------- settings

    pub fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut page = ui::page_root("settings-page")
            .child(ui::page_title(t("Settings"), cx));

        // App Config card: theme + language (web keeps these in localStorage).
        let theme_value = match self.prefs.theme.as_str() {
            "Light" => t("Light"),
            "Dark" => t("Dark"),
            _ => t("Use system setting"),
        };
        let language_value = i18n::language_label(&self.prefs.language).to_owned();
        page = page.child(
            ui::card(cx)
                .gap_2()
                .p(px(16.0))
                .child(ui::section_title(t("App Config"), cx))
                .child(ui::config_row(
                    t("Application Theme"),
                    t("Change the appearance of the application"),
                    self.choice_control(
                        "theme",
                        &theme_value,
                        &["Light", "Dark", "Auto"],
                        cx,
                        |view, value, cx| view.set_theme(&value, cx),
                    ),
                    false,
                    cx,
                ))
                .child(ui::config_row(
                    t("Language"),
                    t("Set your preferred language"),
                    self.choice_control(
                        "language",
                        &language_value,
                        &i18n::LOCALES,
                        cx,
                        |view, value, cx| view.set_language(&value, cx),
                    ),
                    false,
                    cx,
                )),
        );

        let groups: Vec<SettingsGroup> = self
            .state
            .settings
            .iter()
            .filter(|group| !group.top_level)
            .cloned()
            .collect();
        if groups.is_empty() {
            page = page.child(ui::muted_text(t("Loading"), cx));
        }
        for group in groups {
            let expanded = self.state.expanded_groups.contains(&group.name)
                || (group.expanded && !self.state.expanded_groups.contains(&format!("closed:{}", group.name)));
            let name = group.name.clone();
            let target = ConfigTarget::Settings(name.clone());
            let busy = self.state.is_busy();
            let mut card = ui::card(cx).gap_2().p(px(16.0)).child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "group-{name}"
                            ))))
                            .h_flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener({
                                let name = name.clone();
                                let expanded = expanded;
                                move |view, _, _, cx| {
                                    if expanded {
                                        view.state.expanded_groups.remove(&name);
                                        view.state
                                            .expanded_groups
                                            .insert(format!("closed:{name}"));
                                    } else {
                                        view.state
                                            .expanded_groups
                                            .remove(&format!("closed:{name}"));
                                        view.state.expanded_groups.insert(name.clone());
                                    }
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .text_size(px(ui::FS_CARD))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(i18n::t(&group.name)),
                            )
                            .child(
                                div()
                                    .text_size(px(ui::FS_SMALL))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(i18n::t(&group.description)),
                            )
                            .child(
                                OkIcon::ChevronDown
                                    .icon()
                                    .size(px(17.0))
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        ui::secondary_button(
                            ElementId::Name(SharedString::from(format!(
                                "settings-reset-{name}"
                            ))),
                            t("Reset Config"),
                            cx,
                        )
                        .disabled(busy)
                        .on_click(cx.listener({
                            let target = ConfigTarget::Settings(name.clone());
                            move |view, _, _, cx| view.reset_config(&target, cx)
                        })),
                    ),
            );
            if expanded {
                for field in &group.fields {
                    card = card.child(self.render_config_row(&target, field, window, cx));
                }
            }
            page = page.child(card);
        }
        page.into_any_element()
    }

    pub fn render_top_level_group(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(group) = self.state.top_level_group(name).cloned() else {
            return ui::page_root("top-level-page")
                .child(ui::muted_text(t("Loading"), cx))
                .into_any_element();
        };
        let target = ConfigTarget::Settings(group.name.clone());
        let busy = self.state.is_busy();
        let mut card = ui::card(cx).gap_2().p(px(16.0));
        for field in &group.fields {
            card = card.child(self.render_config_row(&target, field, window, cx));
        }
        ui::page_root("top-level-page")
            .child(ui::page_title(i18n::t(&group.name), cx))
            .when(!group.description.is_empty(), |this| {
                this.child(ui::muted_text(i18n::t(&group.description), cx))
            })
            .child(card.child(
                div().h_flex().justify_end().pt(px(4.0)).child(
                    ui::secondary_button("top-level-reset", t("Reset Config"), cx)
                        .disabled(busy)
                        .on_click(cx.listener({
                            let target = ConfigTarget::Settings(group.name.clone());
                            move |view, _, _, cx| view.reset_config(&target, cx)
                        })),
                ),
            ))
            .into_any_element()
    }

    pub fn render_notifications(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_top_level_group("Notifications", window, cx)
    }

    fn choice_control(
        &self,
        id: &'static str,
        current: &str,
        options: &[&str],
        cx: &mut Context<Self>,
        on_pick: impl Fn(&mut Self, String, &mut Context<Self>) + Clone + 'static,
    ) -> AnyElement {
        let open = self.open_select.as_deref() == Some(id);
        let mut column = div().v_flex().gap_1().items_end();
        column = column.child(
            ui::secondary_button(
                ElementId::Name(SharedString::from(format!("choice-{id}"))),
                current.to_owned(),
                cx,
            )
            .icon(OkIcon::ChevronDown.icon())
            .on_click(cx.listener(move |view, _, _, cx| {
                view.open_select = if view.open_select.as_deref() == Some(id) {
                    None
                } else {
                    Some(id.to_owned())
                };
                cx.notify();
            })),
        );
        if open {
            let mut popup = div()
                .v_flex()
                .min_w(px(180.0))
                .gap(px(1.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .p(px(4.0))
                .id(SharedString::from(format!("choice-popup-{id}")));
            for option in options {
                let option_owned = (*option).to_owned();
                let label = i18n::language_label(&option_owned).to_owned();
                let pick = on_pick.clone();
                popup = popup.child(
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "choice-{id}-{option_owned}"
                        ))))
                        .w_full()
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(5.0))
                        .text_size(px(ui::FS_SMALL))
                        .cursor_pointer()
                        .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.open_select = None;
                            pick(view, option_owned.clone(), cx);
                            cx.notify();
                        }))
                        .child(label),
                );
            }
            column = column.child(popup);
        }
        column.into_any_element()
    }

    // ---------------------------------------------------------------- schedule

    pub fn render_schedule(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.state.is_busy();
        let tasks = self.state.schedule.tasks.clone();
        let mut page = ui::page_root("schedule-page").child(
            div()
                .h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(ui::page_title(t("Schedule"), cx))
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            ui::secondary_button("schedule-refresh", t("Refresh"), cx)
                                .icon(OkIcon::Refresh.icon())
                                .disabled(busy)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.get("/api/schedule", cx);
                                })),
                        )
                        .child(
                            ui::secondary_button("schedule-create", t("Create Task"), cx)
                                .icon(OkIcon::Add.icon())
                                .disabled(busy || self.state.schedule.available_tasks.is_empty())
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.open_schedule_editor(None, cx);
                                })),
                        ),
                ),
        );
        if tasks.is_empty() {
            return page
                .child(ui::muted_text(t("No options available"), cx))
                .into_any_element();
        }
        let mut groups: Vec<(String, Vec<ScheduledTask>)> = Vec::new();
        for task in tasks {
            let group = task
                .path
                .trim_start_matches('\\')
                .split('\\')
                .next()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| t("Current App"));
            match groups.iter_mut().find(|(name, _)| name == &group) {
                Some((_, items)) => items.push(task),
                None => groups.push((group, vec![task])),
            }
        }
        for (group, items) in groups {
            let mut card = ui::card(cx).gap_0().p(px(4.0));
            for task in items {
                let name = if task.path.is_empty() {
                    task.name.clone()
                } else {
                    task.path.clone()
                };
                let encoded = api::url_encode(&name);
                let read_only = task.read_only;
                card = card.child(
                    div()
                        .h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px(px(11.0))
                        .py(px(9.0))
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .v_flex()
                                .gap(px(2.0))
                                .flex_1()
                                .child(
                                    div()
                                        .text_size(px(ui::FS_SMALL))
                                        .child(i18n::t(&task.name)),
                                )
                                .child(
                                    div()
                                        .text_size(px(ui::FS_TINY))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "{} · {} · {}",
                                            i18n::t(&task.status),
                                            i18n::t(&task.trigger_type),
                                            if task.next_run_time.is_empty() {
                                                "-".to_owned()
                                            } else {
                                                task.next_run_time.clone()
                                            }
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap_2()
                                .child(ui::switch_control(
                                    ElementId::Name(SharedString::from(format!(
                                        "schedule-enable-{encoded}"
                                    ))),
                                    task.enabled,
                                    busy || read_only,
                                    cx,
                                    cx.listener({
                                        let path =
                                            format!("/api/schedule/{encoded}/action");
                                        move |view, checked, _, cx| {
                                            view.post_action(
                                                &path,
                                                if *checked { "enable" } else { "disable" },
                                                cx,
                                            );
                                        }
                                    }),
                                ))
                                .child(
                                    ui::secondary_button(
                                        ElementId::Name(SharedString::from(format!(
                                            "schedule-modify-{encoded}"
                                        ))),
                                        t("Modify"),
                                        cx,
                                    )
                                    .disabled(busy || read_only)
                                    .on_click(cx.listener({
                                        let name = name.clone();
                                        move |view, _, _, cx| {
                                            view.open_schedule_editor(Some(name.clone()), cx);
                                        }
                                    })),
                                )
                                .child(
                                    ui::secondary_button(
                                        ElementId::Name(SharedString::from(format!(
                                            "schedule-delete-{encoded}"
                                        ))),
                                        t("Delete"),
                                        cx,
                                    )
                                    .icon(OkIcon::Delete.icon())
                                    .disabled(busy || read_only)
                                    .on_click(cx.listener({
                                        let path =
                                            format!("/api/schedule/{encoded}/action");
                                        let name = name.clone();
                                        move |view, _, _, cx| {
                                            view.modal = Some(Modal::Confirm {
                                                title: t("Confirm Delete"),
                                                message: i18n::tv(
                                                    "Are you sure you want to delete '{name}'?",
                                                    &[("name", &name)],
                                                ),
                                                confirm_label: t("Delete"),
                                                action: ConfirmAction::DeleteSchedule(
                                                    path.clone(),
                                                ),
                                            });
                                            cx.notify();
                                        }
                                    })),
                                ),
                        ),
                );
            }
            page = page
                .child(ui::section_title(group, cx))
                .child(card);
        }
        page.into_any_element()
    }

    // ------------------------------------------------------------------- about

    pub fn render_about(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let about = self.state.about.clone();
        let busy = self.state.is_busy();
        let avatar = self.app_avatar(40.0, 14.0, cx);
        let mut page = ui::page_root("about-page").child(
            div()
                .h_flex()
                .items_center()
                .gap_3()
                .child(avatar)
                .child(
                    div()
                        .v_flex()
                        .gap(px(2.0))
                        .child(ui::page_title(
                            if about.title.is_empty() {
                                "ok-script".to_owned()
                            } else {
                                about.title.clone()
                            },
                            cx,
                        ))
                        .child(ui::muted_text(
                            format!(
                                "{} · {}",
                                if about.version.is_empty() {
                                    "dev".to_owned()
                                } else {
                                    about.version.clone()
                                },
                                if about.debug {
                                    t("Debug")
                                } else {
                                    t("Release")
                                }
                            ),
                            cx,
                        )),
                ),
        );

        let mut links = div().h_flex().gap_2().flex_wrap();
        for (key, label, icon) in crate::brand_icons::ABOUT_LINK_ORDER {
            let Some(url) = crate::brand_icons::link_url(&about.links, key, i18n::locale()) else {
                continue;
            };
            let is_share = key == "share";
            let url_for_click = url.clone();
            links = links.child(
                ui::secondary_button(
                    ElementId::Name(SharedString::from(format!("about-link-{key}"))),
                    label,
                    cx,
                )
                .icon(icon.icon())
                .on_click(cx.listener(move |view, _, window, cx| {
                    if is_share {
                        match &url_for_click {
                            url if !url.is_empty() => {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(url.clone()));
                                view.toast(
                                    crate::model::ToastKind::Success,
                                    t("Share Link copied to clipboard"),
                                );
                            }
                            _ => view.toast(
                                crate::model::ToastKind::Error,
                                t("Action failed"),
                            ),
                        }
                    } else {
                        cx.open_url(&url_for_click);
                    }
                })),
            );
        }
        page = page.child(links);

        if about.update_supported {
            let current = about.version.clone();
            let versions: Vec<UpdateVersion> = self
                .state
                .updates
                .as_ref()
                .map(|updates| updates.versions.clone())
                .unwrap_or_default();
            let selected = self.updates_selected().or_else(|| {
                versions
                    .iter()
                    .find(|version| version.version != current)
                    .map(|version| version.version.clone())
            });
            let status = match (&self.state.updates, selected.as_deref()) {
                (None, _) => t("Click to check for updates"),
                (Some(_), None) => t("No updates available."),
                (Some(updates), Some(version)) => {
                    if compare_versions(version, &updates.current_version) > 0 {
                        format!("{} {}", t("Update"), version)
                    } else if compare_versions(version, &updates.current_version) < 0 {
                        format!("{} {}", t("Downgrade"), version)
                    } else {
                        t("Current version")
                    }
                }
            };
            let mut version_options = div().h_flex().gap_2().flex_wrap();
            for version in &versions {
                let value = version.version.clone();
                let is_selected = selected.as_deref() == Some(value.as_str());
                version_options = version_options.child(
                    ui::secondary_button(
                        ElementId::Name(SharedString::from(format!(
                            "update-version-{value}"
                        ))),
                        value.clone(),
                        cx,
                    )
                    .when(is_selected, |this| this.border_1().border_color(cx.theme().accent))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.input_values
                            .insert("update-version".to_owned(), value.clone());
                        cx.notify();
                    })),
                );
            }
            let mut card = ui::card(cx)
                .gap_2()
                .p(px(14.0))
                .child(ui::section_title(t("App update"), cx))
                .child(ui::muted_text(status, cx));
            if !versions.is_empty() {
                card = card.child(ui::muted_text(t("Version"), cx)).child(version_options);
            }
            card = card.child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        ui::secondary_button("update-check", t("Check for updates"), cx)
                            .icon(OkIcon::Refresh.icon())
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.get("/api/updates?release_only=true", cx);
                            })),
                    )
                    .when_some(selected.clone(), |this, version| {
                        this.child(
                            ui::primary_button("update-apply", t("Update"))
                                .disabled(busy)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.post(
                                        "/api/updates/apply",
                                        Some(json!({ "version": version.clone() })),
                                        cx,
                                    );
                                })),
                        )
                    }),
            );
            if let Some(updates) = &self.state.updates {
                let notes = selected
                    .as_deref()
                    .and_then(|version| {
                        updates
                            .versions
                            .iter()
                            .find(|item| item.version == version)
                    })
                    .map(|item| item.notes.clone())
                    .unwrap_or_default();
                card = card.child(ui::muted_text(
                    if notes.is_empty() {
                        t("No release notes.")
                    } else {
                        notes.join("\n")
                    },
                    cx,
                ));
            }
            page = page.child(card);
        }

        if !about.about.is_empty() {
            page = page
                .child(ui::section_title(t("Disclaimer"), cx))
                .child(
                    ui::card(cx).p(px(18.0)).child(
                        div()
                            .text_size(px(ui::FS_SMALL))
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::brand_icons::html_to_text(&about.about)),
                    ),
                );
        }

        if !about.projects.is_empty() {
            let mut grid = div().h_flex().gap_2().flex_wrap();
            for project in &about.projects {
                grid = grid.child(
                    ui::card(cx)
                        .w(px(300.0))
                        .gap(px(4.0))
                        .p(px(12.0))
                        .child(
                            div()
                                .text_size(px(ui::FS_BODY))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(i18n::t(&project.name)),
                        )
                        .child(
                            div()
                                .text_size(px(ui::FS_TINY))
                                .text_color(cx.theme().accent)
                                .child(project.url.replace("https://github.com/", "")),
                        ),
                );
            }
            page = page
                .child(ui::section_title(t("Other Projects"), cx))
                .child(grid);
        }
        page.into_any_element()
    }

    fn updates_selected(&self) -> Option<String> {
        self.input_values.get("update-version").cloned()
    }

    // --------------------------------------------------------- script/templates

    pub fn render_templates(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.state.is_busy();
        let query = self.template_query.to_lowercase();
        let templates: Vec<TemplateImage> = self
            .state
            .templates
            .iter()
            .filter(|template| {
                query.is_empty()
                    || template.name.to_lowercase().contains(&query)
                    || template
                        .categories
                        .iter()
                        .any(|category| category.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();
        for template in &templates {
            let url = self.client.url(&template.url);
            self.ensure_image(&url);
        }
        let selected = self.state.templates_selected.clone();
        let mut page = ui::page_root("templates-page").child(
            div()
                .h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(ui::page_title(t("Templates"), cx))
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            ui::primary_button("template-capture", t("Screenshot"))
                                .icon(OkIcon::Capture.icon())
                                .disabled(busy)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.post("/api/templates/capture", None, cx);
                                })),
                        )
                        .child(
                            ui::secondary_button("template-save", t("Save"), cx)
                                .icon(OkIcon::Save.icon())
                                .disabled(busy)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.modal = Some(Modal::TemplateSaveTo);
                                    cx.notify();
                                })),
                        )
                        .when_some(selected.clone(), |this, name| {
                            let encoded = api::url_encode(&name);
                            this.child(
                                ui::secondary_button("template-markup", t("Markup"), cx)
                                    .icon(OkIcon::Edit.icon())
                                    .disabled(busy)
                                    .on_click(cx.listener({
                                        let name = name.clone();
                                        move |view, _, _, cx| view.open_markup(name.clone(), cx)
                                    })),
                            )
                            .child(
                                ui::secondary_button("template-delete", t("Delete"), cx)
                                    .icon(OkIcon::Delete.icon())
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.modal = Some(Modal::Confirm {
                                            title: t("Confirm Delete"),
                                            message: i18n::tv(
                                                "Are you sure you want to delete '{name}'?",
                                                &[("name", &name)],
                                            ),
                                            confirm_label: t("Delete"),
                                            action: ConfirmAction::DeleteTemplate(
                                                name.clone(),
                                            ),
                                        });
                                        cx.notify();
                                    })),
                            )
                        }),
                ),
        );
        if templates.is_empty() {
            return page
                .child(ui::muted_text(t("No templates yet"), cx))
                .into_any_element();
        }
        let mut grid = div().h_flex().gap(px(10.0)).flex_wrap();
        for template in templates {
            let name = template.name.clone();
            let key = self.client.url(&template.url);
            let image = self.images.get(&key).cloned();
            let is_selected = self.state.templates_selected.as_deref() == Some(name.as_str());
            let detail = template.categories.join(", ");
            grid = grid.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "template-{name}"
                    ))))
                    .v_flex()
                    .w(px(180.0))
                    .gap(px(6.0))
                    .p(px(8.0))
                    .rounded(px(ui::CARD_RADIUS))
                    .bg(cx.theme().list)
                    .when(is_selected, |this| {
                        this.border_1().border_color(cx.theme().accent)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let name = name.clone();
                        move |view, _, _, cx| {
                            view.state.templates_selected = Some(name.clone());
                            cx.notify();
                        }
                    }))
                    .child(match image {
                        Some(image) => div()
                            .h(px(160.0))
                            .w_full()
                            .rounded(px(8.0))
                            .bg(Tokens::preview(ui::is_dark(cx)))
                            .child(
                                img(image)
                                    .size_full()
                                    .object_fit(ObjectFit::Contain),
                            )
                            .into_any_element(),
                        None => div()
                            .h(px(160.0))
                            .w_full()
                            .rounded(px(8.0))
                            .bg(Tokens::preview(ui::is_dark(cx)))
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .text_size(px(ui::FS_SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(i18n::t(&name)),
                    )
                    .child(
                        div()
                            .text_size(px(ui::FS_TINY))
                            .text_color(cx.theme().muted_foreground)
                            .child(detail),
                    ),
            );
        }
        page.child(grid).into_any_element()
    }


}

fn target_is_integer(target: &ConfigTarget) -> bool {
    let _ = target;
    false
}

fn about_link(links: &Value, key: &str) -> Option<String> {
    let value = links.get(key)?;
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    value
        .get("default")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// `compareVersions` from the web frontend (numeric dot segments).
pub fn compare_versions(left: &str, right: &str) -> i64 {
    let parse = |value: &str| -> Vec<i64> {
        value
            .trim_start_matches('v')
            .split(['.', '-', '_'])
            .map(|part| part.parse::<i64>().unwrap_or(0))
            .collect()
    };
    let left = parse(left);
    let right = parse(right);
    for index in 0..left.len().max(right.len()) {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        if a != b {
            return a - b;
        }
    }
    0
}

