//! Modal host: capture preview, instructions, log viewer, confirms and the
//! script/template dialogs.

use gpui::{
    div, img, prelude::*, px, AnyElement, Context, ElementId, FontWeight, IntoElement, ObjectFit,
    ParentElement, SharedString, Styled, Window,
};
use gpui_component::{button::Button, input::Input, ActiveTheme as _, Disableable as _, StyledExt as _};
use serde_json::Value;

use crate::api;
use crate::app::{ConfirmAction, ConfigTarget, Modal, OkApp};
use crate::components as ui;
use crate::i18n::{self, t};
use crate::icons::OkIcon;
use crate::theme::Tokens;

impl OkApp {
    pub fn render_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(modal) = self.modal.clone() else {
            return div().into_any_element();
        };
        let body = match modal {
            Modal::CapturePreview => self.modal_capture_preview(cx),
            Modal::Instructions { title, text } => self.modal_instructions(title, text, cx),
            Modal::Logs => self.modal_logs(window, cx),
            Modal::Confirm {
                title,
                message,
                confirm_label,
                action,
            } => self.modal_confirm(title, message, confirm_label, action, cx),
            Modal::UnsavedScript => self.modal_unsaved(cx),
            Modal::ExternalScriptChange => self.modal_external_change(cx),
            Modal::ListEditor {
                target,
                field,
                selected,
                active,
                draft,
            } => self.modal_list_editor(target, field, selected, active, draft, window, cx),
            Modal::CreateScript => self.modal_create_script(window, cx),
            Modal::RecordScript => self.modal_record_script(cx),
            other => self.modal_placeholder(other, cx),
        };
        div()
            .id("modal-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x0c070aad))
            .on_click(cx.listener(|view, _, _, cx| {
                view.modal = None;
                cx.notify();
            }))
            .child(body)
            .into_any_element()
    }

    fn close_button(&self, id: &'static str, cx: &mut Context<Self>) -> AnyElement {
        ui::icon_button(id, OkIcon::Close, cx)
            .on_click(cx.listener(|view, _, _, cx| {
                view.modal = None;
                cx.notify();
            }))
            .into_any_element()
    }

