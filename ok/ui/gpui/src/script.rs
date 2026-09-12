//! Script page and its dialogs — mirrors `ScriptPage`, `ScriptDialogs.tsx` and
//! `PythonCodeEditor.tsx` from the web frontend.
//!
//! OWNER: this file is owned by the script work package. The page keeps its own
//! [`ScriptUiState`]; the shell calls `OkApp::render_script`, `OkApp::open_script`,
//! `OkApp::save_script`, `OkApp::run_script` and `OkApp::poll_script`.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, px, AnyElement, Context, ElementId, FontWeight, IntoElement, KeyDownEvent,
    ParentElement, SharedString, Styled, Window,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState, Position},
    ActiveTheme as _, Disableable as _, StyledExt as _,
};
use serde_json::json;

use crate::api;
use crate::app::OkApp;
use crate::components as ui;
use crate::i18n::{self, t};
use crate::icons::OkIcon;
use crate::model::*;
use crate::theme::Tokens;

/// Bridge for the two operations that need a background thread and cannot
/// travel through the shell's update queue: export options (`GET`) and the file
/// picked by `rfd` for import.
static EXPORT_BRIDGE: Mutex<Option<ScriptExportOptions>> = Mutex::new(None);
static IMPORT_BRIDGE: Mutex<Option<(String, Vec<u8>)>> = Mutex::new(None);

/// Page-local UI state (editor, dialogs, dirty tracking).
pub struct ScriptUiState {
    /// Code editor entity (created lazily; `code_editor("python")` + line numbers).
    pub editor: Option<gpui::Entity<InputState>>,
    /// Working copy of the open document.
    pub code: String,
    /// Working copy differs from the server document.
    pub dirty: bool,
    /// The editor must adopt `state.script_document` when it arrives.
    pub sync_pending: bool,
    /// `modified` timestamp of the document the editor was last synced with.
    pub loaded_modified: f64,
    /// Template search query.
    pub query: String,
    /// Template tree categories the user expanded.
    pub expanded_categories: HashSet<String>,
    /// Template waiting for parameter values.
    pub parameter_template: Option<ScriptTemplate>,
    pub parameter_values: HashMap<String, String>,
    /// Task dropdown / File menu popovers.
    pub task_menu_open: bool,
    pub file_menu_open: bool,
    /// Script the user wants to open while the editor is dirty.
    pub pending_script: Option<String>,
    pub delete_target: Option<String>,
    /// Create Task dialog.
    pub create_open: bool,
    pub create_class: String,
    pub create_name: String,
    pub create_desc: String,
    /// Export dialog.
    pub export_open: bool,
    pub export_options: Option<ScriptExportOptions>,
    pub export_selected: Vec<String>,
    pub export_file_name: String,
    pub export_script_name: String,
    pub export_version: String,
    /// Import dialog.
    pub import_open: bool,
    pub import_name: String,
    pub import_bytes: Option<Vec<u8>>,
    pub import_accepted: bool,
    pub import_seconds: i64,
    pub import_tick: Option<Instant>,
    /// Record dialog.
    pub record_open: bool,
    pub record_loop: String,
    pub record_count: String,
    /// External change detected while dirty.
    pub external: Option<ScriptDocument>,
    /// Feedback bookkeeping.
    pub save_pending: bool,
    pub run_pending: bool,
    pub copy_pending: bool,
    pub create_pending: bool,
    pub delete_pending: bool,
    pub import_pending: bool,
    /// Set right after a record stop so the editor adopts the merged code.
    pub record_sync: bool,
    pub last_tick: Option<Instant>,
}

impl Default for ScriptUiState {
    fn default() -> Self {
        Self {
            editor: None,
            code: String::new(),
            dirty: false,
            sync_pending: false,
            loaded_modified: 0.0,
            query: String::new(),
            expanded_categories: HashSet::new(),
            parameter_template: None,
            parameter_values: HashMap::new(),
            task_menu_open: false,
            file_menu_open: false,
            pending_script: None,
            delete_target: None,
            create_open: false,
            create_class: String::new(),
            create_name: String::new(),
            create_desc: String::new(),
            export_open: false,
            export_options: None,
            export_selected: Vec::new(),
            export_file_name: String::new(),
            export_script_name: String::new(),
            export_version: String::new(),
            import_open: false,
            import_name: String::new(),
            import_bytes: None,
            import_accepted: false,
            import_seconds: 15,
            import_tick: None,
            record_open: false,
            record_loop: "none".to_owned(),
            record_count: "10".to_owned(),
            external: None,
            save_pending: false,
            run_pending: false,
            copy_pending: false,
            create_pending: false,
            delete_pending: false,
            import_pending: false,
            record_sync: false,
            last_tick: None,
        }
    }
}

impl OkApp {
    // ------------------------------------------------------------------ editor

