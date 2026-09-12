//! Schedule (Windows task scheduler) create/edit dialog.
//!
//! Mirrors the `Create Schedule Task` / `Modify Schedule Task` modals in
//! `web_src/src/App.tsx` (same fields, ranges, defaults and request bodies).

use gpui::{
    div, prelude::*, px, AnyElement, App, Context, ElementId, IntoElement, ParentElement,
    SharedString, Styled, Window,
};
use gpui_component::{
    checkbox::Checkbox, input::Input, ActiveTheme as _, Disableable as _, StyledExt as _,
};
use serde_json::{json, Value};

use crate::api;
use crate::app::{Modal, OkApp};
use crate::components as ui;
use crate::i18n::t;
use crate::icons::OkIcon;
use crate::model::ScheduledTask;

const TRIGGER_TYPES: [&str; 5] = ["Daily", "Weekly", "Monthly", "Once", "Custom"];

/// Form state for the create/edit modal.
#[derive(Default, Clone)]
pub struct ScheduleForm {
    /// `None` while creating, `path || name` while editing.
    pub editing: Option<String>,
    pub title: String,
    pub task_index: i64,
    pub trigger_type: String,
    pub hour: String,
    pub minute: String,
    pub timeout: String,
    pub days: String,
    pub hours: String,
    pub auto_exit: bool,
    pub error: Option<String>,
}

impl ScheduleForm {
    fn defaults() -> Self {
        Self {
            editing: None,
            title: String::new(),
            task_index: 0,
            trigger_type: "Daily".to_owned(),
            hour: "9".to_owned(),
            minute: "0".to_owned(),
            timeout: "0".to_owned(),
            days: "0".to_owned(),
            hours: "0".to_owned(),
            auto_exit: true,
            error: None,
        }
    }

    fn from_task(task: &ScheduledTask) -> Self {
        Self {
            editing: Some(if task.path.is_empty() {
                task.name.clone()
            } else {
                task.path.clone()
            }),
            title: task.name.clone(),
            task_index: task.task_index,
            trigger_type: if task.trigger_type.is_empty() {
                "Daily".to_owned()
            } else {
                task.trigger_type.clone()
            },
            hour: task.start_hour.unwrap_or(9).to_string(),
            minute: task.start_minute.unwrap_or(0).to_string(),
            timeout: task.timeout_hours.unwrap_or(0).to_string(),
            days: task.interval_days.max(0).to_string(),
            hours: task.interval_hours.max(0).to_string(),
            auto_exit: task.auto_exit.unwrap_or(true),
            error: None,
        }
    }
}

impl OkApp {
    /// Open the create modal (`name = None`) or the edit modal for a task.
    pub fn open_schedule_editor(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        match name {
            Some(target) => {
                let task = self
                    .state
                    .schedule
                    .tasks
                    .iter()
                    .find(|task| task.path == target || task.name == target)
                    .cloned();
                let Some(task) = task else { return };
                self.schedule_form = ScheduleForm::from_task(&task);
            }
            None => {
                let mut form = ScheduleForm::defaults();
                form.task_index = self
                    .state
                    .schedule
                    .available_tasks
                    .first()
                    .map(|task| task.index)
                    .unwrap_or(0);
                self.schedule_form = form;
            }
        }
        for key in [
            "schedule-hour",
            "schedule-minute",
            "schedule-timeout",
            "schedule-days",
            "schedule-hours",
        ] {
            self.inputs.forget(key);
            self.input_values.remove(key);
        }
        self.open_select = None;
        self.modal = Some(Modal::ScheduleEditor { name: None });
        cx.notify();
    }

    /// Render the schedule create/edit modal.
    pub fn render_schedule_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editing = self.schedule_form.editing.is_some();
        let title = if editing {
            t("Modify Schedule Task")
        } else {
            t("Create Schedule Task")
        };
        let trigger_type = self.schedule_form.trigger_type.clone();
        let custom = trigger_type == "Custom";
        let auto_exit = self.schedule_form.auto_exit;
        let busy = self.state.is_busy();
        let error = self.schedule_form.error.clone();

        let form_values = self.schedule_form.clone();
        let number_input = |view: &mut Self,
                            window: &mut Window,
                            cx: &mut Context<Self>,
                            key: &str,
                            value: &str|
         -> gpui::Entity<gpui_component::input::InputState> {
            view.ensure_input(window, cx, key, "", value, false)
        };

