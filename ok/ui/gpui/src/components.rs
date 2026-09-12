//! Shell building blocks shared by every page.
//!
//! Sizes and paddings are transcribed from `web_src/src/styles.css`; the
//! numeric constants below exist so page code reads close to the React
//! markup it mirrors.

use gpui::{
    div, prelude::*, px, AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, Hsla,
    IntoElement, ParentElement, SharedString, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    ActiveTheme as _, Disableable as _, Icon, StyledExt as _,
};

use crate::icons::OkIcon;
use crate::model::ToastKind;
use crate::theme::Tokens;

/// `j`h1 { font-size: 1.4rem }` with the web root size of 20px.
pub const FS_H1: f32 = 28.0;
/// `h2 { font-size: .88rem }`
pub const FS_H2: f32 = 17.6;
/// `.task-summary strong { font-size: .9rem }`
pub const FS_CARD: f32 = 18.0;
/// `.app-identity strong { font-size: 1.06rem }`
pub const FS_IDENTITY: f32 = 21.0;
/// `.task-config-row > span strong { font-size: .8rem }`
pub const FS_BODY: f32 = 16.0;
/// `.nav-item { font-size: .7rem }`
pub const FS_NAV: f32 = 14.0;
/// `.about-identity p { font-size: .7rem }`, `.task-summary small`
pub const FS_SMALL: f32 = 14.0;
/// `.template-card small { font-size: .65rem }`
pub const FS_TINY: f32 = 13.0;

pub const CARD_RADIUS: f32 = 11.0;
pub const MODAL_RADIUS: f32 = 10.0;
pub const BUTTON_HEIGHT: f32 = 32.0;
pub const BUTTON_RADIUS: f32 = 6.0;
pub const OPTION_ROW_MIN_HEIGHT: f32 = 45.0;
pub const DEVICE_SEARCH_HEIGHT: f32 = 43.0;
pub const SIDEBAR_WIDTH: f32 = 196.0;
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 61.0;
pub const NAV_ITEM_HEIGHT: f32 = 34.0;
pub const PAGE_PADDING: f32 = 16.0;
pub const CONTENT_GAP: f32 = 9.0;

/// `.surface-card { border-radius: 11px; border: 0; background: var(--card-bg) }`
pub fn card(cx: &App) -> Div {
    div().v_flex().rounded(px(CARD_RADIUS)).bg(cx.theme().list)
}

/// Standard page container: `padding: 16px`, vertical, 9px gaps, scrollable.
pub fn page_root(id: &str) -> gpui::Stateful<Div> {
    div()
        .id(ElementId::Name(SharedString::from(id.to_owned())))
        .v_flex()
        .size_full()
        .gap(px(CONTENT_GAP))
        .p(px(PAGE_PADDING))
        .overflow_y_scroll()
}

pub fn page_title(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(FS_H1))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.into())
}

pub fn section_title(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(FS_H2))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.into())
}

pub fn body_text(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(FS_BODY))
        .text_color(cx.theme().foreground)
        .child(text.into())
}

pub fn muted_text(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(FS_SMALL))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub fn tiny_text(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(FS_TINY))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub fn toolbar(cx: &App) -> Div {
    div().h_flex().items_center().gap_2().flex_wrap().text_color(cx.theme().foreground)
}

/// `.start-actions button/.tool-card button { height: 32px; border-radius: 6px }`
pub fn button_surface(cx: &App) -> Hsla {
    if is_dark(cx) {
        gpui::rgba(0xffffff12).into()
    } else {
        gpui::rgba(0x50314111).into()
    }
}

pub fn is_dark(cx: &App) -> bool {
    matches!(cx.theme().mode, gpui_component::ThemeMode::Dark)
}

pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .primary()
        .label(label.into())
        .h(px(BUTTON_HEIGHT))
        .rounded(px(BUTTON_RADIUS))
        .text_size(px(FS_SMALL))
}

pub fn secondary_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Button {
    Button::new(id)
        .label(label.into())
        .bg(button_surface(cx))
        .h(px(BUTTON_HEIGHT))
        .rounded(px(BUTTON_RADIUS))
        .text_size(px(FS_SMALL))
}

