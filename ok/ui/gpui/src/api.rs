use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use reqwest::blocking::Client;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tungstenite::{connect, Message};

use crate::model::{
    AboutInfo, ApiSnapshot, AutomationTask, CaptureUiState, NavigationCapabilities, RuntimeEvent,
    ScheduleData, ScriptSummary, ScriptTemplate, SettingsGroup, TemplateImage, Update,
};

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    http: Client,
}

impl ApiClient {
    pub fn new(base_url: String) -> Result<Self, String> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("HTTP client: {error}"))?;
        Ok(Self { base_url, http })
    }

    fn url(&self, path: &str) -> String {
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
        decode_response(response)
    }

    fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, String> {
        let request = self.http.post(self.url(path));
        let response = match body {
            Some(body) => request.json(body).send(),
            None => request.send(),
        }
        .map_err(|error| error.to_string())?;
        decode_response(response)
    }

    pub fn snapshot(&self) -> Result<ApiSnapshot, String> {
        let capture: CaptureUiState = self.get("/api/ui/capture")?;
        let tasks: Vec<AutomationTask> = self.get("/api/tasks").unwrap_or_default();
        let settings: Vec<SettingsGroup> = self.get("/api/settings").unwrap_or_default();
        let navigation: NavigationCapabilities = self.get("/api/navigation").unwrap_or_default();
        let about: AboutInfo = self.get("/api/about").unwrap_or_default();
        let scripts: Vec<ScriptSummary> = self.get("/api/scripts").unwrap_or_default();
        let script_templates: Vec<ScriptTemplate> =
            self.get("/api/script-templates").unwrap_or_default();
        let templates: Vec<TemplateImage> = self.get("/api/templates").unwrap_or_default();
        let schedule: ScheduleData = self.get("/api/schedule").unwrap_or_default();
        Ok(ApiSnapshot {
            capture,
            tasks,
            settings,
            navigation,
            about,
            logs: None,
            scripts,
            script_templates,
            templates,
            schedule,
        })
    }

    pub fn post_value(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        let body = body.unwrap_or_else(|| json!({}));
        self.post(path, Some(&body))
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

fn decode_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(if body.is_empty() {
            format!("HTTP {status}")
        } else {
            format!("HTTP {status}: {body}")
        });
    }
    response.json::<T>().map_err(|error| error.to_string())
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

pub fn start_network(client: Arc<ApiClient>, queue: Arc<Mutex<VecDeque<Update>>>) {
    let snapshot_client = client.clone();
    let snapshot_queue = queue.clone();
    thread::spawn(move || match snapshot_client.snapshot() {
        Ok(snapshot) => push_update(&snapshot_queue, Update::Snapshot(snapshot)),
        Err(error) => push_update(&snapshot_queue, Update::Error(error)),
    });

    thread::spawn(move || {
        // The event endpoint is intentionally a separate worker. HTTP refreshes
        // remain responsive even if a WebSocket peer disconnects or is absent.
        thread::sleep(Duration::from_millis(250));
        let session_key = match client.get::<CaptureUiState>("/api/ui/capture") {
            Ok(capture) if !capture.event_session_key.is_empty() => capture.event_session_key,
            _ => return,
        };
        let websocket = connect(client.event_url(&session_key));
        let Ok((mut socket, _response)) = websocket else {
            return;
        };
        loop {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(event) = serde_json::from_str::<RuntimeEvent>(text.as_ref()) {
                        push_update(&queue, Update::Event(event));
                    }
                }
                Ok(Message::Ping(payload)) => {
                    let _ = socket.send(Message::Pong(payload));
                }
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
}

pub fn run_action(
    client: Arc<ApiClient>,
    queue: Arc<Mutex<VecDeque<Update>>>,
    path: String,
    body: Option<Value>,
) {
    thread::spawn(move || {
        let result = client.post_value(&path, body);
        match result {
            Ok(value) => {
                let message = value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("操作已完成")
                    .to_owned();
                let snapshot = client.snapshot().ok();
                push_update(&queue, Update::ActionFinished { message, snapshot });
            }
            Err(error) => push_update(&queue, Update::Error(error)),
        }
    });
}

fn push_update(queue: &Arc<Mutex<VecDeque<Update>>>, update: Update) {
    if let Ok(mut queue) = queue.lock() {
        queue.push_back(update);
        while queue.len() > 128 {
            queue.pop_front();
        }
    }
}