    fn ensure_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.script_ui.editor.is_some() {
            return;
        }
        let code = self
            .state
            .script_document
            .as_ref()
            .map(|document| document.code.clone())
            .unwrap_or_default();
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .code_editor("python")
                .line_number(true)
                .soft_wrap(false)
                .default_value(code.clone())
        });
        self.script_ui.code = code;
        cx.subscribe(&editor, |view, state, event, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let text = state.read(cx).value().to_string();
            view.script_ui.code = text.clone();
            let dirty = view
                .state
                .script_document
                .as_ref()
                .map(|document| document.code != text)
                .unwrap_or(false);
            view.script_ui.dirty = dirty;
            view.state.script_dirty = dirty;
            cx.notify();
        })
        .detach();
        self.script_ui.editor = Some(editor);
    }

    fn set_editor_value(
        &mut self,
        value: String,
        cursor: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.script_ui.code = value.clone();
        let Some(editor) = self.script_ui.editor.clone() else {
            return;
        };
        let dirty = self
            .state
            .script_document
            .as_ref()
            .map(|document| document.code != value)
            .unwrap_or(false);
        self.script_ui.dirty = dirty;
        self.state.script_dirty = dirty;
        editor.update(cx, |state, cx| {
            state.set_value(value.clone(), window, cx);
            if let Some(offset) = cursor {
                let offset = clamp_boundary(&value, offset);
                let position = position_for_offset(&value, offset);
                state.set_cursor_position(position, window, cx);
            }
        });
    }

    /// Adopt the server document / detect external edits. Called every render.
    fn tick_script(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let due = self
            .script_ui
            .last_tick
            .map(|last| now.duration_since(last) >= Duration::from_millis(500))
            .unwrap_or(true);
        if due {
            self.script_ui.last_tick = Some(now);
            if self.script_ui.import_open {
                self.script_ui.import_seconds = (self.script_ui.import_seconds - 1).max(0);
                cx.notify();
            }
        }
        if let Some(last) = self.script_ui.import_tick {
            if !self.script_ui.import_open || now.duration_since(last) > Duration::from_secs(600) {
                self.script_ui.import_tick = None;
            }
        }

        // Background bridges.
        if let Ok(mut guard) = EXPORT_BRIDGE.lock() {
            if let Some(options) = guard.take() {
                self.script_ui.export_selected = options.tasks.clone();
                self.script_ui.export_file_name = manifest_value(&options, "file_name")
                    .unwrap_or_else(|| "tasks".to_owned());
                self.script_ui.export_script_name = manifest_value(&options, "script_name")
                    .unwrap_or_default();
                self.script_ui.export_version = manifest_value(&options, "version")
                    .unwrap_or_else(|| "1.0.0".to_owned());
                self.script_ui.export_options = Some(options);
                self.script_ui.export_open = true;
            }
        }
        if let Ok(mut guard) = IMPORT_BRIDGE.lock() {
            if let Some((name, bytes)) = guard.take() {
                self.script_ui.import_name = name;
                self.script_ui.import_bytes = Some(bytes);
                self.script_ui.import_accepted = false;
                self.script_ui.import_seconds = 15;
                self.script_ui.import_tick = Some(Instant::now());
                self.script_ui.import_open = true;
            }
        }

        // The shell's response router refreshes `script_dirty` from every poll
        // response; the editor owns the working copy, so re-assert it here.
        if self.state.script_dirty != self.script_ui.dirty {
            self.state.script_dirty = self.script_ui.dirty;
        }

        let document = self.state.script_document.clone();
        let Some(document) = document else {
            return;
        };
        let matches_open = self
            .script_open
            .as_deref()
            .map(|name| name == document.name)
            .unwrap_or(false);

        if self.script_ui.sync_pending && matches_open {
            self.script_ui.sync_pending = false;
            self.script_ui.loaded_modified = document.modified;
            self.set_editor_value(document.code.clone(), None, window, cx);
        } else if self.script_ui.record_sync && matches_open {
            self.script_ui.record_sync = false;
            self.script_ui.loaded_modified = document.modified;
            self.set_editor_value(document.code.clone(), None, window, cx);
        } else if matches_open && document.modified != self.script_ui.loaded_modified {
            if self.script_ui.dirty {
                self.script_ui.external = Some(document.clone());
            } else {
                self.script_ui.loaded_modified = document.modified;
                self.set_editor_value(document.code.clone(), None, window, cx);
            }
        }

        // Feedback that arrives with the response entity.
        let error = document.error.clone();
        if self.script_ui.save_pending && !self.state.pending(format!("/api/scripts/{}", api::url_encode(&document.name)).as_str()) {
            self.script_ui.save_pending = false;
            match error.clone() {
                Some(message) => self.toast(ToastKind::Error, message),
                None => {
                    self.toast(ToastKind::Success, t("Task rebuilt successfully."));
                    self.get("/api/scripts", cx);
                }
            }
        }
        if self.script_ui.run_pending {
            self.script_ui.run_pending = false;
            if let Some(message) = error.clone() {
                self.toast(ToastKind::Error, message);
            }
        }
        if self.script_ui.copy_pending && !self.state.is_busy() {
            self.script_ui.copy_pending = false;
            self.toast(ToastKind::Success, t("Task copied successfully."));
            self.get("/api/scripts", cx);
        }
        if self.script_ui.create_pending && !self.state.is_busy() {
            self.script_ui.create_pending = false;
            self.toast(ToastKind::Success, t("Task created successfully."));
            self.get("/api/scripts", cx);
        }
        if self.script_ui.delete_pending && !self.state.is_busy() {
            self.script_ui.delete_pending = false;
            self.toast(ToastKind::Success, t("Task deleted successfully."));
            self.get("/api/scripts", cx);
        }
        if self.script_ui.import_pending && !self.state.is_busy() {
            self.script_ui.import_pending = false;
            self.toast(ToastKind::Success, t("Task created successfully."));
            self.get("/api/scripts", cx);
        }
    }

    // -------------------------------------------------------------------- page

    pub fn render_script(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_editor(window, cx);
        self.tick_script(window, cx);

        let busy = self.state.is_busy();
        let document = self.state.script_document.clone();
        let open_name = self.script_open.clone();

        // ---- left panel: template tree -------------------------------------
        let search = self.ensure_input(
            window,
            cx,
            "script-template-search",
            &t("Search templates..."),
            "",
            false,
        );
        let query = self
            .input_values
            .get("script-template-search")
            .cloned()
            .unwrap_or_else(|| self.script_ui.query.clone())
            .trim()
            .to_lowercase();
        let mut groups: Vec<(String, Vec<ScriptTemplate>)> = Vec::new();
        for template in self.state.script_templates.clone() {
            if !query.is_empty() {
                let haystack = format!("{} {}", template.name, template.doc).to_lowercase();
                if !haystack.contains(&query) {
                    continue;
                }
            }
            match groups
                .iter_mut()
                .find(|(category, _)| category == &template.category)
            {
                Some((_, items)) => items.push(template),
                None => groups.push((template.category.clone(), vec![template])),
            }
        }
        let mut tree = div()
            .v_flex()
            .id("script-template-tree")
            .flex_1()
            .min_h(px(0.0))
            .gap(px(1.0))
            .overflow_y_scroll();
        if groups.is_empty() {
            tree = tree.child(ui::muted_text(t("No options available"), cx));
        }
        for (category, items) in groups {
            let expanded = self.script_ui.expanded_categories.contains(&category);
            let category_key = category.clone();
            tree = tree.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "template-category-{category}"
                    ))))
                    .h_flex()
                    .items_center()
                    .gap_1()
                    .px(px(8.0))
                    .py(px(7.0))
                    .rounded(px(5.0))
                    .cursor_pointer()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view
                            .script_ui
                            .expanded_categories
                            .contains(&category_key)
                        {
                            view.script_ui.expanded_categories.remove(&category_key);
                        } else {
                            view.script_ui
                                .expanded_categories
                                .insert(category_key.clone());
                        }
                        cx.notify();
                    }))
                    .child(
                        OkIcon::ChevronRight
                            .icon()
                            .size(px(14.0))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(i18n::t(&category)),
            );
            if expanded {
                for template in items {
                    let template_for_click = template.clone();
                    let label = if template.template_name.is_empty() {
                        template.name.clone()
                    } else {
                        template.template_name.clone()
                    };
                    tree = tree.child(
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "template-{}-{}",
                                template.class_name, template.name
                            ))))
                            .w_full()
                            .px(px(8.0))
                            .py(px(7.0))
                            .pl(px(24.0))
                            .rounded(px(5.0))
                            .cursor_pointer()
                            .text_size(px(14.0))
                            .truncate()
                            .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.insert_template(template_for_click.clone(), window, cx);
                            }))
                            .child(i18n::t(&label)),
                    );
                }
            }
        }
        let template_panel = div()
            .v_flex()
            .w(px(235.0))
            .flex_none()
            .gap(px(9.0))
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
            )
            .child(ui::card(cx).flex_1().min_h(px(0.0)).p(px(4.0)).child(tree));

        // ---- toolbar -------------------------------------------------------
        let current_label = open_name.clone().unwrap_or_else(|| t("Select task to edit"));
        let mut task_menu = div().v_flex().gap(px(1.0));
        for script in self.state.scripts.clone() {
            let name = script.name.clone();
            let selected = open_name.as_deref() == Some(name.as_str());
            let target = name.clone();
            task_menu = task_menu.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "script-option-{name}"
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
                        view.script_ui.task_menu_open = false;
                        view.request_open_script(target.clone(), cx);
                    }))
                    .child(i18n::t(&name)),
            );
        }

        let mut file_menu = div()
            .v_flex()
            .min_w(px(220.0))
            .gap(px(1.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p(px(4.0));
        let file_items: Vec<(&str, OkIcon, bool)> = vec![
            ("Save", OkIcon::Save, document.is_none() || busy),
            ("Create Task", OkIcon::Add, false),
            ("Copy Task", OkIcon::Copy, document.is_none() || busy),
            ("Delete Task", OkIcon::Delete, document.is_none() || busy),
        ];
        for (label, icon, disabled) in file_items {
            file_menu = file_menu.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "file-menu-{label}"
                    ))))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(5.0))
                    .text_size(px(ui::FS_SMALL))
                    .when(disabled, |this| this.opacity(0.5))
                    .when(!disabled, |this| {
                        this.cursor_pointer()
                            .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                    })
                    .child(icon.icon().size(px(16.0)))
                    .child(i18n::t(label))
                    .when(label == "Save", |this| {
                        this.child(
                            div()
                                .ml_auto()
                                .pl(px(24.0))
                                .text_size(px(ui::FS_TINY))
                                .text_color(cx.theme().muted_foreground)
                                .child("Ctrl+S"),
                        )
                    })
                    .when(!disabled, |this| {
                        this.on_click(cx.listener(move |view, _, window, cx| {
                            view.script_file_action(label, window, cx);
                        }))
                    }),
            );
        }
        file_menu = file_menu.child(div().h(px(1.0)).w_full().bg(cx.theme().border));
        for (label, icon) in [
            ("Export Script", OkIcon::ArrowExport),
            ("Import Script", OkIcon::ArrowImport),
        ] {
            file_menu = file_menu.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "file-menu-{label}"
                    ))))
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(5.0))
                    .text_size(px(ui::FS_SMALL))
                    .cursor_pointer()
                    .hover(|style| style.bg(gpui::rgba(0xffffff11)))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.script_file_action(label, window, cx);
                    }))
                    .child(icon.icon().size(px(16.0)))
                    .child(i18n::t(label)),
            );
        }

        let toolbar = div()
            .h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                div()
                    .relative()
                    .child(ui::secondary_button(
                        "script-task-dropdown",
                        current_label.clone(),
                        cx,
                    )
                    .icon(OkIcon::ChevronDown.icon())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.script_ui.task_menu_open = !view.script_ui.task_menu_open;
                        view.script_ui.file_menu_open = false;
                        cx.notify();
                    })))
                    .when(self.script_ui.task_menu_open, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(36.0))
                                .left(px(0.0))
                                .min_w(px(240.0))
                                .max_h(px(320.0))
                                .id("script-task-menu")
                                .overflow_y_scroll()
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().popover)
                                .p(px(4.0))
                                .child(task_menu),
                        )
                    }),
            )
            .child(
                div()
                    .relative()
                    .child(
                        ui::secondary_button("script-file-menu", t("File"), cx)
                            .icon(OkIcon::DocumentText.icon())
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.file_menu_open = !view.script_ui.file_menu_open;
                                view.script_ui.task_menu_open = false;
                                cx.notify();
                            })),
                    )
                    .when(self.script_ui.file_menu_open, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(36.0))
                                .left(px(0.0))
                                .child(file_menu),
                        )
                    }),
            )
            .child(div().flex_1())
            .child(
                ui::primary_button(
                    "script-run",
                    if self.script_recording {
                        t("Stop")
                    } else {
                        t("Run")
                    },
                )
                .icon(if self.script_recording {
                    OkIcon::Stop.icon()
                } else {
                    OkIcon::Play.icon()
                })
                .disabled(document.is_none() || busy)
                .on_click(cx.listener(|view, _, window, cx| {
                    if view.script_recording {
                        view.stop_recording(cx);
                    } else {
                        view.save_or_run(window, cx, true);
                    }
                })),
            )
            .when(!self.script_recording, |this| {
                this.child(
                    ui::secondary_button("script-record", t("Record"), cx)
                        .icon(OkIcon::Record.icon())
                        .disabled(document.is_none() || busy)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.script_ui.record_open = true;
                            cx.notify();
                        })),
                )
            })
            .child(
                ui::secondary_button("script-guide", t("Guide"), cx)
                    .icon(OkIcon::QuestionCircle.icon())
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.open_url("https://github.com/ok-oldking/ok-py");
                    })),
            );

        // ---- editor --------------------------------------------------------
        let error_line = document
            .as_ref()
            .and_then(|document| document.error.as_deref())
            .and_then(extract_error_line);
        let editor_body: AnyElement = match &self.script_ui.editor {
            Some(editor) => div()
                .v_flex()
                .flex_1()
                .min_h(px(0.0))
                .id("script-editor-host")
                .on_key_down(cx.listener(
                    move |view, event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.to_lowercase();
                        if event.keystroke.modifiers.control && key == "s" {
                            view.save_or_run(window, cx, false);
                            cx.stop_propagation();
                            return;
                        }
                        if key == "tab" {
                            view.indent_selection(event.keystroke.modifiers.shift, window, cx);
                            cx.stop_propagation();
                        }
                    },
                ))
                .child(
                    div()
                        .w_full()
                        .min_h(px(420.0))
                        .child(
                            Input::new(editor)
                                .h_full()
                                .bordered(false)
                                .disabled(busy),
                        ),
                )
                .when_some(error_line, |this, line| {
                    this.child(
                        div()
                            .text_size(px(ui::FS_TINY))
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n::tv("Error on line {line}", &[("line", &line.to_string())])),
                    )
                })
                .into_any_element(),
            None => div().flex_1().into_any_element(),
        };
        let editor_panel = div()
            .v_flex()
            .flex_1()
            .min_w(px(0.0))
            .gap(px(9.0))
            .child(toolbar)
            .child(
                ui::card(cx)
                    .flex_1()
                    .min_h(px(0.0))
                    .p(px(12.0))
                    .when(document.is_none(), |this| {
                        this.items_center().justify_center().min_h(px(420.0))
                    })
                    .child(if document.is_some() {
                        editor_body
                    } else {
                        div()
                            .v_flex()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .child(
                                ui::primary_button("script-create-new", t("Create New Task"))
                                    .icon(OkIcon::Add.icon())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.script_ui.create_open = true;
                                        cx.notify();
                                    })),
                            )
                            .into_any_element()
                    })
                    .when(
                        document
                            .as_ref()
                            .and_then(|document| document.error.clone())
                            .is_some(),
                        |this| {
                            this.child(
                                div()
                                    .id("script-error-block")
                                    .w_full()
                                    .max_h(px(110.0))
                                    .overflow_y_scroll()
                                    .rounded(px(6.0))
                                    .bg(gpui::rgba(0xc42b1c33))
                                    .p(px(10.0))
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_size(px(13.0))
                                    .text_color(cx.theme().danger)
                                    .child(
                                        document
                                            .as_ref()
                                            .and_then(|document| document.error.clone())
                                            .unwrap_or_default(),
                                    ),
                            )
                        },
                    ),
            );

        let page = ui::page_root("script-page").child(
            div()
                .h_flex()
                .w_full()
                .flex_1()
                .min_h(px(0.0))
                .gap(px(12.0))
                .items_start()
                .child(template_panel)
                .child(editor_panel),
        );

        // ---- overlays ------------------------------------------------------
        let mut overlays: Vec<AnyElement> = Vec::new();
        if self.script_ui.parameter_template.is_some() {
            overlays.push(self.parameter_dialog(window, cx));
        }
        if self.script_ui.create_open {
            overlays.push(self.create_dialog(window, cx));
        }
        if self.script_ui.export_open {
            overlays.push(self.export_dialog(window, cx));
        }
        if self.script_ui.import_open {
            overlays.push(self.import_dialog(cx));
        }
        if self.script_ui.record_open {
            overlays.push(self.record_dialog(cx));
        }
        if let Some(name) = self.script_ui.delete_target.clone() {
            overlays.push(self.delete_dialog(name, cx));
        }
        if let Some(document) = self.script_ui.external.clone() {
            overlays.push(self.external_dialog(document, cx));
        }
        if let Some(name) = self.script_ui.pending_script.clone() {
            overlays.push(self.unsaved_dialog(name, cx));
        }

        div()
            .relative()
            .size_full()
            .child(page)
            .children(overlays)
            .into_any_element()
    }

    // ----------------------------------------------------------- interactions

    /// Open a script, guarding unsaved edits exactly like the web page does.
    pub fn request_open_script(&mut self, name: String, cx: &mut Context<Self>) {
        let dirty = self.script_ui.dirty;
        let same = self.script_open.as_deref() == Some(name.as_str());
        if dirty && !same {
            self.script_ui.pending_script = Some(name);
            cx.notify();
            return;
        }
        self.open_script(&name, cx);
    }

    /// Open (or reload) a script document.
    pub fn open_script(&mut self, name: &str, cx: &mut Context<Self>) {
        self.script_open = Some(name.to_owned());
        self.state.script_error = None;
        self.script_ui.sync_pending = true;
        self.script_ui.external = None;
        self.get(&format!("/api/scripts/{}", api::url_encode(name)), cx);
    }

    /// Save the working copy (`POST /api/scripts/{name}`).
    pub fn save_script(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.script_open.clone() else {
            return;
        };
        let code = self.current_script_code_public(cx);
        self.script_ui.save_pending = true;
        self.post(
            &format!("/api/scripts/{}", api::url_encode(&name)),
            Some(json!({ "code": code })),
            cx,
        );
    }

    /// Run the working copy (`POST /api/scripts/{name}/run`).
    pub fn run_script(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.script_open.clone() else {
            return;
        };
        let code = self.current_script_code_public(cx);
        self.script_ui.run_pending = true;
        self.post(
            &format!("/api/scripts/{}/run", api::url_encode(&name)),
            Some(json!({ "code": code })),
            cx,
        );
    }

    /// Save or run, depending on the caller. `window` is unused but keeps the
    /// signature symmetric with the click handlers.
    fn save_or_run(&mut self, _window: &mut Window, cx: &mut Context<Self>, run: bool) {
        if self.script_open.is_none() {
            return;
        }
        if run {
            self.run_script(cx);
        } else {
            self.save_script(cx);
        }
    }

    pub fn current_script_code_public(&self, cx: &Context<Self>) -> String {
        self.script_ui
            .editor
            .as_ref()
            .map(|editor| editor.read(cx).value().to_string())
            .unwrap_or_else(|| self.script_ui.code.clone())
    }

    fn script_file_action(&mut self, label: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.script_ui.file_menu_open = false;
        match label {
            "Save" => {
                self.save_script(cx);
                let _ = window;
            }
            "Create Task" => {
                self.script_ui.create_open = true;
                self.script_ui.create_class.clear();
                self.script_ui.create_name.clear();
                self.script_ui.create_desc.clear();
            }
            "Copy Task" => {
                if let Some(name) = self.script_open.clone() {
                    self.script_ui.copy_pending = true;
                    self.post(
                        &format!("/api/scripts/{}/copy", api::url_encode(&name)),
                        None,
                        cx,
                    );
                }
            }
            "Delete Task" => {
                self.script_ui.delete_target = self.script_open.clone();
            }
            "Export Script" => {
                fetch_export_options(self.client.clone());
            }
            "Import Script" => {
                pick_import_file();
            }
            _ => {}
        }
        cx.notify();
    }

    fn indent_selection(&mut self, outdent: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.script_ui.editor.clone() else {
            return;
        };
        let (value, cursor) = {
            let state = editor.read(cx);
            (state.value().to_string(), state.cursor())
        };
        let line_start = value[..cursor.min(value.len())]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        if outdent {
            let line = &value[line_start..];
            let remove = line.chars().take(4).take_while(|c| *c == ' ').count();
            if remove == 0 {
                return;
            }
            let mut new_value = String::with_capacity(value.len());
            new_value.push_str(&value[..line_start]);
            new_value.push_str(&line[remove..]);
            self.set_editor_value(new_value, Some(cursor.saturating_sub(remove)), window, cx);
        } else {
            let mut new_value = String::with_capacity(value.len() + 4);
            new_value.push_str(&value[..cursor.min(value.len())]);
            new_value.push_str("    ");
            new_value.push_str(&value[cursor.min(value.len())..]);
            self.set_editor_value(new_value, Some(cursor + 4), window, cx);
        }
        cx.notify();
    }

    fn insert_template(
        &mut self,
        template: ScriptTemplate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if template.params.is_empty() {
            self.apply_template(template, HashMap::new(), window, cx);
            return;
        }
        self.script_ui.parameter_values.clear();
        for parameter in &template.params {
            if let Some(default) = &parameter.default {
                self.script_ui
                    .parameter_values
                    .insert(parameter.name.clone(), default.clone());
            }
        }
        self.script_ui.parameter_template = Some(template);
        cx.notify();
    }

    /// Mirror of `insertTemplateValues` in `App.tsx`.
    fn apply_template(
        &mut self,
        template: ScriptTemplate,
        values: HashMap<String, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snippet = build_template_snippet(&template, &values);

        let (value, cursor) = match self.script_ui.editor.clone() {
            Some(editor) => {
                let state = editor.read(cx);
                (state.value().to_string(), state.cursor())
            }
            None => (self.script_ui.code.clone(), 0),
        };
        let cursor = cursor.min(value.len());
        let line_start = value[..cursor]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let indentation: String = value[line_start..cursor]
            .chars()
            .take_while(|character| character.is_whitespace())
            .collect();
        let indentation = if indentation.is_empty() {
            "        ".to_owned()
        } else {
            indentation
        };
        let insertion = format!(
            "{}{}{}\n",
            if cursor > line_start { "\n" } else { "" },
            indentation,
            snippet
        );
        let mut new_value = String::with_capacity(value.len() + insertion.len());
        new_value.push_str(&value[..cursor]);
        new_value.push_str(&insertion);
        new_value.push_str(&value[cursor..]);
        let new_cursor = cursor + insertion.len();
        self.script_ui.parameter_template = None;
        self.set_editor_value(new_value, Some(new_cursor), window, cx);
        cx.notify();
    }

    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        let code = self.current_script_code_public(cx);
        let loop_mode = self.script_ui.record_loop.clone();
        let count: i64 = self
            .script_ui
            .record_count
            .parse()
            .unwrap_or(10)
            .clamp(1, 999_999);
        self.script_ui.record_sync = true;
        self.post(
            "/api/scripts-record/stop",
            Some(json!({ "code": code, "loop": loop_mode, "count": count })),
            cx,
        );
    }

    // ---------------------------------------------------------------- dialogs

    fn overlay(frame: AnyElement) -> AnyElement {
        div()
            .id("script-dialog-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x0c070aad))
            .on_click(|_, _, _| {})
            .child(frame)
            .into_any_element()
    }

    fn dialog_close(&self, id: &'static str, cx: &mut Context<Self>) -> AnyElement {
        ui::icon_button(id, OkIcon::Close, cx)
            .on_click(cx.listener(|view, _, _, cx| {
                view.script_ui.parameter_template = None;
                view.script_ui.create_open = false;
                view.script_ui.export_open = false;
                view.script_ui.import_open = false;
                view.script_ui.record_open = false;
                view.script_ui.delete_target = None;
                view.script_ui.pending_script = None;
                view.script_ui.external = None;
                cx.notify();
            }))
            .into_any_element()
    }

    fn parameter_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(template) = self.script_ui.parameter_template.clone() else {
            return div().into_any_element();
        };
        let mut content = div().v_flex().gap_3();
        if !template.full_doc.is_empty() {
            content = content.child(
                div()
                    .id("template-doc")
                    .max_h(px(180.0))
                    .overflow_y_scroll()
                    .rounded(px(6.0))
                    .bg(Tokens::editor(ui::is_dark(cx)))
                    .p(px(10.0))
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(template.full_doc.clone()),
            );
        }
        let mut missing = false;
        for parameter in &template.params {
            let key = format!("template-param-{}", parameter.name);
            let current = self
                .script_ui
                .parameter_values
                .get(&parameter.name)
                .cloned()
                .unwrap_or_default();
            let placeholder = match &parameter.default {
                None => t("required"),
                Some(default) => format!("{}: {}", t("default"), default),
            };
            let input = self.ensure_input(
                window,
                cx,
                &key,
                &placeholder,
                &current,
                false,
            );
            if parameter.default.is_none() && current.trim().is_empty() {
                missing = true;
            }
            content = content.child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .text_size(px(ui::FS_BODY))
                            .child(parameter.name.clone())
                            .when(parameter.default.is_none(), |this| {
                                this.child(
                                    div().text_color(cx.theme().danger).child("*"),
                                )
                            }),
                    )
                    .child(Input::new(&input))
                    .when_some(parameter.doc.clone(), |this, doc| {
                        this.child(ui::tiny_text(doc, cx))
                    }),
            );
        }
        let frame = ui::modal_frame(560.0, cx)
            .child(
                ui::modal_header(i18n::t(&template.name), cx)
                    .child(self.dialog_close("param-close", cx)),
            )
            .child(ui::modal_body().child(content))
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("param-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.parameter_template = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button("param-insert", t("Insert"))
                            .disabled(missing)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                let mut values = HashMap::new();
                                for parameter in &template.params {
                                    let key = format!("template-param-{}", parameter.name);
                                    if let Some(value) = view.input_values.get(&key) {
                                        values.insert(parameter.name.clone(), value.clone());
                                    }
                                }
                                view.apply_template(template.clone(), values, window, cx);
                            })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let class_input = self.ensure_input(
            window,
            cx,
            "create-class",
            &t("Class Name (English only)"),
            &self.script_ui.create_class.clone(),
            false,
        );
        let name_input = self.ensure_input(
            window,
            cx,
            "create-name",
            &t("Task Name"),
            &self.script_ui.create_name.clone(),
            false,
        );
        let desc_input = self.ensure_input(
            window,
            cx,
            "create-desc",
            &t("Description (Optional)"),
            &self.script_ui.create_desc.clone(),
            false,
        );
        let class_value = self
            .input_values
            .get("create-class")
            .cloned()
            .unwrap_or_default();
        let name_value = self
            .input_values
            .get("create-name")
            .cloned()
            .unwrap_or_default();
        let valid_class = is_valid_class_name(&class_value);
        let valid = valid_class && !name_value.trim().is_empty();
        let busy = self.state.is_busy();

        let body = ui::modal_body()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_size(px(ui::FS_BODY)).child(t("Class Name")))
                    .child(Input::new(&class_input))
                    .when(!class_value.is_empty() && !valid_class, |this| {
                        this.child(
                            div()
                                .text_size(px(ui::FS_TINY))
                                .text_color(cx.theme().danger)
                                .child(t(
                                    "Use English letters, numbers, and underscores; do not start with a number.",
                                )),
                        )
                    }),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_size(px(ui::FS_BODY)).child(t("Task Name")))
                    .child(Input::new(&name_input)),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_size(px(ui::FS_BODY)).child(t("Description")))
                    .child(Input::new(&desc_input)),
            );
        let frame = ui::modal_frame(520.0, cx)
            .child(
                ui::modal_header(t("Create Task"), cx)
                    .child(self.dialog_close("create-close", cx)),
            )
            .child(body)
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("create-cancel", t("Cancel"), cx)
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.create_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button("create-ok", t("Create"))
                            .disabled(!valid || busy)
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
                                view.script_ui.create_open = false;
                                view.script_ui.create_pending = true;
                                view.post(
                                    "/api/scripts",
                                    Some(json!({
                                        "class_name": class_name,
                                        "task_name": task_name.trim(),
                                        "description": description.trim(),
                                    })),
                                    cx,
                                );
                            })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn export_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(options) = self.script_ui.export_options.clone() else {
            return div().into_any_element();
        };
        let file_input = self.ensure_input(
            window,
            cx,
            "export-file-name",
            &t("File Name:"),
            &self.script_ui.export_file_name.clone(),
            false,
        );
        let name_input = self.ensure_input(
            window,
            cx,
            "export-script-name",
            &t("Script Name:"),
            &self.script_ui.export_script_name.clone(),
            false,
        );
        let version_input = self.ensure_input(
            window,
            cx,
            "export-version",
            &t("Version:"),
            &self.script_ui.export_version.clone(),
            false,
        );
        let file_value = self
            .input_values
            .get("export-file-name")
            .cloned()
            .unwrap_or_else(|| self.script_ui.export_file_name.clone());
        let name_value = self
            .input_values
            .get("export-script-name")
            .cloned()
            .unwrap_or_else(|| self.script_ui.export_script_name.clone());
        let version_value = self
            .input_values
            .get("export-version")
            .cloned()
            .unwrap_or_else(|| self.script_ui.export_version.clone());
        let selected = self.script_ui.export_selected.clone();
        let valid = !selected.is_empty()
            && !file_value.is_empty()
            && file_value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_.-".contains(character))
            && !name_value.trim().is_empty();

        let mut tasks = div().v_flex().gap_1();
        for task in &options.tasks {
            let label = task.trim_end_matches(".py").to_owned();
            let is_selected = selected.contains(task);
            let value = task.clone();
            let current = selected.clone();
            tasks = tasks.child(
                Checkbox::new(ElementId::Name(SharedString::from(format!(
                    "export-task-{task}"
                ))))
                .checked(is_selected)
                .label(label)
                .on_click(cx.listener(move |view, checked, _, cx| {
                    let mut next = current.clone();
                    if *checked {
                        if !next.contains(&value) {
                            next.push(value.clone());
                        }
                    } else {
                        next.retain(|item| item != &value);
                    }
                    view.script_ui.export_selected = next;
                    cx.notify();
                })),
            );
        }

        let body = ui::modal_body()
            .child(
                div()
                    .text_size(px(ui::FS_BODY))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t("Select tasks to export:")),
            )
            .child(
                div()
                    .id("export-task-list")
                    .v_flex()
                    .max_h(px(200.0))
                    .gap(px(1.0))
                    .overflow_y_scroll()
                    .child(tasks),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_size(px(ui::FS_BODY)).child(t("File Name:")))
                    .child(Input::new(&file_input)),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_size(px(ui::FS_BODY)).child(t("Script Name:")))
                    .child(Input::new(&name_input)),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_size(px(ui::FS_BODY)).child(t("Version:")))
                    .child(Input::new(&version_input)),
            );
        let frame = ui::modal_frame(560.0, cx)
            .child(
                ui::modal_header(t("Export Script"), cx)
                    .child(self.dialog_close("export-close", cx)),
            )
            .child(body)
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("export-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.export_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button("export-ok", t("Export"))
                            .disabled(!valid)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                let selected = view.script_ui.export_selected.clone();
                                let file_name = view
                                    .input_values
                                    .get("export-file-name")
                                    .cloned()
                                    .unwrap_or_else(|| file_value.clone());
                                let script_name = view
                                    .input_values
                                    .get("export-script-name")
                                    .cloned()
                                    .unwrap_or_else(|| name_value.clone());
                                let version = view
                                    .input_values
                                    .get("export-version")
                                    .cloned()
                                    .unwrap_or_else(|| version_value.clone());
                                view.script_ui.export_open = false;
                                let path = "/api/scripts-export".to_owned();
                                view.state.pending.insert(path.clone());
                                api::run_post_bytes(
                                    view.client.clone(),
                                    view.queue.clone(),
                                    path,
                                    Some(json!({
                                        "tasks": selected,
                                        "file_name": file_name,
                                        "script_name": script_name.trim(),
                                        "version": version,
                                    })),
                                );
                                cx.notify();
                            })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn import_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let seconds = self.script_ui.import_seconds;
        let accepted = self.script_ui.import_accepted;
        let name = self.script_ui.import_name.clone();
        let frame = ui::modal_frame(560.0, cx)
            .child(
                ui::modal_header(t("Warning"), cx)
                    .child(self.dialog_close("import-close", cx)),
            )
            .child(
                ui::modal_body()
                    .child(
                        div()
                            .text_size(px(ui::FS_BODY))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(ui::FS_BODY))
                            .whitespace_normal()
                            .child(t(
                                "Make sure that you trust the script publisher. Unverified scripts can steal accounts or data, destroy data, or control your computer.",
                            )),
                    )
                    .child(
                        Checkbox::new("import-accept")
                            .checked(accepted)
                            .label(t("I understand the risks and want to import this script."))
                            .on_click(cx.listener(|view, checked, _, cx| {
                                view.script_ui.import_accepted = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("import-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.import_open = false;
                                view.script_ui.import_bytes = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button(
                            "import-ok",
                            if seconds > 0 {
                                format!("{} ({seconds})", t("Confirm"))
                            } else {
                                t("Confirm")
                            },
                        )
                        .disabled(!accepted || seconds > 0)
                        .on_click(cx.listener(|view, _, _, cx| {
                            let name = view.script_ui.import_name.clone();
                            let bytes = view.script_ui.import_bytes.clone();
                            view.script_ui.import_open = false;
                            if let Some(bytes) = bytes {
                                view.script_ui.import_pending = true;
                                api::run_upload(
                                    view.client.clone(),
                                    view.queue.clone(),
                                    "/api/scripts-import".to_owned(),
                                    name,
                                    bytes,
                                );
                            }
                            cx.notify();
                        })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn record_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let loop_mode = self.script_ui.record_loop.clone();
        let count = self.script_ui.record_count.clone();
        let loop_label = match loop_mode.as_str() {
            "count" => t("Loop x times"),
            "forever" => t("Loop infinitely"),
            _ => t("No loop"),
        };
        let frame = ui::modal_frame(480.0, cx)
            .child(
                ui::modal_header(t("Record"), cx)
                    .child(self.dialog_close("record-close", cx)),
            )
            .child(
                ui::modal_body()
                    .child(
                        div()
                            .text_size(px(ui::FS_BODY))
                            .whitespace_normal()
                            .child(t("Record will override the current script logic. Continue?")),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(div().text_size(px(ui::FS_BODY)).child(t("Loop")))
                            .child(
                                ui::secondary_button("record-loop", loop_label, cx)
                                    .icon(OkIcon::ChevronDown.icon())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        let next = match view.script_ui.record_loop.as_str() {
                                            "none" => "count",
                                            "count" => "forever",
                                            _ => "none",
                                        };
                                        view.script_ui.record_loop = next.to_owned();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(loop_mode == "count", |this| {
                        this.child(
                            div()
                                .v_flex()
                                .gap_1()
                                .child(div().text_size(px(ui::FS_BODY)).child(t("Count")))
                                .child(count.clone()),
                        )
                    }),
            )
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("record-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.record_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button("record-ok", t("OK")).on_click(cx.listener(
                            |view, _, _, cx| {
                                view.script_ui.record_open = false;
                                view.post("/api/scripts-record/start", None, cx);
                            },
                        )),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn delete_dialog(&mut self, name: String, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.state.is_busy();
        let frame = ui::modal_frame(420.0, cx)
            .child(
                ui::modal_header(t("Confirm Delete"), cx)
                    .child(self.dialog_close("delete-close", cx)),
            )
            .child(ui::modal_body().child(
                div()
                    .text_size(px(ui::FS_BODY))
                    .whitespace_normal()
                    .child(i18n::tv(
                        "Are you sure you want to delete '{name}'?",
                        &[("name", &name)],
                    )),
            ))
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("delete-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.delete_target = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::primary_button("delete-ok", t("Delete"))
                            .disabled(busy)
                            .on_click(cx.listener({
                                let name = name.clone();
                                move |view, _, window, cx| {
                                    view.script_ui.delete_target = None;
                                    view.script_ui.delete_pending = true;
                                    if view.script_open.as_deref() == Some(name.as_str()) {
                                        view.script_open = None;
                                        view.state.script_document = None;
                                        view.state.script_code.clear();
                                        view.script_ui.code.clear();
                                        view.script_ui.dirty = false;
                                        view.state.script_dirty = false;
                                        view.set_editor_value(
                                            String::new(),
                                            None,
                                            window,
                                            cx,
                                        );
                                    }
                                    view.post(
                                        &format!(
                                            "/api/scripts/{}/delete",
                                            api::url_encode(&name)
                                        ),
                                        None,
                                        cx,
                                    );
                                }
                            })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn external_dialog(&mut self, document: ScriptDocument, cx: &mut Context<Self>) -> AnyElement {
        let frame = ui::modal_frame(560.0, cx)
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
                            .on_click(cx.listener({
                                let modified = document.modified;
                                move |view, _, _, cx| {
                                    view.script_ui.loaded_modified = modified;
                                    view.script_ui.external = None;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        ui::primary_button("external-reload", t("Reload"))
                            .on_click(cx.listener({
                                let code = document.code.clone();
                                let modified = document.modified;
                                move |view, _, window, cx| {
                                    view.script_ui.external = None;
                                    view.script_ui.loaded_modified = modified;
                                    view.set_editor_value(code.clone(), None, window, cx);
                                    cx.notify();
                                }
                            })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    fn unsaved_dialog(&mut self, name: String, cx: &mut Context<Self>) -> AnyElement {
        let frame = ui::modal_frame(520.0, cx)
            .child(ui::modal_header(t("Save Changes"), cx))
            .child(ui::modal_body().child(
                div()
                    .text_size(px(ui::FS_BODY))
                    .whitespace_normal()
                    .child(t("The current task has unsaved changes. Do you want to save them?")),
            ))
            .child(
                ui::modal_footer()
                    .child(
                        ui::secondary_button("unsaved-cancel", t("Cancel"), cx)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.script_ui.pending_script = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::secondary_button("unsaved-discard", t("Don't Save"), cx)
                            .on_click(cx.listener({
                                let name = name.clone();
                                move |view, _, _, cx| {
                                    view.script_ui.pending_script = None;
                                    view.script_ui.dirty = false;
                                    view.state.script_dirty = false;
                                    view.open_script(&name, cx);
                                }
                            })),
                    )
                    .child(
                        ui::primary_button("unsaved-save", t("Save")).on_click(cx.listener({
                            let name = name.clone();
                            move |view, _, _, cx| {
                                view.save_script(cx);
                                view.script_ui.pending_script = None;
                                view.script_ui.dirty = false;
                                view.state.script_dirty = false;
                                view.open_script(&name, cx);
                            }
                        })),
                    ),
            );
        Self::overlay(frame.into_any_element())
    }

    /// The shell polls `/api/scripts/{name}` for the open document; this hook
    /// lets the page react to the fetched document (sync or external change).
    pub fn poll_script(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tick_script(window, cx);
    }
}

// ------------------------------------------------------------------ helpers

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn is_valid_class_name(value: &str) -> bool {
    is_identifier(value)
}

fn json_string(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

fn clamp_boundary(value: &str, offset: usize) -> usize {
    let mut offset = offset.min(value.len());
    while offset > 0 && !value.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Byte offset → `lsp_types::Position` (line/character, both 0-based chars).
fn position_for_offset(value: &str, offset: usize) -> Position {
    let offset = clamp_boundary(value, offset);
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (index, character) in value.char_indices() {
        if index >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            line_start = index + character.len_utf8();
        }
    }
    let character = value[line_start..offset].chars().count() as u32;
    Position::new(line, character)
}

fn extract_error_line(error: &str) -> Option<u32> {
    let marker = "line ";
    let index = error.to_lowercase().find(marker)? + marker.len();
    let digits: String = error[index..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn manifest_value(options: &ScriptExportOptions, key: &str) -> Option<String> {
    options
        .manifest
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn fetch_export_options(client: std::sync::Arc<api::ApiClient>) {
    std::thread::spawn(move || {
        let Ok(bytes) = client.get_bytes("/api/scripts-export/options") else {
            return;
        };
        if let Ok(options) = serde_json::from_slice::<ScriptExportOptions>(&bytes) {
            if let Ok(mut guard) = EXPORT_BRIDGE.lock() {
                *guard = Some(options);
            }
        }
    });
}

fn pick_import_file() {
    std::thread::spawn(move || {
        let picked = rfd::FileDialog::new()
            .add_filter("ok-script", &["okscript"])
            .set_title(t("Import Script"))
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "script.okscript".to_owned());
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        if let Ok(mut guard) = IMPORT_BRIDGE.lock() {
            *guard = Some((name, bytes));
        }
    });
}

/// The exact call expression `insertTemplateValues` produces: skip empty
/// values, JSON-quote bare identifiers (except the Python literals), keep
/// `name=value` for parameters that declare a default, and pick `self.` vs
/// `Class.` from `is_static`.
fn build_template_snippet(template: &ScriptTemplate, values: &HashMap<String, String>) -> String {
    let mut args: Vec<String> = Vec::new();
    for parameter in &template.params {
        let mut value = values
            .get(&parameter.name)
            .map(|value| value.trim().to_owned())
            .unwrap_or_default();
        if value.is_empty() {
            continue;
        }
        if is_identifier(&value) && !matches!(value.as_str(), "True" | "False" | "None") {
            value = json_string(&value);
        }
        if parameter.default.is_none() {
            args.push(value);
        } else {
            args.push(format!("{}={}", parameter.name, value));
        }
    }
    let prefix = if template.is_static {
        format!("{}.", template.class_name)
    } else {
        "self.".to_owned()
    };
    format!("{prefix}{}({})", template.name, args.join(", "))
}

/// The exact insertion the web editor performs at the cursor line.
fn build_template_insertion(document: &str, cursor: usize, snippet: &str) -> (String, usize) {
    let cursor = clamp_boundary(document, cursor.min(document.len()));
    let line_start = document[..cursor]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let indentation: String = document[line_start..cursor]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect();
    let indentation = if indentation.is_empty() {
        "        ".to_owned()
    } else {
        indentation
    };
    let insertion = format!(
        "{}{}{}\n",
        if cursor > line_start { "\n" } else { "" },
        indentation,
        snippet
    );
    let mut value = String::with_capacity(document.len() + insertion.len());
    value.push_str(&document[..cursor]);
    value.push_str(&insertion);
    value.push_str(&document[cursor..]);
    let new_cursor = cursor + insertion.len();
    (value, new_cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(name: &str) -> ScriptTemplate {
        ScriptTemplate {
            name: name.to_owned(),
            ..Default::default()
        }
    }

    fn parameter(name: &str, default: Option<&str>) -> ScriptParameter {
        ScriptParameter {
            name: name.to_owned(),
            default: default.map(str::to_owned),
            doc: None,
        }
    }

    fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn snippet_without_params_is_a_plain_self_call() {
        assert_eq!(
            build_template_snippet(&template("click"), &values(&[])),
            "self.click()"
        );
    }

    #[test]
    fn static_templates_use_the_class_prefix() {
        let mut item = template("find");
        item.is_static = true;
        item.class_name = "ExecutorOperation".to_owned();
        assert_eq!(
            build_template_snippet(&item, &values(&[])),
            "ExecutorOperation.find()"
        );
    }

    #[test]
    fn required_values_are_positional_and_quoted() {
        let mut item = template("tap");
        item.params = vec![parameter("text", None)];
        assert_eq!(
            build_template_snippet(&item, &values(&[("text", "Start")])),
            "self.tap(\"Start\")"
        );
    }

    #[test]
    fn defaulted_values_are_passed_as_keywords() {
        let mut item = template("tap");
        item.params = vec![parameter("text", Some("Start"))];
        assert_eq!(
            build_template_snippet(&item, &values(&[("text", "Go")])),
            "self.tap(text=\"Go\")"
        );
    }

    #[test]
    fn python_literals_and_numbers_are_not_quoted() {
        let mut item = template("press");
        item.params = vec![
            parameter("key", Some("None")),
            parameter("times", Some("1")),
        ];
        assert_eq!(
            build_template_snippet(&item, &values(&[("key", "True"), ("times", "3")])),
            "self.press(key=True, times=3)"
        );
    }

    #[test]
    fn empty_values_are_skipped() {
        let mut item = template("tap");
        item.params = vec![parameter("a", Some("1")), parameter("b", None)];
        assert_eq!(
            build_template_snippet(&item, &values(&[("a", "   ")])),
            "self.tap()"
        );
    }

    #[test]
    fn insertion_uses_the_line_indentation_and_adds_a_leading_newline() {
        let document = "def run(self):\n        pass\n";
        let cursor = document.find("pass").unwrap();
        let (updated, new_cursor) = build_template_insertion(document, cursor, "self.click()");
        assert_eq!(
            updated,
            "def run(self):\n        \n        self.click()\npass\n"
        );
        // The cursor lands at the end of the inserted snippet, i.e. on the
        // line that follows it.
        assert_eq!(new_cursor, updated.find("pass").unwrap());
    }

    #[test]
    fn insertion_at_the_line_start_keeps_the_line() {
        let document = "        pass\n";
        let (updated, _) = build_template_insertion(document, 0, "self.click()");
        assert_eq!(updated, "        self.click()\n        pass\n");
    }

    #[test]
    fn error_lines_are_extracted_like_the_web_editor() {
        assert_eq!(
            extract_error_line("SyntaxError: invalid syntax (line 12)"),
            Some(12)
        );
        assert_eq!(extract_error_line("LINE 3: bad indent"), Some(3));
        assert_eq!(extract_error_line("no line here"), None);
    }

    #[test]
    fn class_names_must_be_identifiers() {
        assert!(is_valid_class_name("MyTask"));
        assert!(is_valid_class_name("_my_task2"));
        assert!(!is_valid_class_name("2Task"));
        assert!(!is_valid_class_name("My-Task"));
        assert!(!is_valid_class_name(""));
    }

    #[test]
    fn positions_follow_the_lsp_convention() {
        let document = "a\nbc\ndef";
        assert_eq!(position_for_offset(document, 0), Position::new(0, 0));
        assert_eq!(position_for_offset(document, 2), Position::new(1, 0));
        assert_eq!(position_for_offset(document, 4), Position::new(1, 2));
        assert_eq!(position_for_offset(document, 100), Position::new(2, 3));
    }
}