pub fn icon_button(id: impl Into<ElementId>, icon: OkIcon, cx: &App) -> Button {
    Button::new(id)
        .icon(icon.icon())
        .ghost()
        .h(px(BUTTON_HEIGHT))
        .rounded(px(BUTTON_RADIUS))
        .text_color(cx.theme().foreground)
}

pub fn labelled_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: OkIcon,
    cx: &App,
) -> Button {
    Button::new(id)
        .label(label.into())
        .icon(icon.icon())
        .bg(button_surface(cx))
        .h(px(BUTTON_HEIGHT))
        .rounded(px(BUTTON_RADIUS))
        .text_size(px(FS_SMALL))
}

/// `.option-row { min-height: 45px; padding: 0 17px; border-radius: 6px }`
pub fn option_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    selected: bool,
    disabled: bool,
    cx: &App,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let hover: Hsla = if is_dark(cx) {
        gpui::rgba(0xffffff11).into()
    } else {
        gpui::rgba(0x42273511).into()
    };
    let accent = cx.theme().accent;
    let foreground = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let background: Hsla = if selected {
        if is_dark(cx) {
            gpui::rgba(0xffffff13).into()
        } else {
            gpui::rgba(0x7e476817).into()
        }
    } else {
        gpui::rgba(0x00000000).into()
    };
    div()
        .id(id)
        .h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_3()
        .min_h(px(OPTION_ROW_MIN_HEIGHT))
        .px(px(17.0))
        .rounded(px(BUTTON_RADIUS))
        .bg(background)
        .text_color(if disabled { muted } else { foreground })
        .text_size(px(FS_SMALL))
        .when(!disabled, |this| {
            this.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .when(selected, |this| {
            this.border_l_2().border_color(accent)
        })
        .child(label.into())
        .when_some(detail, |this, detail| {
            this.child(
                div()
                    .text_size(px(FS_TINY))
                    .text_color(muted)
                    .child(detail),
            )
        })
        .when(disabled, |this| this.opacity(0.55))
        .when(!disabled, |this| this.on_click(on_click))
        .into_any_element()
}

/// `.nav-item { height: 34px; grid-template-columns: 17px minmax(0, 1fr); gap: 4px; padding: 0 7px }`
pub fn nav_item(
    key: &str,
    label: impl Into<SharedString>,
    icon: OkIcon,
    active: bool,
    collapsed: bool,
    show_dot: bool,
    cx: &App,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let hover: Hsla = if is_dark(cx) {
        gpui::rgba(0xffffff0e).into()
    } else {
        gpui::rgba(0x42273511).into()
    };
    let active_bg: Hsla = if is_dark(cx) {
        gpui::rgba(0xffffff0d).into()
    } else {
        gpui::rgba(0x42273511).into()
    };
    let foreground = cx.theme().foreground;
    let icon_color = if is_dark(cx) {
        gpui::rgba(0xe0daddff).into()
    } else {
        gpui::rgba(0x4c4046ff).into()
    };
    div()
        .id(ElementId::Name(SharedString::from(format!("nav-{key}"))))
        .relative()
        .h_flex()
        .w_full()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .h(px(NAV_ITEM_HEIGHT))
        .px(px(7.0))
        .rounded(px(BUTTON_RADIUS))
        .bg(if active { active_bg } else { gpui::rgba(0x00000000).into() })
        .text_color(if active { foreground } else { icon_color })
        .text_size(px(FS_NAV))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(icon.icon().size(px(17.0)).text_color(icon_color))
        .when(!collapsed, |this| this.child(label.into()))
        .when(show_dot, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(6.0))
                    .left(px(21.0))
                    .size(px(6.0))
                    .rounded_full()
                    .bg(Tokens::update_dot()),
            )
        })
        .on_click(on_click)
        .into_any_element()
}

