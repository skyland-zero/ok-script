mod api;
mod model;
mod ui;

use std::{
    collections::VecDeque,
    env,
    sync::{Arc, Mutex},
};

use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};

use api::ApiClient;
use ui::GpuiView;

#[derive(Debug)]
struct Cli {
    url: String,
    width: f32,
    height: f32,
    min_width: f32,
    min_height: f32,
    debug: bool,
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
        }
    }
}

fn parse_cli() -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--url" => cli.url = next_value("--url", &mut args)?,
            "--width" => {
                cli.width = next_value("--width", &mut args)?
                    .parse()
                    .map_err(|_| "invalid --width".to_owned())?
            }
            "--height" => {
                cli.height = next_value("--height", &mut args)?
                    .parse()
                    .map_err(|_| "invalid --height".to_owned())?
            }
            "--min-width" => {
                cli.min_width = next_value("--min-width", &mut args)?
                    .parse()
                    .map_err(|_| "invalid --min-width".to_owned())?
            }
            "--min-height" => {
                cli.min_height = next_value("--min-height", &mut args)?
                    .parse()
                    .map_err(|_| "invalid --min-height".to_owned())?
            }
            "--debug" => cli.debug = true,
            "--help" | "-h" => {
                println!("ok-script-gpui --url URL [--width N] [--height N] [--min-width N] [--min-height N] [--debug]");
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

fn next_value<I>(name: &str, args: &mut I) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| format!("missing value for {name}"))
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

    let client = match ApiClient::new(cli.url) {
        Ok(client) => Arc::new(client),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    api::start_network(client.clone(), queue.clone());

    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(cli.width), px(cli.height)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(cli.min_width), px(cli.min_height))),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|cx| {
                    GpuiView::new(
                        client.clone(),
                        queue.clone(),
                        cli.min_width,
                        cli.min_height,
                        cx,
                    )
                })
            },
        )
        .expect("failed to open GPUI window");
        cx.activate(true);
    });
}
