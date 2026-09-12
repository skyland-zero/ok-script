//! HTTP + WebSocket client for the shared ok-script Web API.
//!
//! The blocking `reqwest` client runs on dedicated threads so the GPUI
//! foreground executor is never blocked; results are pushed into a queue that
//! the shell drains on a timer (250 ms), mirroring the web frontend's
//! event-plus-polling model.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tungstenite::{connect, Message};

use crate::model::Update;

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    http: Client,
}

impl ApiClient {
    pub fn new(base_url: String) -> Result<Self, String> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        let http = Client::builder()
            // The backend is always local; never route it through a system proxy.
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("HTTP client: {error}"))?;
        Ok(Self { base_url, http })
    }

    pub fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            // Callers may already hold an absolute URL (image resources report
            // them that way); never prefix those twice.
            return path.to_owned();
        }
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let response = self
            .http
            .get(self.url(path))
            .send()
            .map_err(|error| error.to_string())?;
        decode_json(response)
    }

    fn get_value(&self, path: &str) -> Result<Value, String> {
        self.get::<Value>(path)
    }

    pub fn get_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        let response = self
            .http
            .get(self.url(path))
            .send()
            .map_err(|error| error.to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| error.to_string())
    }

    fn post_value(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        let request = self.http.post(self.url(path));
        let response = match body {
            Some(body) => request.json(&body).send(),
            None => request.send(),
        }
        .map_err(|error| error.to_string())?;
        decode_json(response)
    }

    fn post_raw(
        &self,
        path: &str,
        body: Option<Vec<u8>>,
        headers: &[(&str, &str)],
    ) -> Result<Vec<u8>, String> {
        let mut request = self.http.post(self.url(path));
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/octet-stream")
                .body(body);
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().map_err(|error| error.to_string())?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(error_message(status.as_u16(), &body));
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| error.to_string())
    }

    pub fn event_url(&self, session_key: &str) -> String {
        let websocket_base = if self.base_url.starts_with("https://") {
            self.base_url.replacen("https://", "wss://", 1)
        } else {
            self.base_url.replacen("http://", "ws://", 1)
        };
        format!(
            "{websocket_base}/api/events?session_key={}",
            url_encode(session_key)
        )
    }
}

/// Web frontend error convention: prefer `detail`, then a bare status.
fn error_message(status: u16, body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(detail) = value.get("detail").and_then(Value::as_str) {
            return detail.to_owned();
        }
        if let Some(detail) = value.get("detail") {
            return detail.to_string();
        }
    }
    if body.trim().is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {body}")
    }
}

fn decode_json<T: DeserializeOwned>(response: reqwest::blocking::Response) -> Result<T, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(error_message(status.as_u16(), &body));
    }
    let text = response.text().map_err(|error| error.to_string())?;
    if text.trim().is_empty() {
        return serde_json::from_str::<Value>("null")
            .map_err(|error| error.to_string())
            .and_then(|value| {
                serde_json::from_value::<T>(value).map_err(|error| error.to_string())
            });
    }
    serde_json::from_str::<T>(&text).map_err(|error| format!("{error}: {text}"))
}

pub fn url_encode(value: &str) -> String {
    value.bytes().fold(String::new(), |mut output, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
        output
    })
}

fn push(queue: &Arc<Mutex<VecDeque<Update>>>, update: Update) {
    if let Ok(mut queue) = queue.lock() {
        queue.push_back(update);
        while queue.len() > 256 {
            queue.pop_front();
        }
    }
}

/// Fetch one endpoint and publish the raw JSON value.
pub fn run_get(
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    path: String,
    query: Option<String>,
) {
    thread::spawn(move || {
        let target = match &query {
            Some(query) if !query.is_empty() => format!("{path}?{query}"),
            _ => path.clone(),
        };
        match client.get_value(&target) {
            Ok(value) => push(
                &queue,
                Update::Value {
                    path,
                    value,
                    message: None,
                    kind: None,
                },
            ),
            Err(message) => push(
                &queue,
                Update::Error {
                    path: Some(path),
                    message,
                },
            ),
        }
    });
}

/// Write endpoint: the response entity is published under the request path so
/// the shell can merge it through the same router it uses for reads.
pub fn run_post(
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    path: String,
    body: Option<Value>,
) {
    thread::spawn(move || {
        let body = body.unwrap_or_else(|| json!({}));
        match client.post_value(&path, Some(body)) {
            Ok(value) => {
                let message = value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let kind = value.get("kind").and_then(Value::as_str).map(str::to_owned);
                push(
                    &queue,
                    Update::Value {
                        path,
                        value,
                        message,
                        kind,
                    },
                )
            }
            Err(message) => push(
                &queue,
                Update::Error {
                    path: Some(path),
                    message,
                },
            ),
        }
    });
}

