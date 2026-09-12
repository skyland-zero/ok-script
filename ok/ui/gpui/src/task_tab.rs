//! Custom task tab pages — mirrors the web frontend's `TaskTabHost.tsx`.
//!
//! A task can declare a browser module (`WebTabConfig`) whose entry point
//! exports `mount(container, context)`. Native code cannot run that module, so
//! the host embeds a WebView and injects the same context bridge the browser
//! host builds: `query`/`action`/`task.getState|start|pause|resume|stop|
//! setConfig`/`subscribe`/`notify`/`t`/`locale`/`theme`/`setDirty`/
//! `registerSave`, plus the `ok-task-tab-event` bus that runtime events are
//! forwarded into.
//!
//! Two hosting strategies:
//!
//! * `manifest.gpui_view` present — a declarative control tree rendered
//!   natively (a native opt-in the web contract has no equivalent for).
//! * otherwise — the module is hosted in a WebView loaded at the tab's own
//!   asset URL, so relative imports inside the module keep working and the
//!   same-origin rule of the (CORS-less) backend is respected.
//!
//! Everything needed is reached through `gpui_component`'s re-exports
//! (`gpui_component::webview::WebView` for the widget, `gpui_component::wry`
//! for the builder), so this module adds no crate dependency.

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, Sender};

use gpui::{
    div, prelude::*, px, AnyElement, Context, ElementId, Entity, IntoElement, ParentElement,
    SharedString, Styled, Window,
};
use gpui_component::{
    webview::WebView,
    wry::{self, http::Request, raw_window_handle::HasWindowHandle},
    ActiveTheme as _, Disableable as _, StyledExt as _,
};
use serde_json::{json, Value};

use crate::api;
use crate::app::OkApp;
use crate::components as ui;
use crate::i18n::t;
use crate::icons::OkIcon;
use crate::model::{TaskTabManifest, ToastKind};

/// Messages the hosted page sends back over `window.ipc.postMessage`.
#[derive(Clone, Debug, PartialEq)]
enum TaskTabMessage {
    /// The module mounted successfully.
    Loaded,
    /// The module failed to load or threw while mounting.
    Error(String),
    /// `context.setDirty(value)`.
    Dirty(bool),
    /// `context.notify(message, intent)`.
    Notify { intent: ToastKind, message: String },
}

impl TaskTabMessage {
    fn parse(body: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(body).ok()?;
        let kind = value.get("type").and_then(Value::as_str)?;
        Some(match kind {
            "loaded" => Self::Loaded,
            "error" => Self::Error(
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
            "dirty" => Self::Dirty(value.get("dirty").and_then(Value::as_bool).unwrap_or(false)),
            "notify" => Self::Notify {
                intent: match value.get("intent").and_then(Value::as_str) {
                    Some("success") => ToastKind::Success,
                    Some("error") => ToastKind::Error,
                    _ => ToastKind::Info,
                },
                message: value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            },
            _ => return None,
        })
    }
}

/// Host state for the custom tab page.
pub struct TaskTabState {
    /// Manifest id the live WebView belongs to.
    pub loaded: Option<String>,
    /// Set when the module could not be hosted; rendered as the web host's
    /// `role="alert"` line.
    pub error: Option<String>,
    /// `context.setDirty` state reported by the page.
    pub dirty: bool,
    /// The hosted module; kept alive across page switches (hidden while away).
    pub webview: Option<Entity<WebView>>,
    /// `task_tab` runtime events waiting to be forwarded into the page.
    pub queued_events: VecDeque<(String, String, Value)>,
    sender: Sender<TaskTabMessage>,
    receiver: Receiver<TaskTabMessage>,
}

impl Default for TaskTabState {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            loaded: None,
            error: None,
            dirty: false,
            webview: None,
            queued_events: VecDeque::new(),
            sender,
            receiver,
        }
    }
}