        let hour = number_input(self, window, cx, "schedule-hour", &form_values.hour);
        let minute = number_input(self, window, cx, "schedule-minute", &form_values.minute);
        let timeout = number_input(self, window, cx, "schedule-timeout", &form_values.timeout);
        let days = number_input(self, window, cx, "schedule-days", &form_values.days);
        let hours = number_input(self, window, cx, "schedule-hours", &form_values.hours);

        let field = |label: &str, control: AnyElement, cx: &App| -> AnyElement {
            div()
                .v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(ui::FS_TINY))
                        .text_color(cx.theme().muted_foreground)
                        .child(label.to_owned()),
                )
                .child(control)
                .into_any_element()
        };

        // Task picker: editing shows the disabled name, creating lists the
        // available tasks (`data.available_tasks`).
        let task_control = if editing {
            div()
                .h_flex()
                .items_center()
                .h(px(ui::BUTTON_HEIGHT))
                .px(px(9.0))
                .rounded(px(ui::BUTTON_RADIUS))
                .border_1()
                .border_color(cx.theme().border)
                .text_size(px(ui::FS_SMALL))
                .text_color(cx.theme().muted_foreground)
                .child(t(&self.schedule_form.title))
                .into_any_element()
        } else {
            let selected = self
                .state
                .schedule
                .available_tasks
                .iter()
                .find(|task| task.index == self.schedule_form.task_index)
                .map(|task| format!("{} · {}", task.index, t(&task.name)))
                .unwrap_or_else(|| t("Select Task"));
            let open = self.open_select.as_deref() == Some("schedule-task");
            let mut column = div().v_flex().gap_1();
            column = column.child(
                ui::secondary_button("schedule-task-select", selected, cx)
                    .icon(OkIcon::ChevronDown.icon())
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.open_select = if view.open_select.as_deref() == Some("schedule-task") {
                            None
                        } else {
                            Some("schedule-task".to_owned())
                        };
                        cx.notify();
                    })),
            );
            if open {
                let mut popup = div()
                    .v_flex()
                    .min_w(px(220.0))
                    .gap(px(1.0))
                    .rounded(px(ui::BUTTON_RADIUS))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .p(px(4.0))
                    .id("schedule-task-menu");
                for task in self.state.schedule.available_tasks.clone() {
                    let selected = task.index == self.schedule_form.task_index;
                    popup = popup.child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "schedule-task-{}",
                                task.index
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
                                view.schedule_form.task_index = task.index;
                                view.open_select = None;
                                cx.notify();
                            }))
                            .child(format!("{} · {}", task.index, t(&task.name))),
                    );
                }
                column = column.child(popup);
            }
            column.into_any_element()
        };

        // Trigger type picker.
        let trigger_open = self.open_select.as_deref() == Some("schedule-trigger");
        let mut trigger_column = div().v_flex().gap_1();
        trigger_column = trigger_column.child(
            ui::secondary_button("schedule-trigger-select", t(&trigger_type), cx)
                .icon(OkIcon::ChevronDown.icon())
                .disabled(busy)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.open_select = if view.open_select.as_deref() == Some("schedule-trigger") {
                        None
                    } else {
                        Some("schedule-trigger".to_owned())
                    };
                    cx.notify();
                })),
        );
        if trigger_open {
            let mut popup = div()
                .v_flex()
                .min_w(px(160.0))
                .gap(px(1.0))
                .rounded(px(ui::BUTTON_RADIUS))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .p(px(4.0))
                .id("schedule-trigger-menu");
            for value in TRIGGER_TYPES {
                let selected = value == trigger_type;
                popup = popup.child(
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "schedule-trigger-{value}"
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
                            view.schedule_form.trigger_type = value.to_owned();
                            view.open_select = None;
                            cx.notify();
                        }))
                        .child(t(value)),
                );
            }
            trigger_column = trigger_column.child(popup);
        }

        let mut grid = div().h_flex().flex_wrap().gap(px(12.0));
        grid = grid
            .child(div().v_flex().gap(px(4.0)).w(px(260.0)).child(
                div()
                    .text_size(px(ui::FS_TINY))
                    .text_color(cx.theme().muted_foreground)
                    .child(if editing {
                        t("Task Name")
                    } else {
                        t("Select Task")
                    }),
            ).child(task_control))
            .child(div().v_flex().gap(px(4.0)).w(px(200.0)).child(
                div()
                    .text_size(px(ui::FS_TINY))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("Trigger Type")),
            ).child(trigger_column))
            .child(field(
                &t("Hour"),
                div().w(px(120.0)).child(Input::new(&hour)).into_any_element(),
                cx,
            ))
            .child(field(
                &t("Minute"),
                div().w(px(120.0)).child(Input::new(&minute)).into_any_element(),
                cx,
            ))
            .child(field(
                &t("Timeout"),
                div().w(px(120.0)).child(Input::new(&timeout)).into_any_element(),
                cx,
            ));
        if custom {
            grid = grid
                .child(field(
                    &t("Days"),
                    div().w(px(120.0)).child(Input::new(&days)).into_any_element(),
                    cx,
                ))
                .child(field(
                    &t("Hours"),
                    div().w(px(120.0)).child(Input::new(&hours)).into_any_element(),
                    cx,
                ));
        }

        let body = ui::modal_body()
            .child(grid)
            .child(
                Checkbox::new("schedule-auto-exit")
                    .checked(auto_exit)
                    .label(t("Auto Exit After Task"))
                    .on_click(cx.listener(|view, checked, _, cx| {
                        view.schedule_form.auto_exit = *checked;
                        cx.notify();
                    })),
            )
            .when_some(error, |this, error| {
                this.child(
                    div()
                        .text_size(px(ui::FS_TINY))
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            });

        let confirm_label = if editing { t("Confirm") } else { t("Create") };
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(620.0, cx)
                    .child(
                        ui::modal_header(title, cx)
                            .child(self.close_button("schedule-close", cx)),
                    )
                    .child(body)
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("schedule-cancel", t("Cancel"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::primary_button("schedule-confirm", confirm_label)
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.submit_schedule_form(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Validate the form and issue the create/update request.
    pub fn submit_schedule_form(&mut self, cx: &mut Context<Self>) {
        let read = |view: &Self, key: &str, fallback: &str| -> String {
            view.input_values
                .get(key)
                .cloned()
                .unwrap_or_else(|| fallback.to_owned())
        };
        let hour = read(self, "schedule-hour", &self.schedule_form.hour);
        let minute = read(self, "schedule-minute", &self.schedule_form.minute);
        let timeout = read(self, "schedule-timeout", &self.schedule_form.timeout);
        let days = read(self, "schedule-days", &self.schedule_form.days);
        let hours = read(self, "schedule-hours", &self.schedule_form.hours);

        let parse = |value: &str, min: i64, max: i64| -> Result<i64, ()> {
            value
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|number| *number >= min && *number <= max)
                .ok_or(())
        };
        let parsed = (|| -> Result<(i64, i64, i64, i64, i64), ()> {
            Ok((
                parse(&hour, 0, 23)?,
                parse(&minute, 0, 59)?,
                parse(&timeout, 0, 12)?,
                parse(&days, 0, 365).unwrap_or(0),
                parse(&hours, 0, 23).unwrap_or(0),
            ))
        })();

        let (start_hour, start_minute, timeout_hours, interval_days, interval_hours) = match parsed
        {
            Ok(values) => values,
            Err(()) => {
                self.schedule_form.error = Some(t("Invalid value"));
                cx.notify();
                return;
            }
        };

        let editing = self.schedule_form.editing.clone();
        let trigger_type = self.schedule_form.trigger_type.clone();
        if editing.is_none() && self.schedule_form.task_index <= 0 {
            self.schedule_form.error = Some(t("Invalid scheduled task"));
            cx.notify();
            return;
        }
        let task_name = self
            .state
            .schedule
            .available_tasks
            .iter()
            .find(|task| task.index == self.schedule_form.task_index)
            .map(|task| task.name.clone())
            .unwrap_or_default();

        let body: Value = match &editing {
            Some(_) => json!({
                "task_index": self.schedule_form.task_index,
                "trigger_type": trigger_type,
                "start_hour": start_hour,
                "start_minute": start_minute,
                "timeout_hours": timeout_hours,
                "auto_exit": self.schedule_form.auto_exit,
                "interval_days": interval_days,
                "interval_hours": interval_hours,
            }),
            None => json!({
                "name": task_name,
                "task_index": self.schedule_form.task_index,
                "trigger_type": trigger_type,
                "start_hour": start_hour,
                "start_minute": start_minute,
                "timeout_hours": timeout_hours,
                "auto_exit": self.schedule_form.auto_exit,
                "interval_days": interval_days,
                "interval_hours": interval_hours,
            }),
        };

        self.schedule_form.error = None;
        self.modal = None;
        match editing {
            Some(target) => {
                let path = format!("/api/schedule/{}", api::url_encode(&target));
                self.post(&path, Some(body), cx);
            }
            None => self.post("/api/schedule", Some(body), cx),
        }
    }
}