/// Write endpoint whose response is binary (exports).
pub fn run_post_bytes(
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    path: String,
    body: Option<Value>,
) {
    thread::spawn(move || {
        let payload = body.map(|body| body.to_string().into_bytes());
        let headers: Vec<(&str, &str)> = if payload.is_some() {
            vec![("Content-Type", "application/json")]
        } else {
            vec![]
        };
        match client.post_raw(&path, payload, &headers) {
            Ok(bytes) => push(&queue, Update::Binary { path, bytes }),
            Err(message) => push(
                &queue,
                Update::Error {
                    path: Some(path),
                    message,
                },
            ),
        }
    });
}

/// Fetch an image endpoint and hand the bytes to the decoder.
pub fn run_image(
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    key: String,
    path: String,
) {
    thread::spawn(move || match client.get_bytes(&path) {
        Ok(bytes) => push(&queue, Update::Image { key, bytes }),
        Err(_) => {}
    });
}

/// Upload an `.okscript` package.
pub fn run_upload(
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    path: String,
    file_name: String,
    bytes: Vec<u8>,
) {
    thread::spawn(move || {
        match client.post_raw(&path, Some(bytes), &[("X-File-Name", &file_name)]) {
            Ok(bytes) => {
                let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                let message = value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                push(
                    &queue,
                    Update::Value {
                        path,
                        value,
                        message,
                        kind: None,
                    },
                )
            }
            Err(message) => push(
                &queue,
                Update::Error {
                    path: Some(path),
                    message,
                },
            ),
        }
    });
}

/// Initial snapshot: the same set of reads the web frontend performs on boot.
pub fn run_snapshot(client: Arc<ApiClient>, queue: Arc<Mutex<VecDeque<Update>>>) {
    thread::spawn(move || {
        use crate::model::{
            AboutInfo, ApiSnapshot, AutomationTask, CaptureUiState, NavigationCapabilities,
            ScheduleData, ScriptSummary, ScriptTemplate, SettingsGroup, TemplateImage,
            ThemeUiState,
        };

        macro_rules! read {
            ($ty:ty, $path:expr) => {
                client.get::<$ty>($path).unwrap_or_default()
            };
        }

        let capture: CaptureUiState = match client.get("/api/ui/capture") {
            Ok(capture) => capture,
            Err(message) => {
                push(
                    &queue,
                    Update::Error {
                        path: Some("/api/ui/capture".into()),
                        message,
                    },
                );
                return;
            }
        };
        let snapshot = ApiSnapshot {
            capture,
            theme: read!(ThemeUiState, "/api/ui/theme"),
            tasks: read!(Vec<AutomationTask>, "/api/tasks"),
            settings: read!(Vec<SettingsGroup>, "/api/settings"),
            navigation: read!(NavigationCapabilities, "/api/navigation"),
            about: read!(AboutInfo, "/api/about"),
            scripts: read!(Vec<ScriptSummary>, "/api/scripts"),
            script_templates: read!(Vec<ScriptTemplate>, "/api/script-templates"),
            templates: read!(Vec<TemplateImage>, "/api/templates"),
            schedule: read!(ScheduleData, "/api/schedule"),
        };
        push(&queue, Update::Snapshot(snapshot));
    });
}

/// WebSocket worker: subscribes to runtime events and reconnects at 1.5 s,
/// matching the web frontend's reconnect policy.
pub fn run_events(client: Arc<ApiClient>, queue: Arc<Mutex<VecDeque<Update>>>) {
    thread::spawn(move || loop {
        let session_key = match client.get::<crate::model::CaptureUiState>("/api/ui/capture") {
            Ok(capture) if !capture.event_session_key.is_empty() => capture.event_session_key,
            _ => {
                push(
                    &queue,
                    Update::Value {
                        path: "/api/ui/capture".into(),
                        value: Value::Null,
                        message: None,
                        kind: None,
                    },
                );
                thread::sleep(Duration::from_millis(1500));
                continue;
            }
        };
        match connect(client.event_url(&session_key)) {
            Ok((mut socket, _response)) => loop {
                match socket.read() {
                    Ok(Message::Text(text)) => {
                        if let Ok(event) =
                            serde_json::from_str::<crate::model::RuntimeEvent>(text.as_ref())
                        {
                            push(&queue, Update::Event(event));
                        }
                    }
                    Ok(Message::Ping(payload)) => {
                        let _ = socket.send(Message::Pong(payload));
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    Ok(_) => {}
                }
            },
            Err(_) => {}
        }
        thread::sleep(Duration::from_millis(1500));
    });
}

