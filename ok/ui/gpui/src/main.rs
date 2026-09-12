//! ok-script native GPUI client.
//!
//! The Python runtime owns the automation engine and exposes the same FastAPI
//! contract the browser UI uses (`ok/ui/web/app.py`); this crate renders that
//! contract natively.

mod api;
mod app;
mod assets;
mod brand_icons;
mod components;
mod i18n;
mod icons;
mod icons_data;
mod images;
mod markup;
mod model;
mod modals;
mod pages;
mod schedule_dialog;
mod script;
mod state;
mod task_tab;
mod theme;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use gpui::{px, size, AppContext as _, Application, Bounds, WindowBounds, WindowOptions};
use gpui_component::Root;

use api::ApiClient;
use app::OkApp;
use assets::Assets;
use state::Prefs;

struct Cli {
    url: String,
    width: f32,
    height: f32,
    min_width: f32,
    min_height: f32,
    debug: bool,
    locale: Option<String>,
    theme: Option<String>,
    page: Option<String>,
    markup: Option<String>,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            url: String::new(),
            width: 1280.0,
            height: 800.0,
            min_width: 960.0,
            min_height: 640.0,
            debug: false,
            locale: None,
            theme: None,
            page: None,
            markup: None,
        }
    }
}

fn parse_cli() -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("missing value for {argument}"));
        match argument.as_str() {
            "--url" => cli.url = value()?,
            "--width" => cli.width = value()?.parse().map_err(|_| "invalid --width")?,
            "--height" => cli.height = value()?.parse().map_err(|_| "invalid --height")?,
            "--min-width" => {
                cli.min_width = value()?.parse().map_err(|_| "invalid --min-width")?
            }
            "--min-height" => {
                cli.min_height = value()?.parse().map_err(|_| "invalid --min-height")?
            }
            "--locale" => cli.locale = Some(value()?),
            "--page" => cli.page = Some(value()?),
            "--markup" => cli.markup = Some(value()?),
            "--theme" => cli.theme = Some(value()?),
            "--debug" => cli.debug = true,
            "--help" | "-h" => {
                println!(
                    "ok-script-gpui --url URL [--width N] [--height N] [--min-width N] \
                     [--min-height N] [--locale xx_XX] [--theme Light|Dark|Auto] \
                     [--page Capture|Tasks|Script|Templates|Schedule|Settings|Notifications|About] \
                     [--debug]"
                );
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }
    if cli.url.is_empty() {
        return Err("--url is required".to_owned());
    }
    Ok(cli)
}

fn main() {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if cli.debug {
        eprintln!("starting GPUI client for {}", cli.url);
    }

    let client = match ApiClient::new(cli.url.clone()) {
        Ok(client) => Arc::new(client),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let queue: Arc<Mutex<VecDeque<model::Update>>> = Arc::new(Mutex::new(VecDeque::new()));
    api::run_snapshot(client.clone(), queue.clone());
    api::run_events(client.clone(), queue.clone());

    let mut prefs = Prefs::load();
    if let Some(locale) = cli.locale.clone() {
        prefs.language = locale;
    }
    if let Some(theme) = cli.theme.clone() {
        prefs.theme = theme;
    }
    let prefs_for_locale = prefs.clone();

    let min_width = cli.min_width;
    let min_height = cli.min_height;
    let debug = cli.debug;
    let start_page = cli.page.as_deref().and_then(state::Page::parse);
    let start_markup = cli.markup.clone();
    // Required on Windows for the task-tab WebView (an OS child window) to
    // composite inside a GPUI window; same setting gpui-component's own
    // webview example uses.
    #[cfg(target_os = "windows")]
    std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");

    let app = Application::new().with_assets(Assets);
    app.run(move |cx| {
        gpui_component::init(cx);
        i18n::set_locale(&prefs_for_locale.language);
        let bounds = Bounds::centered(None, size(px(cli.width), px(cli.height)), cx);
        let client = client.clone();
        let queue = queue.clone();
        let prefs = prefs_for_locale.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(min_width), px(min_height))),
                ..Default::default()
            },
            move |window, cx| {
                let client = client.clone();
                let queue = queue.clone();
                let prefs = prefs.clone();
                let view = cx.new(move |cx| {
                    OkApp::new(
                        client, queue, prefs, min_width, min_height, debug, start_page,
                        start_markup.clone(), cx,
                    )
                });
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open GPUI window");
        cx.activate(true);
    });
}
