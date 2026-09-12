//! The icon set used by the shell.
//!
//! Every entry maps onto a Fluent System Icon (20 regular) vendored under
//! `assets/icons/ok`, so the native shell renders exactly the glyphs the web
//! frontend gets from `@fluentui/react-icons`.

use gpui::SharedString;
use gpui_component::{Icon, IconNamed};

macro_rules! ok_icons {
    ($($variant:ident => $file:expr),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum OkIcon {
            $($variant),+
        }

        impl IconNamed for OkIcon {
            fn path(self) -> SharedString {
                match self {
                    $(Self::$variant => concat!("icons/ok/", $file, ".svg")),+
                }
                .into()
            }
        }
    };
}

ok_icons! {
    Add => "add",
    Alert => "alert",
    ArrowDown => "arrow-down",
    ArrowDownload => "arrow-download",
    ArrowExport => "arrow-export",
    ArrowImport => "arrow-import",
    ArrowLeft => "arrow-left",
    ArrowRight => "arrow-right",
    ArrowSync => "arrow-sync",
    ArrowUp => "arrow-up",
    Broom => "broom",
    Calendar => "calendar",
    Capture => "capture",
    CheckmarkCircle => "checkmark-circle",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    Close => "close",
    Code => "code",
    Copy => "copy",
    Delete => "delete",
    DeveloperBoard => "developer-board",
    DocumentText => "document-text",
    Edit => "edit",
    ErrorCircle => "error-circle",
    Eye => "eye",
    Filter => "filter",
    Folder => "folder",
    Globe => "globe",
    Grid => "grid",
    Heart => "heart",
    History => "history",
    Image => "image",
    Info => "info",
    Lightbulb => "lightbulb",
    List => "list",
    LocalLanguage => "local-language",
    Navigation => "navigation",
    PaintBrush => "paint-brush",
    Pause => "pause",
    Person => "person",
    Play => "play",
    QuestionCircle => "question-circle",
    Record => "record",
    Refresh => "refresh",
    Save => "save",
    Search => "search",
    Settings => "settings",
    Share => "share",
    Shield => "shield",
    Square => "square",
    Star => "star",
    Stop => "stop",
    Subtract => "subtract",
    TaskList => "task-list",
    Timer => "timer",
    WeatherMoon => "weather-moon",
    Window => "window",
}

impl OkIcon {
    /// The Fluent System Icon this glyph is generated from.
    ///
    /// The parity test (`tests/test_gpui_parity.py`) checks that every icon the
    /// web frontend renders from `@fluentui/react-icons` has a native glyph.
    pub fn fluent_name(self) -> &'static str {
        match self {
            Self::Add => "Add20Regular",
            Self::Alert => "Alert20Regular",
            Self::ArrowDown => "ArrowDown20Regular",
            Self::ArrowDownload => "ArrowDownload20Regular",
            Self::ArrowExport => "ArrowExport20Regular",
            Self::ArrowImport => "ArrowImport20Regular",
            Self::ArrowLeft => "ArrowLeft20Regular",
            Self::ArrowRight => "ArrowRight20Regular",
            Self::ArrowSync => "ArrowSync20Regular",
            Self::ArrowUp => "ArrowUp20Regular",
            Self::Broom => "Broom20Regular",
            Self::Calendar => "Calendar20Regular",
            Self::Capture => "Camera20Regular",
            Self::CheckmarkCircle => "CheckmarkCircle20Regular",
            Self::ChevronDown => "ChevronDown20Regular",
            Self::ChevronLeft => "ChevronLeft20Regular",
            Self::ChevronRight => "ChevronRight20Regular",
            Self::ChevronUp => "ChevronUp20Regular",
            Self::Close => "Dismiss20Regular",
            Self::Code => "Code20Regular",
            Self::Copy => "Copy20Regular",
            Self::Delete => "Delete20Regular",
            Self::DeveloperBoard => "DeveloperBoard20Regular",
            Self::DocumentText => "DocumentText20Regular",
            Self::Edit => "Edit20Regular",
            Self::ErrorCircle => "ErrorCircle20Regular",
            Self::Eye => "Eye20Regular",
            Self::Filter => "Filter20Regular",
            Self::Folder => "Folder20Regular",
            Self::Globe => "Globe20Regular",
            Self::Grid => "Grid20Regular",
            Self::Heart => "Heart20Regular",
            Self::History => "History20Regular",
            Self::Image => "Image20Regular",
            Self::Info => "Info20Regular",
            Self::Lightbulb => "Lightbulb20Regular",
            Self::List => "List20Regular",
            Self::LocalLanguage => "LocalLanguage20Regular",
            Self::Navigation => "Navigation20Regular",
            Self::PaintBrush => "PaintBrush20Regular",
            Self::Pause => "Pause20Regular",
            Self::Person => "Person20Regular",
            Self::Play => "Play20Regular",
            Self::QuestionCircle => "QuestionCircle20Regular",
            Self::Record => "Record20Regular",
            Self::Refresh => "ArrowClockwise20Regular",
            Self::Save => "Save20Regular",
            Self::Search => "Search20Regular",
            Self::Settings => "Settings20Regular",
            Self::Share => "Share20Regular",
            Self::Shield => "Shield20Regular",
            Self::Square => "Square20Regular",
            Self::Star => "Star20Regular",
            Self::Stop => "Stop20Regular",
            Self::Subtract => "Subtract20Regular",
            Self::TaskList => "TaskListSquareLtr20Regular",
            Self::Timer => "Timer20Regular",
            Self::WeatherMoon => "WeatherMoon20Regular",
            Self::Window => "Window20Regular",
        }
    }

    /// Map a manifest icon key (`code`, `image`, `calendar`, ...) to a glyph,
    /// mirroring the web frontend's `taskTabIcon`.
    pub fn from_key(key: &str) -> Self {
        match key.to_ascii_lowercase().as_str() {
            "settings" | "config" => Self::Settings,
            "image" | "assets" => Self::Image,
            "calendar" | "schedule" => Self::Calendar,
            "play" | "capture" => Self::Play,
            "code" | "script" => Self::DeveloperBoard,
            "timer" | "trigger" => Self::Timer,
            "alert" | "notification" => Self::Alert,
            "info" => Self::Info,
            "list" | "task" | "tasks" => Self::TaskList,
            _ => Self::DeveloperBoard,
        }
    }

    pub fn icon(self) -> Icon {
        self.into()
    }
}