    fn modal_capture_preview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let url = self.capture_preview.clone().unwrap_or_default();
        self.ensure_image(&url);
        let image = self.images.get(&url).cloned();
        let header = ui::modal_header(t("Capture Preview"), cx).child(
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(
                    ui::secondary_button("preview-folder", t("Open Screenshot Folder"), cx)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.post("/api/tools/screenshot-folder", None, cx);
                        })),
                )
                .child(self.close_button("preview-close", cx)),
        );
        ui::modal_frame(1100.0, cx)
            .child(header)
            .child(
                div()
                    .v_flex()
                    .min_h(px(280.0))
                    .max_h(px(720.0))
                    .items_center()
                    .justify_center()
                    .bg(Tokens::preview(ui::is_dark(cx)))
                    .p(px(12.0))
                    .child(match image {
                        Some(image) => img(image)
                            .max_h(px(640.0))
                            .object_fit(ObjectFit::Contain)
                            .into_any_element(),
                        None => ui::muted_text(t("Loading"), cx).into_any_element(),
                    }),
            )
            .into_any_element()
    }

    fn modal_instructions(
        &self,
        title: String,
        text: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        ui::modal_frame(720.0, cx)
            .child(
                ui::modal_header(i18n::t(&title), cx)
                    .child(self.close_button("instructions-close", cx)),
            )
            .child(
                ui::modal_body().child(
                    div()
                        .text_size(px(ui::FS_BODY))
                        .whitespace_normal()
                        .child(text),
                ),
            )
            .into_any_element()
    }

    fn modal_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.ensure_input(
            window,
            cx,
            "logs-query",
            &t("Filter logs..."),
            "",
            false,
        );
        let levels = ["ALL", "DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"];
        let level_label = if self.logs_level == "ALL" {
            t("All Levels")
        } else {
            self.logs_level.clone()
        };
        let log_text = self
            .state
            .logs
            .as_ref()
            .map(|logs| logs.text.clone())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| t("Waiting for ok-script.log"));
        let footer = self
            .state
            .logs
            .as_ref()
            .map(|logs| {
                format!(
                    "{} · {} {}",
                    logs.path,
                    logs.line_count,
                    t("lines")
                )
            })
            .unwrap_or_else(|| "logs/ok-script.log".to_owned());
        let paused = self.logs_paused;

        let toolbar = div()
            .h_flex()
            .items_center()
            .gap_2()
            .w_full()
            .child(
                ui::secondary_button("logs-level", level_label, cx)
                    .icon(OkIcon::Filter.icon())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let index = levels
                            .iter()
                            .position(|level| *level == view.logs_level)
                            .unwrap_or(0);
                        let next = levels[(index + 1) % levels.len()];
                        view.logs_level = next.to_owned();
                        view.last_log_poll = std::time::Instant::now()
                            - std::time::Duration::from_secs(1);
                        cx.notify();
                    })),
            )
            .child(div().flex_1().child(Input::new(&query)))
            .child(
                ui::secondary_button(
                    "logs-pause",
                    if paused { t("Resume") } else { t("Pause") },
                    cx,
                )
                .icon(if paused {
                    OkIcon::Play.icon()
                } else {
                    OkIcon::Pause.icon()
                })
                .on_click(cx.listener(|view, _, _, cx| {
                    view.logs_paused = !view.logs_paused;
                    cx.notify();
                })),
            )
            .child(
                ui::secondary_button("logs-clear", t("Clear"), cx)
                    .icon(OkIcon::Close.icon())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.logs_paused = true;
                        if let Some(logs) = view.state.logs.as_mut() {
                            logs.text.clear();
                            logs.line_count = 0;
                        }
                        cx.notify();
                    })),
            );

        ui::modal_frame(1000.0, cx)
            .child(
                ui::modal_header(t("View Log"), cx)
                    .child(self.close_button("logs-close", cx)),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .p(px(12.0))
                    .h(px(640.0))
                    .child(toolbar)
                    .child(
                        div()
                            .v_flex()
                            .id("log-console")
                            .flex_1()
                            .min_h(px(0.0))
                            .rounded(px(6.0))
                            .bg(Tokens::console(ui::is_dark(cx)))
                            .p(px(10.0))
                            .overflow_y_scroll()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(px(13.0))
                            .text_color(cx.theme().foreground)
                            .child(log_text),
                    )
                    .child(
                        div()
                            .text_size(px(ui::FS_TINY))
                            .text_color(cx.theme().muted_foreground)
                            .child(footer),
                    ),
            )
            .into_any_element()
    }

    fn modal_confirm(
        &mut self,
        title: String,
        message: String,
        confirm_label: String,
        action: ConfirmAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.state.is_busy();
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(420.0, cx)
                    .child(
                        ui::modal_header(title, cx)
                            .child(self.close_button("confirm-close", cx)),
                    )
                    .child(ui::modal_body().child(
                        div()
                            .text_size(px(ui::FS_BODY))
                            .whitespace_normal()
                            .child(message),
                    ))
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("confirm-cancel", t("Cancel"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::primary_button("confirm-ok", confirm_label)
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.modal = None;
                                        view.run_confirm(action.clone(), cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub fn run_confirm(&mut self, action: ConfirmAction, cx: &mut Context<Self>) {
        match action {
            ConfirmAction::DeleteScript(name) => {
                let path = format!("/api/scripts/{}/delete", api::url_encode(&name));
                if self.script_open.as_deref() == Some(name.as_str()) {
                    self.script_open = None;
                    self.state.script_document = None;
                }
                self.post(&path, None, cx);
            }
            ConfirmAction::DeleteTemplate(name) => {
                let path = format!("/api/templates/{}/delete", api::url_encode(&name));
                self.state.templates_selected = None;
                self.post(&path, None, cx);
            }
            ConfirmAction::DeleteSchedule(path) => {
                self.post_action(&path, "delete", cx);
            }
            ConfirmAction::DeleteAnnotation(name, id) => {
                if let Some(annotations) = self.state.annotations.as_mut() {
                    annotations.annotations.retain(|item| item.id != id);
                    let body = serde_json::to_value(&annotations.annotations)
                        .unwrap_or(Value::Array(Vec::new()));
                    let path =
                        format!("/api/templates/{}/annotations", api::url_encode(&name));
                    self.post(&path, Some(serde_json::json!({ "annotations": body })), cx);
                }
            }
        }
    }

    fn modal_unsaved(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pending = self.pending_page.clone();
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(520.0, cx)
                    .child(ui::modal_header(t("Save Changes"), cx))
                    .child(ui::modal_body().child(
                        div()
                            .text_size(px(ui::FS_BODY))
                            .whitespace_normal()
                            .child(t(
                                "The current task has unsaved changes. Do you want to save them?",
                            )),
                    ))
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("unsaved-cancel", t("Cancel"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        view.pending_page = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::secondary_button("unsaved-discard", t("Don't Save"), cx)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.state.script_dirty = false;
                                        view.modal = None;
                                        if let Some(page) = pending.clone() {
                                            view.pending_page = None;
                                            view.set_page(page, cx);
                                        } else {
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                ui::primary_button("unsaved-save", t("Save"))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.save_script(cx);
                                        view.state.script_dirty = false;
                                        view.modal = None;
                                        if let Some(page) = view.pending_page.clone() {
                                            view.pending_page = None;
                                            view.set_page(page, cx);
                                        } else {
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn modal_external_change(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(560.0, cx)
                    .child(ui::modal_header(t("File Changed"), cx))
                    .child(ui::modal_body().child(
                        div()
                            .text_size(px(ui::FS_BODY))
                            .whitespace_normal()
                            .child(t(
                                "The file was modified externally. Reload it and discard your unsaved changes?",
                            )),
                    ))
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("external-keep", t("Keep Editing"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::primary_button("external-reload", t("Reload"))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        view.state.script_dirty = false;
                                        if let Some(name) = view.script_open.clone() {
                                            view.open_script(&name, cx);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn modal_list_editor(
        &mut self,
        target: ConfigTarget,
        field: crate::model::TaskConfigField,
        selected: Vec<Value>,
        active: Option<usize>,
        draft: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let options = field.options.clone().unwrap_or_default();
        let allow_duplication = field.allow_duplication;
        let draft_input = self.ensure_input(
            window,
            cx,
            "list-editor-draft",
            &t("Add"),
            &draft,
            false,
        );
        let mut left = div()
            .v_flex()
            .gap_2()
            .flex_1()
            .child(ui::section_title(
                if options.is_empty() {
                    t("Add")
                } else {
                    t("Available Options")
                },
                cx,
            ));
        if options.is_empty() {
            left = left.child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&draft_input)))
                    .child(
                        ui::secondary_button("list-add", t("Add"), cx).icon(OkIcon::Add.icon()).on_click(
                            cx.listener({
                                let target = target.clone();
                                let field = field.clone();
                                let selected = selected.clone();
                                move |view, _, _, cx| {
                                    let draft = view
                                        .input_values
                                        .get("list-editor-draft")
                                        .cloned()
                                        .unwrap_or_default();
                                    if draft.trim().is_empty() {
                                        return;
                                    }
                                    let mut next = selected.clone();
                                    next.push(Value::from(draft.trim()));
                                    view.state.pending.remove("list-editor");
                                    view.set_config(&target, &field.key, Value::Array(next), cx);
                                    view.modal = Some(Modal::ListEditor {
                                        target: target.clone(),
                                        field: field.clone(),
                                        selected: Vec::new(),
                                        active: None,
                                        draft: String::new(),
                                    });
                                    cx.notify();
                                }
                            }),
                        ),
                    ),
            );
        } else {
            let mut list = div().v_flex().gap(px(1.0));
            for option in &options {
                let label = match option {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                let already = selected.contains(option) && !allow_duplication;
                let target = target.clone();
                let field = field.clone();
                let value = option.clone();
                let current = selected.clone();
                list = list.child(
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "list-option-{label}"
                        ))))
                        .h_flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px(px(8.0))
                        .py(px(6.0))
                        .rounded(px(5.0))
                        .text_size(px(ui::FS_SMALL))
                        .when(already, |this| this.opacity(0.5))
                        .when(!already, |this| {
                            this.cursor_pointer()
                                .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                        })
                        .child(label.clone())
                        .when(!already, |this| {
                            this.on_click(cx.listener(move |view, _, _, cx| {
                                let mut next = current.clone();
                                next.push(value.clone());
                                view.modal = Some(Modal::ListEditor {
                                    target: target.clone(),
                                    field: field.clone(),
                                    selected: next.clone(),
                                    active: Some(next.len().saturating_sub(1)),
                                    draft: String::new(),
                                });
                                cx.notify();
                            }))
                        })
                        .child(
                            OkIcon::Add
                                .icon()
                                .size(px(16.0))
                                .text_color(cx.theme().muted_foreground),
                        ),
                );
            }
            left = left.child(
                div()
                    .v_flex()
                    .id("list-options")
                    .max_h(px(240.0))
                    .gap(px(1.0))
                    .overflow_y_scroll()
                    .child(list),
            );
        }

        let mut ordered = div().v_flex().gap(px(1.0));
        for (index, item) in selected.iter().enumerate() {
            let label = match item {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let is_active = active == Some(index);
            let count = selected.len();
            ordered = ordered.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "list-selected-{index}"
                    ))))
                    .h_flex()
                    .w_full()
                    .items_center()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(5.0))
                    .text_size(px(ui::FS_SMALL))
                    .when(is_active, |this| this.bg(cx.theme().list_active))
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let target = target.clone();
                        let field = field.clone();
                        let selected = selected.clone();
                        move |view, _, _, cx| {
                            view.modal = Some(Modal::ListEditor {
                                target: target.clone(),
                                field: field.clone(),
                                selected: selected.clone(),
                                active: Some(index),
                                draft: String::new(),
                            });
                            cx.notify();
                        }
                    }))
                    .child(label)
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                OkIcon::ArrowUp
                                    .icon()
                                    .size(px(14.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .when(index + 1 < count, |this| {
                                this.child(
                                    OkIcon::ArrowDown
                                        .icon()
                                        .size(px(14.0))
                                        .text_color(cx.theme().muted_foreground),
                                )
                            }),
                    ),
            );
        }

        ui::modal_frame(840.0, cx)
            .child(
                ui::modal_header(field.key.clone(), cx)
                    .child(self.close_button("list-close", cx)),
            )
            .child(
                ui::modal_body().child(
                    div()
                        .h_flex()
                        .gap(px(16.0))
                        .items_start()
                        .child(left)
                        .child(
                            div()
                                .v_flex()
                                .gap_2()
                                .flex_1()
                                .child(ui::section_title(t("Selected Options"), cx))
                                .child(
                                    div()
                                        .v_flex()
                                        .id("list-selected")
                                        .max_h(px(240.0))
                                        .gap(px(1.0))
                                        .overflow_y_scroll()
                                        .child(ordered),
                                ),
                        ),
                ),
            )
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("list-move-up", t("Move Up"), cx)
                            .disabled(active.is_none() || active == Some(0))
                            .on_click(cx.listener({
                                let target = target.clone();
                                let field = field.clone();
                                let selected = selected.clone();
                                move |view, _, _, cx| {
                                    if let Some(index) = active {
                                        if index > 0 {
                                            let mut next = selected.clone();
                                            next.swap(index, index - 1);
                                            view.modal = Some(Modal::ListEditor {
                                                target: target.clone(),
                                                field: field.clone(),
                                                selected: next,
                                                active: Some(index - 1),
                                                draft: String::new(),
                                            });
                                            cx.notify();
                                        }
                                    }
                                }
                            })),
                    )
                    .child(
                        ui::secondary_button("list-move-down", t("Move Down"), cx)
                            .disabled(active.is_none() || active.map(|index| index + 1 >= selected.len()).unwrap_or(true))
                            .on_click(cx.listener({
                                let target = target.clone();
                                let field = field.clone();
                                let selected = selected.clone();
                                move |view, _, _, cx| {
                                    if let Some(index) = active {
                                        if index + 1 < selected.len() {
                                            let mut next = selected.clone();
                                            next.swap(index, index + 1);
                                            view.modal = Some(Modal::ListEditor {
                                                target: target.clone(),
                                                field: field.clone(),
                                                selected: next,
                                                active: Some(index + 1),
                                                draft: String::new(),
                                            });
                                            cx.notify();
                                        }
                                    }
                                }
                            })),
                    )
                    .child(
                        ui::secondary_button("list-remove", t("Remove"), cx)
                            .disabled(active.is_none())
                            .on_click(cx.listener({
                                let target = target.clone();
                                let field = field.clone();
                                let selected = selected.clone();
                                move |view, _, _, cx| {
                                    if let Some(index) = active {
                                        let mut next = selected.clone();
                                        next.remove(index);
                                        view.modal = Some(Modal::ListEditor {
                                            target: target.clone(),
                                            field: field.clone(),
                                            selected: next.clone(),
                                            active: None,
                                            draft: String::new(),
                                        });
                                        cx.notify();
                                    }
                                }
                            })),
                    )
                    .child(
                        ui::secondary_button("list-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.modal = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button("list-confirm", t("Confirm"))
                            .on_click(cx.listener({
                                let target = target.clone();
                                let field = field.clone();
                                let selected = selected.clone();
                                move |view, _, _, cx| {
                                    view.set_config(
                                        &target,
                                        &field.key,
                                        Value::Array(selected.clone()),
                                        cx,
                                    );
                                    view.modal = None;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .into_any_element()
    }

    fn modal_create_script(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let class = self.ensure_input(window, cx, "create-class", "Class Name (English only)", "", false);
        let name = self.ensure_input(window, cx, "create-name", "Task Name", "", false);
        let description =
            self.ensure_input(window, cx, "create-desc", "Description (Optional)", "", false);
        let busy = self.state.is_busy();
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(520.0, cx)
                    .child(
                        ui::modal_header(t("Create Task"), cx)
                            .child(self.close_button("create-close", cx)),
                    )
                    .child(
                        ui::modal_body()
                            .child(Input::new(&class))
                            .child(Input::new(&name))
                            .child(Input::new(&description)),
                    )
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("create-cancel", t("Cancel"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::primary_button("create-ok", t("Create"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let class_name = view
                                            .input_values
                                            .get("create-class")
                                            .cloned()
                                            .unwrap_or_default();
                                        let task_name = view
                                            .input_values
                                            .get("create-name")
                                            .cloned()
                                            .unwrap_or_default();
                                        let description = view
                                            .input_values
                                            .get("create-desc")
                                            .cloned()
                                            .unwrap_or_default();
                                        if class_name.trim().is_empty()
                                            || task_name.trim().is_empty()
                                        {
                                            view.toast(
                                                crate::model::ToastKind::Error,
                                                t("Class Name (English only)"),
                                            );
                                            return;
                                        }
                                        view.modal = None;
                                        view.post(
                                            "/api/scripts",
                                            Some(serde_json::json!({
                                                "class_name": class_name,
                                                "task_name": task_name,
                                                "description": description,
                                            })),
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn modal_record_script(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let loop_choice = self
            .input_values
            .get("record-loop")
            .cloned()
            .unwrap_or_else(|| "none".to_owned());
        let label = match loop_choice.as_str() {
            "count" => t("Loop x times"),
            "forever" => t("Loop infinitely"),
            _ => t("No loop"),
        };
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(460.0, cx)
                    .child(
                        ui::modal_header(t("Record"), cx)
                            .child(self.close_button("record-close", cx)),
                    )
                    .child(ui::modal_body().child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(t("Record will override the current script logic. Continue?"))
                            .child(
                                ui::secondary_button("record-loop", label, cx)
                                    .icon(OkIcon::Refresh.icon())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let current = view
                                            .input_values
                                            .get("record-loop")
                                            .cloned()
                                            .unwrap_or_else(|| "none".to_owned());
                                        let next = match current.as_str() {
                                            "none" => "count",
                                            "count" => "forever",
                                            _ => "none",
                                        };
                                        view.input_values
                                            .insert("record-loop".to_owned(), next.to_owned());
                                        cx.notify();
                                    })),
                            ),
                    ))
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("record-cancel", t("Cancel"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::primary_button("record-ok", t("OK")).on_click(
                                    cx.listener(|view, _, _, cx| {
                                        view.modal = None;
                                        view.post("/api/scripts-record/start", None, cx);
                                    }),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn modal_placeholder(&mut self, modal: Modal, cx: &mut Context<Self>) -> AnyElement {
        let title = match modal {
            Modal::TemplateSaveTo => t("Save To"),
            Modal::ExportScript => t("Export Script"),
            Modal::ImportScript { .. } => t("Warning"),
            Modal::BboxEditor { .. } => t("Bounding Box"),
            _ => t("Loading"),
        };
        div()
            .id("modal-content")
            .on_click(|_, _, _| {})
            .child(
                ui::modal_frame(520.0, cx)
                    .child(
                        ui::modal_header(title, cx)
                            .child(self.close_button("placeholder-close", cx)),
                    )
                    .child(ui::modal_body().child(ui::muted_text(
                        "This dialog is not implemented in the native shell yet.",
                        cx,
                    ))),
            )
            .into_any_element()
    }
}

#[allow(unused_imports)]
use Button as _button_used;