impl OkApp {
    /// Render the custom tab page for `id`.
    pub fn render_task_tab(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.drain_task_tab_messages(cx);
        let Some(tab) = self.task_tab_manifest(id) else {
            return ui::page_root("task-tab-page")
                .child(ui::muted_text(t("Loading"), cx))
                .into_any_element();
        };
        let title = if tab.name.is_empty() {
            tab.id.clone()
        } else {
            crate::i18n::t(&tab.name)
        };

        if let Some(tree) = tab.gpui_view.clone() {
            let body = self.render_control_node(&tab.id, &tree, cx);
            return ui::page_root("task-tab-page")
                .child(ui::page_title(title, cx))
                .child(ui::card(cx).gap_2().p(px(16.0)).child(body))
                .into_any_element();
        }

        if tab.module_url.is_empty() {
            return self.task_tab_fallback(
                &tab,
                &title,
                &t("This task tab exposes Web assets only."),
                cx,
            );
        }

        if self.task_tab.error.is_none()
            && self.task_tab.loaded.as_deref() != Some(tab.id.as_str())
        {
            // A different tab (or none) owned the view: run the module's
            // cleanup, then retire the child window so it never overlaps a page
            // it does not belong to.
            if let Some(webview) = self.task_tab.webview.clone() {
                webview.update(cx, |view, _| {
                    let _ = view.evaluate_script(
                        "window.__okTaskTabCleanup && window.__okTaskTabCleanup(); \
                         window.__okTaskTabCleanup = null;",
                    );
                });
                self.hide_task_tab(cx);
            }
            self.task_tab.webview = None;
            self.task_tab.loaded = None;
            self.task_tab.dirty = false;
            self.ensure_task_tab_webview(&tab, window, cx);
        }

        if let Some(error) = self.task_tab.error.clone() {
            let message = format!("{}: {error}", t("Could not load custom tab"));
            return self.task_tab_fallback(&tab, &title, &message, cx);
        }

        self.flush_task_tab_events(cx);

        let Some(webview) = self.task_tab.webview.clone() else {
            return self.task_tab_fallback(
                &tab,
                &title,
                &t("This task tab exposes Web assets only."),
                cx,
            );
        };
        if !webview.read(cx).visible() {
            self.show_task_tab(cx);
        }

        ui::page_root("task-tab-page")
            .child(ui::page_title(title, cx))
            .child(
                div()
                    .w_full()
                    .h(px((f32::from(window.viewport_size().height) - 180.0).max(320.0)))
                    .rounded(px(ui::CARD_RADIUS))
                    .overflow_hidden()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(webview),
            )
            .into_any_element()
    }

    /// Queue one `task_tab` runtime event for the hosted page.
    ///
    /// The shell owns event delivery (`OkApp::apply_event` in `app.rs`); call
    /// this from the `"task_tab"` arm with the event's
    /// `(tab_id, name, payload)`. Queued events are forwarded on the next
    /// render of the tab page.
    pub fn push_task_tab_event(&mut self, tab_id: &str, name: &str, payload: Value) {
        self.task_tab
            .queued_events
            .push_back((tab_id.to_owned(), name.to_owned(), payload));
        while self.task_tab.queued_events.len() > 64 {
            self.task_tab.queued_events.pop_front();
        }
    }

    /// `context.setDirty` state of the hosted page; the shell's navigation
    /// guard should treat it like the script page's dirty flag.
    #[allow(dead_code)]
    pub fn task_tab_dirty(&self) -> bool {
        self.task_tab.dirty
    }

    /// Invoke the page's registered save handler.
    #[allow(dead_code)]
    pub fn save_task_tab(&mut self, cx: &mut Context<Self>) {
        let Some(webview) = self.task_tab.webview.clone() else {
            return;
        };
        webview.update(cx, |view, _| {
            let _ = view.evaluate_script("window.__okTaskTabSave ? window.__okTaskTabSave() : false");
        });
        self.task_tab.dirty = false;
        cx.notify();
    }