/// `.switch-control` wrapper used by task cards and settings rows.
pub fn switch_control(
    id: impl Into<ElementId>,
    checked: bool,
    disabled: bool,
    cx: &App,
    on_click: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    gpui_component::switch::Switch::new(id)
        .checked(checked)
        .disabled(disabled)
        .on_click(on_click)
        .text_size(px(FS_BODY))
        .text_color(cx.theme().foreground)
        .into_any_element()
}

/// `.task-config-row { grid-template-columns: minmax(150px,1fr) minmax(0,2fr); min-height: 52px }`
pub fn config_row(
    key: impl Into<SharedString>,
    description: impl Into<SharedString>,
    control: AnyElement,
    sub_config: bool,
    cx: &App,
) -> AnyElement {
    let key = key.into();
    let description = description.into();
    div()
        .h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .min_h(px(52.0))
        .py(px(6.0))
        .when(sub_config, |this| this.pl(px(28.0)))
        .border_b_1()
        .border_color(gpui::rgba(0xffffff0b))
        .child(
            div()
                .v_flex()
                .gap(px(3.0))
                .flex_1()
                .child(
                    div()
                        .text_size(px(FS_BODY))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(key),
                )
                .when(!description.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(FS_TINY))
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    )
                }),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// Modal shell matching `.modal-backdrop` + `.modal`.
pub fn modal_backdrop() -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(0x0c070aad))
}

pub fn modal_frame(width: f32, cx: &App) -> Div {
    div()
        .v_flex()
        .w(px(width))
        .max_h(gpui::relative(0.92))
        .rounded(px(MODAL_RADIUS))
        .border_1()
        .border_color(gpui::rgba(0xffffff24))
        .bg(Tokens::modal(is_dark(cx)))
        .text_color(cx.theme().foreground)
        .shadow(vec![BoxShadow {
            color: gpui::rgba(0x0000008c).into(),
            offset: gpui::point(px(0.0), px(22.0)),
            blur_radius: px(70.0),
            spread_radius: px(0.0),
        }])
        .overflow_hidden()
}

pub fn modal_header(title: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_between()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .px(px(16.0))
        .py(px(12.0))
        .text_size(px(FS_BODY))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

pub fn modal_body() -> gpui::Stateful<Div> {
    div()
        .id("modal-body")
        .v_flex()
        .flex_1()
        .min_h(px(0.0))
        .gap_2()
        .p(px(16.0))
        .overflow_y_scroll()
}

pub fn modal_footer() -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_end()
        .gap_2()
        .border_t_1()
        .border_color(gpui::rgba(0xffffff14))
        .px(px(16.0))
        .py(px(12.0))
}

/// `.toast` card: 4px accent bar, 8px radius, 48px min height.
pub fn toast_card(kind: ToastKind, message: impl Into<SharedString>, cx: &App) -> AnyElement {
    let accent = Tokens::toast_accent(kind);
    let foreground = cx.theme().foreground;
    let icon = match kind {
        ToastKind::Success => OkIcon::CheckmarkCircle,
        ToastKind::Info => OkIcon::Info,
        ToastKind::Error => OkIcon::ErrorCircle,
    };
    div()
        .h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .min_h(px(48.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(gpui::rgba(0xffffff21))
        .bg(Tokens::toast(is_dark(cx)))
        .text_color(foreground)
        .text_size(px(FS_SMALL))
        .overflow_hidden()
        .child(div().w(px(4.0)).h_full().flex_none().bg(accent))
        .child(icon.icon().size(px(20.0)).text_color(accent))
        .child(div().flex_1().child(message.into()))
        .into_any_element()
}

pub fn tag(label: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .h_flex()
        .items_center()
        .gap_1()
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(5.0))
        .bg(button_surface(cx))
        .text_size(px(FS_TINY))
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
        .into_any_element()
}

pub fn busy_spinner(cx: &App) -> AnyElement {
    gpui_component::spinner::Spinner::new()
        .color(cx.theme().accent)
        .into_any_element()
}

pub fn centered_empty(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .text_size(px(FS_SMALL))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn icon(size: f32, glyph: OkIcon, cx: &App) -> Icon {
    glyph.icon().size(px(size)).text_color(cx.theme().muted_foreground)
}
