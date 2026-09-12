//! Design tokens for the native shell.
//!
//! Values are transcribed from the web frontend's `web_src/src/styles.css`
//! (`:root` for dark, `:root[data-theme="light"]` for light) so both frontends
//! share one palette. `ok/ui/gpui/src/theme.rs` and `web_src/src/styles.css`
//! must be updated together.

use gpui::{rgb, rgba, App, Hsla, Rgba, SharedString};
use gpui_component::Theme;

use crate::model::SystemAccent;

/// `WINDOWS_STANDARD_BLUE` from `App.tsx`.
pub const WINDOWS_STANDARD_BLUE: u32 = 0x60cdff;

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

fn hexa(value: u32) -> Hsla {
    rgba(value).into()
}

pub fn mix(left: Hsla, right: Hsla, amount: f32) -> Hsla {
    let left: Rgba = left.into();
    let right: Rgba = right.into();
    let amount = amount.clamp(0.0, 1.0);
    let blend = |a: f32, b: f32| a + (b - a) * amount;
    Rgba {
        r: blend(left.r, right.r),
        g: blend(left.g, right.g),
        b: blend(left.b, right.b),
        a: blend(left.a, right.a),
    }
    .into()
}

/// Parse `#rrggbb` exactly like the web frontend's validation regex.
pub fn parse_hex(text: &str) -> Option<Hsla> {
    let trimmed = text.trim();
    let body = trimmed.strip_prefix('#')?;
    if body.len() != 6 || !body.chars().all(|value| value.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(body, 16).ok().map(hex)
}

/// Resolve the accent color for the active theme.
pub fn accent_for(system: Option<&SystemAccent>, dark: bool) -> Hsla {
    let from_system = system.and_then(|accent| {
        if dark {
            accent.dark.as_deref()
        } else {
            accent.light.as_deref()
        }
    });
    from_system
        .and_then(parse_hex)
        .unwrap_or_else(|| hex(WINDOWS_STANDARD_BLUE))
}

pub fn accent_hover(accent: Hsla) -> Hsla {
    mix(accent, hex(0xffffff), 0.14)
}

/// Apply the shared palette to gpui-component's theme.
pub fn apply(cx: &mut App, mode: gpui_component::ThemeMode, accent: Hsla) {
    let dark = matches!(mode, gpui_component::ThemeMode::Dark);
    Theme::change(mode, None, cx);
    let accent_hover = accent_hover(accent);
    let accent_active = mix(accent, hex(0x000000), 0.12);
    // `#102a35` — the web palette's `colorNeutralForegroundOnBrand`.
    let on_accent = hex(0x102a35);

    let theme = Theme::global_mut(cx);
    theme.font_family = SharedString::from("Segoe UI Variable");
    theme.mono_font_family = SharedString::from("Consolas");
    theme.font_size = gpui::px(15.0);
    theme.mono_font_size = gpui::px(13.0);
    // `.surface-card { border-radius: 11px }`, `.modal { border-radius: 10px }`
    theme.radius = gpui::px(6.0);
    theme.radius_lg = gpui::px(11.0);
    theme.shadow = true;

    let colors = &mut theme.colors;
    if dark {
        let background = hex(0x251e22);
        let card = hexa(0xffffff0d);
        let stroke = hexa(0xffffff17);
        let selected = hexa(0xffffff13);
        let text = hex(0xeee9ec);
        let muted = hex(0xcfc8cc);

        colors.background = background;
        colors.foreground = text;
        colors.border = stroke;
        colors.input = stroke;
        colors.muted = hex(0x302a2e);
        colors.muted_foreground = muted;
        colors.secondary = hexa(0xffffff0d);
        colors.secondary_hover = hexa(0xffffff18);
        colors.secondary_active = selected;
        colors.secondary_foreground = text;
        colors.accent = accent;
        colors.accent_foreground = on_accent;
        colors.primary = accent;
        colors.primary_foreground = on_accent;
        colors.primary_hover = accent_hover;
        colors.primary_active = accent_active;
        colors.ring = accent;
        colors.caret = hex(0xf0edf0);
        colors.selection = hexa(0x264f7899);
        colors.danger = hex(0xe87272);
        colors.danger_hover = hex(0xef8080);
        colors.danger_active = hex(0xd96161);
        colors.danger_foreground = hex(0xffffff);
        colors.success = hex(0x6ccb7a);
        colors.success_hover = hex(0x7dd68a);
        colors.success_active = hex(0x5cbb6a);
        colors.success_foreground = hex(0x102a35);
        colors.info = hex(0x75a9ef);
        colors.info_hover = hex(0x86b6f4);
        colors.info_active = hex(0x6398de);
        colors.info_foreground = hex(0x102a35);
        colors.warning = hex(0xe8b872);
        colors.warning_hover = hex(0xf0c583);
        colors.warning_active = hex(0xd6a661);
        colors.warning_foreground = hex(0x102a35);
        colors.link = accent;
        colors.link_hover = accent_hover;
        colors.link_active = accent_active;
        colors.popover = hex(0x282126);
        colors.popover_foreground = text;
        colors.sidebar = background;
        colors.sidebar_foreground = text;
        colors.sidebar_border = stroke;
        colors.sidebar_primary = accent;
        colors.sidebar_primary_foreground = on_accent;
        colors.sidebar_accent = selected;
        colors.sidebar_accent_foreground = text;
        colors.title_bar = background;
        colors.title_bar_border = stroke;
        colors.tab = hexa(0xffffff0a);
        colors.tab_active = selected;
        colors.tab_active_foreground = text;
        colors.tab_foreground = muted;
        colors.tab_bar = background;
        colors.tab_bar_segmented = hex(0x302a2e);
        colors.list = card;
        colors.list_hover = hexa(0xffffff11);
        colors.list_active = selected;
        colors.list_active_border = accent;
        colors.list_even = hexa(0xffffff05);
        colors.list_head = hex(0x302a2e);
        colors.table = card;
        colors.table_hover = hexa(0xffffff11);
        colors.table_active = selected;
        colors.table_active_border = accent;
        colors.table_even = hexa(0xffffff05);
        colors.table_head = hex(0x302a2e);
        colors.table_head_foreground = muted;
        colors.table_row_border = stroke;
        colors.switch = hexa(0xffffff26);
        colors.switch_thumb = hex(0xf0edf0);
        colors.skeleton = hexa(0xffffff14);
        colors.slider_bar = hexa(0xffffff1f);
        colors.slider_thumb = accent;
        colors.progress_bar = accent;
        colors.scrollbar = hexa(0x00000000);
        colors.scrollbar_thumb = hexa(0xffffff94);
        colors.scrollbar_thumb_hover = hexa(0xffffffb0);
        colors.overlay = hexa(0x0c070aad);
        colors.window_border = stroke;
        colors.group_box = card;
        colors.group_box_foreground = text;
        colors.accordion = card;
        colors.accordion_hover = hexa(0xffffff11);
        colors.drop_target = hexa(0x60cdff40);
        colors.drag_border = accent;
        colors.description_list_label = card;
        colors.description_list_label_foreground = muted;
    } else {
        let background = hex(0xeee7eb);
        let card = hexa(0xffffffb8);
        let stroke = hexa(0x47303c1f);
        let selected = hexa(0x7e476817);
        let text = hex(0x2f252a);
        let muted = hex(0x6e6268);

        colors.background = background;
        colors.foreground = text;
        colors.border = stroke;
        colors.input = stroke;
        colors.muted = hex(0xffffffb8);
        colors.muted_foreground = muted;
        colors.secondary = hexa(0x50314111);
        colors.secondary_hover = hexa(0x5031411c);
        colors.secondary_active = selected;
        colors.secondary_foreground = text;
        colors.accent = accent;
        colors.accent_foreground = on_accent;
        colors.primary = accent;
        colors.primary_foreground = on_accent;
        colors.primary_hover = accent_hover;
        colors.primary_active = accent_active;
        colors.ring = accent;
        colors.caret = hex(0x2f252a);
        colors.selection = hexa(0x264f7899);
        colors.danger = hex(0xd13b45);
        colors.danger_hover = hex(0xdb4b55);
        colors.danger_active = hex(0xbb2f39);
        colors.danger_foreground = hex(0xffffff);
        colors.success = hex(0x2f8f43);
        colors.success_hover = hex(0x369c4c);
        colors.success_active = hex(0x27793a);
        colors.success_foreground = hex(0xffffff);
        colors.info = hex(0x2f6fd0);
        colors.info_hover = hex(0x3a7cdd);
        colors.info_active = hex(0x265eb5);
        colors.info_foreground = hex(0xffffff);
        colors.warning = hex(0xa9721a);
        colors.warning_hover = hex(0xb87d1f);
        colors.warning_active = hex(0x946116);
        colors.warning_foreground = hex(0xffffff);
        colors.link = accent_active;
        colors.link_hover = accent_hover;
        colors.link_active = accent_active;
        colors.popover = hex(0xfff9fc);
        colors.popover_foreground = text;
        colors.sidebar = background;
        colors.sidebar_foreground = hex(0x4c4046);
        colors.sidebar_border = stroke;
        colors.sidebar_primary = accent;
        colors.sidebar_primary_foreground = on_accent;
        colors.sidebar_accent = selected;
        colors.sidebar_accent_foreground = text;
        colors.title_bar = background;
        colors.title_bar_border = stroke;
        colors.tab = hexa(0x50314111);
        colors.tab_active = selected;
        colors.tab_active_foreground = text;
        colors.tab_foreground = muted;
        colors.tab_bar = background;
        colors.tab_bar_segmented = hex(0xffffffb8);
        colors.list = card;
        colors.list_hover = hexa(0x42273511);
        colors.list_active = selected;
        colors.list_active_border = accent;
        colors.list_even = hexa(0x482b3a0a);
        colors.list_head = hex(0xffffffb8);
        colors.table = card;
        colors.table_hover = hexa(0x42273511);
        colors.table_active = selected;
        colors.table_active_border = accent;
        colors.table_even = hexa(0x482b3a0a);
        colors.table_head = hex(0xffffffb8);
        colors.table_head_foreground = muted;
        colors.table_row_border = hexa(0x47303c14);
        colors.switch = hexa(0x7f727933);
        colors.switch_thumb = hex(0xffffff);
        colors.skeleton = hexa(0x482b3a14);
        colors.slider_bar = hexa(0x482b3a1f);
        colors.slider_thumb = accent;
        colors.progress_bar = accent;
        colors.scrollbar = hexa(0x00000000);
        colors.scrollbar_thumb = hexa(0x47303c73);
        colors.scrollbar_thumb_hover = hexa(0x47303c99);
        colors.overlay = hexa(0x2f252a73);
        colors.window_border = stroke;
        colors.group_box = card;
        colors.group_box_foreground = text;
        colors.accordion = card;
        colors.accordion_hover = hexa(0x42273511);
        colors.drop_target = hexa(0x60cdff40);
        colors.drag_border = accent;
        colors.description_list_label = card;
        colors.description_list_label_foreground = muted;
    }
}

/// Web palette constants reused by page code (cards, editors, logs, toasts).
pub struct Tokens;

impl Tokens {
    /// `.modal { background: #282126 }`
    pub fn modal(dark: bool) -> Hsla {
        if dark {
            hex(0x282126)
        } else {
            hex(0xfff9fc)
        }
    }

    /// `.toast { background: linear-gradient(...#302a2e) }`
    pub fn toast(dark: bool) -> Hsla {
        if dark {
            hex(0x302a2e)
        } else {
            hex(0xfff9fc)
        }
    }

    /// `.python-editor { background: #171518 }`
    pub fn editor(dark: bool) -> Hsla {
        if dark {
            hex(0x171518)
        } else {
            hex(0xfff9fc)
        }
    }

    /// `.log-console { background: #191719 }`
    pub fn console(dark: bool) -> Hsla {
        if dark {
            hex(0x191719)
        } else {
            hex(0xffffff)
        }
    }

    /// `.capture-preview { background: #161216 }`
    pub fn preview(dark: bool) -> Hsla {
        if dark {
            hex(0x161216)
        } else {
            hex(0xe3dbe0)
        }
    }

    /// `.nav-update-dot { background: #e74856 }`
    pub fn update_dot() -> Hsla {
        hex(0xe74856)
    }

    /// `.toast-error { --toast-accent: #e87272 }`
    pub fn toast_accent(kind: crate::model::ToastKind) -> Hsla {
        match kind {
            crate::model::ToastKind::Success => hex(0x6ccb7a),
            crate::model::ToastKind::Info => hex(0x75a9ef),
            crate::model::ToastKind::Error => hex(0xe87272),
        }
    }

    /// `.window-close:hover { background: #c42b1c }`
    pub fn window_close() -> Hsla {
        hex(0xc42b1c)
    }
}

/// Parse a `#rgb`/`#rrggbb` value coming from the API (icons, accents).
pub fn parse_color(text: &str) -> Option<Hsla> {
    parse_hex(text)
}