    /// Hide the hosted page's child window. Call this from the shell whenever
    /// the active page stops being this tab (`OkApp::set_page` in `app.rs`),
    /// because a WebView is an OS child window that is not clipped by GPUI.
    pub fn hide_task_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(webview) = self.task_tab.webview.clone() {
            webview.update(cx, |view, _| view.hide());
        }
    }

    /// Show the hosted page again (used when the tab page re-renders).
    pub fn show_task_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(webview) = self.task_tab.webview.clone() {
            webview.update(cx, |view, _| view.show());
        }
    }

    // ------------------------------------------------------------ hosting

    fn task_tab_manifest(&self, id: &str) -> Option<TaskTabManifest> {
        self.state
            .navigation
            .task_tabs
            .iter()
            .find(|tab| tab.id == id)
            .cloned()
    }

    fn ensure_task_tab_webview(
        &mut self,
        tab: &TaskTabManifest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<WebView>> {
        let module_url = absolute_url(&self.client, &tab.module_url);
        let document_url = document_url_for(&module_url);
        let script = self.host_script(tab, &module_url, cx);
        let sender = self.task_tab.sender.clone();

        let mut builder = wry::WebViewBuilder::new()
            .with_url(document_url)
            .with_initialization_script(script)
            .with_ipc_handler(move |request: Request<String>| {
                if let Some(message) = TaskTabMessage::parse(request.body()) {
                    let _ = sender.send(message);
                }
            });
        if cfg!(debug_assertions) {
            builder = builder.with_devtools(true);
        }

        let handle = match window.window_handle() {
            Ok(handle) => handle,
            Err(error) => {
                self.task_tab.error = Some(error.to_string());
                return None;
            }
        };
        let raw = match builder.build_as_child(&handle) {
            Ok(raw) => raw,
            Err(error) => {
                self.task_tab.error = Some(error.to_string());
                return None;
            }
        };
        drop(handle);

        let webview = cx.new(|cx| WebView::new(raw, window, cx));
        self.task_tab.webview = Some(webview.clone());
        self.task_tab.loaded = Some(tab.id.clone());
        self.task_tab.error = None;
        Some(webview)
    }

    fn drain_task_tab_messages(&mut self, cx: &mut Context<Self>) {
        let messages: Vec<TaskTabMessage> = self.task_tab.receiver.try_iter().collect();
        if messages.is_empty() {
            return;
        }
        for message in messages {
            match message {
                TaskTabMessage::Loaded => {
                    self.task_tab.error = None;
                }
                TaskTabMessage::Error(error) => {
                    self.task_tab.error = Some(error);
                }
                TaskTabMessage::Dirty(dirty) => {
                    self.task_tab.dirty = dirty;
                }
                TaskTabMessage::Notify { intent, message } => {
                    if !message.is_empty() {
                        self.toast(intent, message);
                    }
                }
            }
        }
        cx.notify();
    }

    fn flush_task_tab_events(&mut self, cx: &mut Context<Self>) {
        if self.task_tab.queued_events.is_empty() {
            return;
        }
        let Some(webview) = self.task_tab.webview.clone() else {
            return;
        };
        let events: Vec<(String, String, Value)> =
            self.task_tab.queued_events.drain(..).collect();
        for (tab_id, name, payload) in events {
            let detail = json!({ "tab_id": tab_id, "name": name, "payload": payload });
            let script = format!(
                "window.__okTaskTabEvent && window.__okTaskTabEvent({});",
                detail
            );
            webview.update(cx, |view, _| {
                let _ = view.evaluate_script(&script);
            });
        }
    }

    /// The page shown when a module cannot be hosted: the web host's failure
    /// wording plus a link into the browser UI, where the tab is available.
    fn task_tab_fallback(
        &self,
        tab: &TaskTabManifest,
        title: &str,
        message: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let browser_url = self.client.url("/");
        let can_open_browser = !tab.module_url.is_empty();
        ui::page_root("task-tab-page")
            .child(ui::page_title(title.to_owned(), cx))
            .child(
                ui::card(cx)
                    .gap_2()
                    .p(px(16.0))
                    .child(
                        div()
                            .text_size(px(ui::FS_SMALL))
                            .text_color(cx.theme().danger)
                            .child(message.to_owned()),
                    )
                    .when(can_open_browser, |this| {
                        this.child(
                            ui::secondary_button(
                                ElementId::Name(SharedString::from(format!(
                                    "task-tab-{}-browser",
                                    tab.id
                                ))),
                                t("Open in browser"),
                                cx,
                            )
                            .icon(OkIcon::Globe.icon())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.open_url(&browser_url);
                                view.toast(ToastKind::Info, t("Open in browser"));
                            })),
                        )
                    }),
            )
            .into_any_element()
    }

    /// Build the initialization script that reproduces `TaskTabHost.tsx` in the
    /// WebView. Placeholders keep the JavaScript readable and unescaped.
    fn host_script(
        &self,
        tab: &TaskTabManifest,
        module_url: &str,
        cx: &gpui::App,
    ) -> String {
        let manifest = json!({
            "id": tab.id,
            "name": tab.name,
            "icon": tab.icon,
            "position": tab.position,
            "add_after_default_tabs": tab.add_after_default_tabs,
            "task_controls": tab.task_controls,
            "task_name": tab.task_name,
            "task_class_name": tab.task_class_name,
            "module_url": tab.module_url,
        });
        let mut catalog = serde_json::Map::new();
        for key in crate::i18n::catalog_keys() {
            catalog.insert(key.clone(), Value::String(t(&key)));
        }
        let colors = json!({
            "background": css_color(cx.theme().background),
            "foreground": css_color(cx.theme().foreground),
            "accent": css_color(cx.theme().accent),
            "danger": css_color(cx.theme().danger),
            "border": css_color(cx.theme().border),
        });
        let theme = if matches!(cx.theme().mode, gpui_component::ThemeMode::Dark) {
            "dark"
        } else {
            "light"
        };

        HOST_SCRIPT
            .replace("__OK_TAB__", &manifest.to_string())
            .replace("__OK_MODULE__", &json_string(module_url))
            .replace("__OK_BASE__", &json_string(self.client.url("/").trim_end_matches('/')))
            .replace("__OK_CATALOG__", &Value::Object(catalog).to_string())
            .replace("__OK_COLORS__", &colors.to_string())
            .replace("__OK_THEME__", &json_string(theme))
            .replace("__OK_LOCALE__", &json_string(crate::i18n::locale()))
    }

    // ------------------------------------------------------ control tree

    /// Render a `gpui_view` control tree (native task tab opt-in).
    fn render_control_node(
        &mut self,
        tab_id: &str,
        node: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(nodes) = node.as_array() {
            let children: Vec<AnyElement> = nodes
                .iter()
                .map(|child| self.render_control_node(tab_id, child, cx))
                .collect();
            return div()
                .v_flex()
                .w_full()
                .gap_2()
                .children(children)
                .into_any_element();
        }
        let Some(object) = node.as_object() else {
            return ui::body_text(display_value(node), cx).into_any_element();
        };
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("text")
            .to_owned();
        let text = object
            .get("text")
            .or_else(|| object.get("label"))
            .or_else(|| object.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let children: Vec<AnyElement> = object
            .get("children")
            .or_else(|| object.get("controls"))
            .and_then(Value::as_array)
            .map(|nodes| {
                nodes
                    .iter()
                    .map(|child| self.render_control_node(tab_id, child, cx))
                    .collect()
            })
            .unwrap_or_default();

        if kind == "button" {
            return self.render_control_button(tab_id, object, &text, cx);
        }

        match kind.as_str() {
            "heading" | "title" => ui::section_title(text, cx).into_any_element(),
            "separator" => div()
                .h(px(1.0))
                .w_full()
                .bg(cx.theme().border)
                .into_any_element(),
            "row" => div()
                .h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .children(children)
                .into_any_element(),
            "column" | "stack" | "group" | "panel" => {
                let panel = div()
                    .v_flex()
                    .w_full()
                    .gap_2()
                    .p(px(12.0))
                    .rounded(px(ui::CARD_RADIUS))
                    .bg(if kind == "panel" {
                        cx.theme().popover
                    } else {
                        cx.theme().list
                    });
                if text.is_empty() {
                    panel.children(children).into_any_element()
                } else {
                    let mut nodes = vec![ui::body_text(text, cx).into_any_element()];
                    nodes.extend(children);
                    panel.children(nodes).into_any_element()
                }
            }
            "muted" => ui::muted_text(text, cx).into_any_element(),
            _ => {
                let value = if text.is_empty() {
                    display_value(node)
                } else {
                    text
                };
                ui::body_text(value, cx).into_any_element()
            }
        }
    }

    fn render_control_button(
        &mut self,
        tab_id: &str,
        object: &serde_json::Map<String, Value>,
        text: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(action) = object.get("action").and_then(Value::as_object) else {
            return ui::muted_text(text.to_owned(), cx).into_any_element();
        };
        let operation = action
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if operation.is_empty() {
            return ui::muted_text(text.to_owned(), cx).into_any_element();
        }
        let channel = action
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("action")
            .to_owned();
        let body = action
            .get("body")
            .cloned()
            .or_else(|| object.get("body").cloned());
        let path = format!(
            "/api/task-tabs/{}/{}/{}",
            api::url_encode(tab_id),
            api::url_encode(&channel),
            api::url_encode(&operation)
        );
        let id = ElementId::Name(SharedString::from(format!(
            "task-tab-{tab_id}-{channel}-{operation}"
        )));
        let busy = self.state.is_busy();
        let primary = object
            .get("primary")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let label = text.to_owned();
        let mut button = if primary {
            ui::primary_button(id, label)
        } else {
            ui::secondary_button(id, label, cx)
        };
        if busy {
            button = button.disabled(true);
        }
        button
            .on_click(cx.listener(move |view, _, _, cx| {
                view.post(&path, body.clone(), cx);
            }))
            .into_any_element()
    }
}

/// `TaskTabManifest.module_url` may be relative; the host needs an absolute URL
/// so the injected module import resolves against the API origin.
fn absolute_url(client: &crate::api::ApiClient, module_url: &str) -> String {
    if module_url.starts_with("http://") || module_url.starts_with("https://") {
        return module_url.to_owned();
    }
    if module_url.starts_with('/') {
        return client.url(module_url);
    }
    client.url(&format!("/{module_url}"))
}

/// The document URL the WebView is navigated to: the directory that owns the
/// module, so `import()` and relative asset requests stay same-origin (the
/// backend serves no CORS headers, so cross-origin module imports would fail).
fn document_url_for(module_url: &str) -> String {
    let authority_end = module_url
        .find("://")
        .map(|index| index + 3)
        .unwrap_or_default();
    match module_url.rfind('/') {
        // Never cut into the scheme/authority (`http://host` has no directory).
        Some(index) if index > authority_end => module_url[..=index].to_owned(),
        _ => format!("{}/", module_url.trim_end_matches('/')),
    }
}

fn json_string(value: &str) -> String {
    Value::String(value.to_owned()).to_string()
}

fn css_color(color: gpui::Hsla) -> String {
    format!(
        "hsla({:.0}, {:.0}%, {:.0}%, {:.2})",
        color.h * 360.0,
        color.s * 100.0,
        color.l * 100.0,
        color.a
    )
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

/// The WebView host page: a JavaScript port of `TaskTabHost.tsx` that mounts the
/// task module and exposes the same context object.
const HOST_SCRIPT: &str = r##"
(function () {
  "use strict";
  var TAB = __OK_TAB__;
  var MODULE_URL = __OK_MODULE__;
  var BASE = __OK_BASE__;
  var CATALOG = __OK_CATALOG__;
  var COLORS = __OK_COLORS__;
  var THEME = __OK_THEME__;
  var LOCALE = __OK_LOCALE__;
  var EVENT_NAME = "ok-task-tab-event";

  function t(key) {
    return Object.prototype.hasOwnProperty.call(CATALOG, key) ? CATALOG[key] : key;
  }

  function toHost(message) {
    try {
      if (window.ipc && window.ipc.postMessage) {
        window.ipc.postMessage(JSON.stringify(message));
      }
    } catch (error) {}
  }

  async function request(method, path, body) {
    var options = { method: method, headers: { "Content-Type": "application/json" } };
    if (body !== undefined) {
      options.body = JSON.stringify(body);
    }
    var response = await fetch(BASE + path, options);
    var text = await response.text();
    var value = text ? JSON.parse(text) : null;
    if (!response.ok) {
      throw new Error(value && value.detail ? value.detail : "HTTP " + response.status);
    }
    return value;
  }

  function taskPath(suffix) {
    return "/api/tasks/" + encodeURIComponent(TAB.task_name) + suffix;
  }

  async function getState() {
    var tasks = await request("GET", "/api/tasks");
    var task = (tasks || []).find(function (candidate) {
      return candidate.class_name === TAB.task_class_name;
    });
    if (!task) {
      throw new Error("Task is no longer registered: " + TAB.task_class_name);
    }
    return task;
  }

  function act(action) {
    return request("POST", taskPath("/action"), { action: action });
  }

  var task = {
    getState: getState,
    start: function () { return request("POST", taskPath("/start")); },
    pause: function () { return act("pause"); },
    resume: function () { return act("resume"); },
    stop: async function () {
      var state = await getState();
      return state.trigger ? act("disable") : act("stop");
    },
    setConfig: function (key, value) {
      return request("POST", taskPath("/config"), { key: key, value: value });
    }
  };

  var context = {
    tab: TAB,
    query: function (name, input) {
      return request(
        "POST",
        "/api/task-tabs/" + encodeURIComponent(TAB.id) + "/query/" + encodeURIComponent(name),
        input || {}
      );
    },
    action: function (name, input) {
      return request(
        "POST",
        "/api/task-tabs/" + encodeURIComponent(TAB.id) + "/action/" + encodeURIComponent(name),
        input || {}
      );
    },
    task: TAB.task_controls ? task : undefined,
    subscribe: function (handler) {
      var listener = function (event) {
        var detail = event.detail;
        if (detail && detail.tab_id === TAB.id) {
          handler(detail);
        }
      };
      window.addEventListener(EVENT_NAME, listener);
      return function () {
        window.removeEventListener(EVENT_NAME, listener);
      };
    },
    notify: function (message, intent) {
      toHost({ type: "notify", intent: intent || "info", message: String(message) });
    },
    t: t,
    locale: LOCALE,
    theme: THEME,
    setDirty: function (dirty) {
      toHost({ type: "dirty", dirty: !!dirty });
    },
    registerSave: function (save) {
      window.__okTaskTabSave = typeof save === "function" ? save : null;
    }
  };

  window.__okTaskTabEvent = function (detail) {
    try {
      window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: detail }));
    } catch (error) {}
  };

  function mount() {
    document.documentElement.dataset.theme = THEME;
    document.body.innerHTML = "";
    document.body.style.margin = "0";
    document.body.style.background = COLORS.background;
    document.body.style.color = COLORS.foreground;
    document.body.style.fontFamily =
      "\"Segoe UI Variable\", \"Segoe UI\", \"Microsoft YaHei UI\", system-ui";
    var host = document.createElement("div");
    host.id = "ok-task-tab";
    host.className = "task-tab-host";
    document.body.appendChild(host);
    return host;
  }

  function start() {
    var host = mount();
    import(MODULE_URL)
      .then(function (module) {
        if (typeof module.mount !== "function") {
          throw new Error("Task tab module does not export mount(): " + MODULE_URL);
        }
        var cleanup = module.mount(host, context);
        window.__okTaskTabCleanup = typeof cleanup === "function" ? cleanup : null;
        toHost({ type: "loaded" });
      })
      .catch(function (reason) {
        var message = reason && reason.message ? reason.message : String(reason);
        host.textContent = t("Could not load custom tab") + ": " + message;
        host.style.color = COLORS.danger;
        toHost({ type: "error", message: message });
      });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start, { once: true });
  } else {
    start();
  }
})();
"##;
