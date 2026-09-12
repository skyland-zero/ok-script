//! Markup (annotation) editor — mirrors `web_src/src/MarkupDialog.tsx`.
//!
//! The editor is a full-window modal: a toolbar, a zoomable canvas with the
//! template image, the annotation boxes and their resize handles, and — on top
//! of it — the bounding-box form and the delete confirmation.
//!
//! The annotation document itself lives in the shell (`self.state.annotations`):
//! `GET /api/templates/{name}/annotations` fills it and the response of
//! `POST /api/templates/{name}/annotations` replaces it, exactly like the web
//! frontend's `runtimeApi.templateAnnotations` / `saveTemplateAnnotations`.
//! Everything that is pure interaction state lives in [`MarkupState`].

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    canvas, div, img, prelude::*, px, AnyElement, Bounds, ClipboardItem, Context, ElementId,
    FocusHandle, FontWeight, Hsla, IntoElement, MouseButton, ObjectFit, ParentElement, Pixels,
    ScrollDelta, SharedString, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _}, input::Input, ActiveTheme as _, Disableable as _,
    StyledExt as _,
};
use serde_json::json;

use crate::api;
use crate::app::{Modal, OkApp};
use crate::components as ui;
use crate::i18n::t;
use crate::icons::OkIcon;
use crate::model::{TemplateAnnotation, TemplateAnnotations, TemplateImage};
use crate::theme::Tokens;

/// Smallest box the editor will create or keep (`MIN_BOX_SIZE` in the web UI).
const MIN_BOX_SIZE: f64 = 3.0;
/// Hit padding around a box, in image pixels (`stroke-width: 14px` hit rect).
const BOX_HIT_PADDING: f64 = 7.0;
const HANDLE_SIZE: f32 = 9.0;
const LABEL_SIZE: f32 = 12.0;

/// `mode` in the web editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkupTool {
    None,
    Draw,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragKind {
    Move,
    Resize,
    Pan,
}

#[derive(Clone, Debug)]
struct BoxDrag {
    kind: DragKind,
    index: usize,
    handle: Option<&'static str>,
    start: (f64, f64),
    original_box: [f64; 4],
    original_view: [f64; 4],
    changed: bool,
}

/// The bounding-box form (`BoxDraft` in the web editor).
#[derive(Clone, Debug)]
pub struct BoxDraft {
    /// `None` while creating a new annotation.
    pub index: Option<usize>,
    pub bbox: [f64; 4],
}

/// Sampled pixel under the pointer.
#[derive(Clone, Copy, Debug)]
pub struct Probe {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub x: i64,
    pub y: i64,
    pub rel_x: f64,
    pub rel_y: f64,
}

pub struct MarkupState {
    /// Template name currently open.
    pub image: String,
    /// Absolute URL of the template image.
    pub url: String,
    pub mode: MarkupTool,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub draw_start: Option<(f64, f64)>,
    pub draw_preview: Option<(f64, f64)>,
    /// Visible image rect: `[x, y, width, height]` in image pixels.
    pub view: [f64; 4],
    pub probe: Option<Probe>,
    pub draft: Option<BoxDraft>,
    /// Bumped per draft so the form inputs start from fresh values.
    pub draft_id: u64,
    pub confirm_delete: Option<usize>,
    /// Whether the viewport currently owns keyboard focus.
    pub focused: bool,
    /// For which image `view` was initialised.
    view_for: Option<String>,
    drag: Option<BoxDrag>,
    viewport: Rc<RefCell<Option<Bounds<Pixels>>>>,
    focus: Option<FocusHandle>,
}

impl Default for MarkupState {
    fn default() -> Self {
        Self {
            image: String::new(),
            url: String::new(),
            mode: MarkupTool::None,
            selected: None,
            hovered: None,
            draw_start: None,
            draw_preview: None,
            view: [0.0, 0.0, 1.0, 1.0],
            probe: None,
            draft: None,
            draft_id: 0,
            confirm_delete: None,
            focused: false,
            view_for: None,
            drag: None,
            viewport: Rc::new(RefCell::new(None)),
            focus: None,
        }
    }
}

// ---------------------------------------------------------------- helpers

fn clamp_box(bbox: [f64; 4], width: f64, height: f64) -> [f64; 4] {
    let box_width = bbox[2].round().max(MIN_BOX_SIZE).min(width.max(MIN_BOX_SIZE));
    let box_height = bbox[3].round().max(MIN_BOX_SIZE).min(height.max(MIN_BOX_SIZE));
    let x = bbox[0].round().clamp(0.0, (width - box_width).max(0.0));
    let y = bbox[1].round().clamp(0.0, (height - box_height).max(0.0));
    [x, y, box_width, box_height]
}

fn annotation_bbox(annotation: &TemplateAnnotation) -> [f64; 4] {
    let mut bbox = [0.0, 0.0, 0.0, 0.0];
    for (index, value) in annotation.bbox.iter().take(4).enumerate() {
        bbox[index] = *value;
    }
    bbox
}

/// `annotationColor()` — golden-angle hue per annotation id.
fn annotation_color(annotation: &TemplateAnnotation, index: usize) -> Hsla {
    let id = annotation.id.unwrap_or(index as i64 + 1) as f64;
    let hue = ((id * 0.618033988749895) % 1.0) * 360.0;
    gpui::hsla((hue / 360.0) as f32, 0.78, 0.62, 1.0)
}

fn box_color(annotation: &TemplateAnnotation, index: usize, selected: bool, hovered: bool) -> Hsla {
    if selected {
        gpui::rgb(0x0078d4).into()
    } else if hovered {
        gpui::rgb(0xffa500).into()
    } else {
        annotation_color(annotation, index)
    }
}

fn box_fill(color: Hsla) -> Hsla {
    Hsla { a: 0.14, ..color }
}

/// Aspect-preserving "meet" fit of `view` into `viewport`.
///
/// Returns `(scale, offset_x, offset_y)` so that
/// `screen = offset + (image_point - view_origin) * scale`.
fn fit(view: [f64; 4], viewport: (f64, f64)) -> (f64, f64, f64) {
    let (viewport_width, viewport_height) = viewport;
    if view[2] <= 0.0 || view[3] <= 0.0 || viewport_width <= 0.0 || viewport_height <= 0.0 {
        return (1.0, 0.0, 0.0);
    }
    let scale = (viewport_width / view[2]).min(viewport_height / view[3]);
    let offset_x = (viewport_width - view[2] * scale) / 2.0;
    let offset_y = (viewport_height - view[3] * scale) / 2.0;
    (scale, offset_x, offset_y)
}

fn rect_on_screen(
    bbox: [f64; 4],
    view: [f64; 4],
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> (f32, f32, f32, f32) {
    (
        (offset_x + (bbox[0] - view[0]) * scale) as f32,
        (offset_y + (bbox[1] - view[1]) * scale) as f32,
        (bbox[2] * scale) as f32,
        (bbox[3] * scale) as f32,
    )
}

fn read_pixel(pixels: &image::RgbaImage, x: i64, y: i64) -> Option<(u8, u8, u8)> {
    if x < 0 || y < 0 || x as u32 >= pixels.width() || y as u32 >= pixels.height() {
        return None;
    }
    let pixel = pixels.get_pixel(x as u32, y as u32);
    Some((pixel[0], pixel[1], pixel[2]))
}

// ------------------------------------------------------------------ shell API

impl OkApp {
    /// Open the markup editor for a template image.
    pub fn open_markup(&mut self, image: String, cx: &mut Context<Self>) {
        let url = self
            .client
            .url(&format!("/api/templates/image/{}", api::url_encode(&image)));
        self.markup.image = image.clone();
        self.markup.url = url.clone();
        self.markup.mode = MarkupTool::None;
        self.markup.selected = None;
        self.markup.hovered = None;
        self.markup.draw_start = None;
        self.markup.draw_preview = None;
        self.markup.probe = None;
        self.markup.draft = None;
        self.markup.confirm_delete = None;
        self.markup.drag = None;
        self.markup.focused = false;
        self.markup.view_for = None;
        self.markup.view = [0.0, 0.0, 1.0, 1.0];
        self.state.annotations = None;
        self.ensure_image(&url);
        self.get(&Self::markup_annotations_path_for(&image), cx);
        self.modal = Some(Modal::Markup);
        cx.notify();
    }

    fn markup_annotations_path_for(image: &str) -> String {
        format!("/api/templates/{}/annotations", api::url_encode(image))
    }

    fn markup_annotations_path(&self) -> String {
        Self::markup_annotations_path_for(&self.markup.image)
    }

    /// The annotation document, when it belongs to the open image.
    fn markup_document(&self) -> Option<TemplateAnnotations> {
        self.state
            .annotations
            .clone()
            .filter(|document| document.name == self.markup.image)
    }

    /// Image size in pixels: prefer the decoded image, fall back to the
    /// document's reported size (the web editor does the same).
    fn markup_image_size(&self, document: Option<&TemplateAnnotations>) -> (f64, f64) {
        if let Some(pixels) = self.pixels.get(&self.markup.url) {
            return (pixels.width() as f64, pixels.height() as f64);
        }
        document
            .map(|document| (document.width.max(1.0), document.height.max(1.0)))
            .unwrap_or((1.0, 1.0))
    }

    fn markup_ensure_view(&mut self, width: f64, height: f64) {
        if self.markup.view_for.as_deref() != Some(self.markup.image.as_str()) {
            self.markup.view = [0.0, 0.0, width, height];
            self.markup.view_for = Some(self.markup.image.clone());
        } else if self.markup.view[2] > width * 1.5 || self.markup.view[3] > height * 1.5 {
            self.markup.view = [0.0, 0.0, width, height];
        }
    }

    fn markup_image_point(&self, local: (f32, f32)) -> (f64, f64) {
        let viewport = self
            .markup
            .viewport
            .borrow()
            .as_ref()
            .map(|bounds| {
                (
                    f64::from(f32::from(bounds.size.width)),
                    f64::from(f32::from(bounds.size.height)),
                )
            })
            .unwrap_or((0.0, 0.0));
        let (scale, offset_x, offset_y) = fit(self.markup.view, viewport);
        if scale <= 0.0 {
            return (0.0, 0.0);
        }
        let view = self.markup.view;
        let x = view[0] + (local.0 as f64 - offset_x) / scale;
        let y = view[1] + (local.1 as f64 - offset_y) / scale;
        (x, y)
    }

    /// Optimistic save: update the shared document, then POST the annotation
    /// list (the shell merges the response back into `state.annotations`).
    fn markup_persist(&mut self, annotations: Vec<TemplateAnnotation>, cx: &mut Context<Self>) {
        if let Some(document) = self.state.annotations.as_mut() {
            document.annotations = annotations.clone();
        }
        let path = self.markup_annotations_path();
        self.post(&path, Some(json!({ "annotations": annotations })), cx);
    }

    fn markup_saving(&self) -> bool {
        let path = self.markup_annotations_path();
        path.ends_with("/annotations") && self.state.pending.contains(&path)
    }

    fn markup_toggle(&mut self, tool: MarkupTool) {
        self.markup.mode = if self.markup.mode == tool {
            MarkupTool::None
        } else {
            tool
        };
        self.markup.draw_start = None;
        self.markup.draw_preview = None;
        self.markup.drag = None;
    }

    /// Load the image at `index` in the template list (`loadImage` in the web UI).
    fn markup_load_image(&mut self, index: i64, cx: &mut Context<Self>) {
        let templates: Vec<TemplateImage> = self.state.templates.clone();
        let Some(template) = usize::try_from(index)
            .ok()
            .and_then(|index| templates.get(index))
        else {
            return;
        };
        let name = template.name.clone();
        let url = self.client.url(&template.url);
        self.markup.image = name.clone();
        self.markup.url = url.clone();
        self.markup.view_for = None;
        self.markup.selected = None;
        self.markup.hovered = None;
        self.markup.mode = MarkupTool::None;
        self.markup.draw_start = None;
        self.markup.draw_preview = None;
        self.markup.draft = None;
        self.markup.confirm_delete = None;
        self.markup.drag = None;
        self.state.annotations = None;
        self.ensure_image(&url);
        self.get(&Self::markup_annotations_path_for(&name), cx);
    }

    fn markup_current_index(&self) -> i64 {
        self.state
            .templates
            .iter()
            .position(|template| template.name == self.markup.image)
            .map(|index| index as i64)
            .unwrap_or(0)
    }

    /// Open the bounding-box form for an existing or new annotation.
    fn markup_open_editor(&mut self, index: Option<usize>, bbox: [f64; 4], cx: &mut Context<Self>) {
        let document = self.markup_document();
        let category = index
            .and_then(|index| document.as_ref().and_then(|doc| doc.annotations.get(index)))
            .map(|annotation| annotation.category.clone())
            .unwrap_or_default();
        self.markup.draft_id += 1;
        let id = self.markup.draft_id;
        self.input_values
            .insert(format!("markup-category-{id}"), category);
        for (name, value) in [
            ("x", bbox[0]),
            ("y", bbox[1]),
            ("width", bbox[2]),
            ("height", bbox[3]),
        ] {
            self.input_values
                .insert(format!("markup-{name}-{id}"), format!("{}", value.round() as i64));
        }
        self.markup.draft = Some(BoxDraft { index, bbox });
        self.markup.mode = MarkupTool::None;
        self.markup.draw_start = None;
        self.markup.draw_preview = None;
        self.markup.focused = false;
        cx.notify();
    }

    fn markup_draft_error(&self) -> Option<String> {
        self.markup.draft.as_ref()?;
        let id = self.markup.draft_id;
        let category = self
            .input_values
            .get(&format!("markup-category-{id}"))
            .cloned()
            .unwrap_or_default();
        let category = category.trim().to_owned();
        if category.is_empty() {
            return Some(t("Name required"));
        }
        let owner = self.state.templates.iter().find(|template| {
            template.name != self.markup.image
                && template
                    .categories
                    .iter()
                    .any(|existing| existing == &category)
        });
        owner.map(|owner| crate::i18n::tv("Already exists in '{name}'", &[("name", &owner.name)]))
    }

    /// Apply the form: create or update the annotation, then persist.
    fn markup_apply_draft(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.markup.draft.clone() else {
            return;
        };
        if self.markup_draft_error().is_some() {
            return;
        }
        let id = self.markup.draft_id;
        let category = self
            .input_values
            .get(&format!("markup-category-{id}"))
            .map(|value| value.trim().to_owned())
            .unwrap_or_default();
        let coordinate = |name: &str, fallback: f64| -> f64 {
            self.input_values
                .get(&format!("markup-{name}-{id}"))
                .and_then(|value| value.trim().parse::<f64>().ok())
                .map(|value| value.max(0.0))
                .unwrap_or(fallback)
        };
        let bbox = [
            coordinate("x", draft.bbox[0]),
            coordinate("y", draft.bbox[1]),
            coordinate("width", draft.bbox[2]),
            coordinate("height", draft.bbox[3]),
        ];
        let document = self.markup_document();
        let (width, height) = self.markup_image_size(document.as_ref());
        let bbox = clamp_box(bbox, width, height);

        let mut annotations = document
            .map(|document| document.annotations)
            .unwrap_or_default();
        let annotation = TemplateAnnotation {
            id: None,
            category,
            bbox: bbox.to_vec(),
        };
        match draft.index {
            Some(index) if index < annotations.len() => annotations[index] = annotation,
            _ => annotations.push(annotation),
        }
        self.markup.selected = draft.index.or_else(|| annotations.len().checked_sub(1));
        self.markup.draft = None;
        self.markup.focused = false;
        self.markup_persist(annotations, cx);
    }

    fn markup_delete(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(document) = self.markup_document() else {
            return;
        };
        let mut annotations = document.annotations;
        if index >= annotations.len() {
            return;
        }
        annotations.remove(index);
        self.markup.selected = None;
        self.markup.hovered = None;
        self.markup.confirm_delete = None;
        self.markup_persist(annotations, cx);
    }

    fn markup_duplicate_selected(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.markup.selected else {
            return;
        };
        let Some(document) = self.markup_document() else {
            return;
        };
        let Some(annotation) = document.annotations.get(index).cloned() else {
            return;
        };
        let (width, height) = self.markup_image_size(Some(&document));
        let bbox = annotation_bbox(&annotation);
        let duplicate = TemplateAnnotation {
            id: None,
            category: format!("{}_copy", annotation.category),
            bbox: clamp_box(
                [bbox[0] + 20.0, bbox[1] + 20.0, bbox[2], bbox[3]],
                width,
                height,
            )
            .to_vec(),
        };
        let mut annotations = document.annotations;
        annotations.push(duplicate);
        self.markup.selected = None;
        self.markup_persist(annotations, cx);
    }

    fn markup_zoom(&mut self, local: (f32, f32), factor: f64, image: (f64, f64)) {
        let viewport = self
            .markup
            .viewport
            .borrow()
            .as_ref()
            .map(|bounds| {
                (
                    f64::from(f32::from(bounds.size.width)),
                    f64::from(f32::from(bounds.size.height)),
                )
            })
            .unwrap_or((0.0, 0.0));
        let (scale, offset_x, offset_y) = fit(self.markup.view, viewport);
        if scale <= 0.0 {
            return;
        }
        let view = self.markup.view;
        let point = (
            view[0] + (local.0 as f64 - offset_x) / scale,
            view[1] + (local.1 as f64 - offset_y) / scale,
        );
        let width = image.0.min((image.0 / 50.0).max(view[2] * factor));
        let height = image.1.min((image.1 / 50.0).max(view[3] * factor));
        let x = (point.0 - (point.0 - view[0]) * width / view[2])
            .clamp(0.0, (image.0 - width).max(0.0));
        let y = (point.1 - (point.1 - view[1]) * height / view[3])
            .clamp(0.0, (image.1 - height).max(0.0));
        self.markup.view = [x, y, width, height];
    }

    /// Update the drag session from a pointer position in image coordinates.
    fn markup_drag_to(&mut self, point: (f64, f64), image: (f64, f64)) {
        let Some(mut drag) = self.markup.drag.clone() else {
            return;
        };
        match drag.kind {
            DragKind::Pan => {
                let next = [
                    (drag.original_view[0] - (point.0 - drag.start.0))
                        .clamp(0.0, (image.0 - drag.original_view[2]).max(0.0)),
                    (drag.original_view[1] - (point.1 - drag.start.1))
                        .clamp(0.0, (image.1 - drag.original_view[3]).max(0.0)),
                    drag.original_view[2],
                    drag.original_view[3],
                ];
                self.markup.view = next;
                self.markup.drag = Some(drag);
                return;
            }
            DragKind::Move | DragKind::Resize => {}
        }
        let [ox, oy, ow, oh] = drag.original_box;
        let dx = point.0 - drag.start.0;
        let dy = point.1 - drag.start.1;
        if dx.abs() < 1.0 && dy.abs() < 1.0 {
            self.markup.drag = Some(drag);
            return;
        }
        drag.changed = true;
        let mut next = if drag.kind == DragKind::Move {
            [ox + dx, oy + dy, ow, oh]
        } else {
            [ox, oy, ow, oh]
        };
        if drag.kind == DragKind::Resize {
            if let Some(handle) = drag.handle {
                if handle.contains('l') {
                    next[0] = ox + dx;
                    next[2] = ow - dx;
                }
                if handle.contains('r') {
                    next[2] = ow + dx;
                }
                if handle.contains('t') {
                    next[1] = oy + dy;
                    next[3] = oh - dy;
                }
                if handle.contains('b') {
                    next[3] = oh + dy;
                }
            }
        }
        let next = clamp_box(next, image.0, image.1);
        if let Some(document) = self.state.annotations.as_mut() {
            if let Some(annotation) = document.annotations.get_mut(drag.index) {
                annotation.bbox = next.to_vec();
            }
        }
        self.markup.drag = Some(drag);
    }

    fn markup_finish_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.markup.drag.take() else {
            return;
        };
        if drag.changed && drag.kind != DragKind::Pan {
            if let Some(document) = self.markup_document() {
                self.markup_persist(document.annotations, cx);
            }
        }
    }
}

// ------------------------------------------------------------------ rendering

impl OkApp {
    /// Render the full-window markup editor.
    pub fn render_markup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let dark = ui::is_dark(cx);
        let document = self.markup_document();
        let url = self.markup.url.clone();
        if !url.is_empty() {
            self.ensure_image(&url);
        }
        let (image_width, image_height) = self.markup_image_size(document.as_ref());
        self.markup_ensure_view(image_width, image_height);
        let image = (image_width, image_height);

        // Focus the canvas while the form is closed so the shortcuts work.
        if !self.markup.focused && self.markup.draft.is_none() {
            let handle = self.markup.focus.get_or_insert_with(|| cx.focus_handle());
            window.focus(handle);
            self.markup.focused = true;
        }
        let focus = self.markup.focus.clone();

        let annotations = document
            .as_ref()
            .map(|document| document.annotations.clone())
            .unwrap_or_default();
        let saving = self.markup_saving();
        let view = self.markup.view;
        let viewport = self.markup.viewport.borrow().clone();
        let viewport_size = viewport
            .as_ref()
            .map(|bounds| {
                (
                    f64::from(f32::from(bounds.size.width)),
                    f64::from(f32::from(bounds.size.height)),
                )
            })
            .unwrap_or((0.0, 0.0));
        let (scale, offset_x, offset_y) = fit(view, viewport_size);
        let zoomed = view[2] < image_width - 0.5 || view[3] < image_height - 0.5;

        // ---- toolbar
        let tool_button = |id: &'static str,
                           label: String,
                           icon: OkIcon,
                           active: bool,
                           cx: &mut Context<Self>| {
            let button = if active {
                ui::primary_button(id, label)
            } else {
                ui::secondary_button(id, label, cx)
            };
            button.icon(icon.icon())
        };

        let draw_button = tool_button(
            "markup-draw",
            t("Draw (R)"),
            OkIcon::Edit,
            self.markup.mode == MarkupTool::Draw,
            cx,
        )
        .on_click(cx.listener(|view, _, _, cx| {
            view.markup_toggle(MarkupTool::Draw);
            cx.notify();
        }));
        let delete_button = tool_button(
            "markup-delete",
            t("Delete (D)"),
            OkIcon::Delete,
            self.markup.mode == MarkupTool::Delete,
            cx,
        )
        .on_click(cx.listener(|view, _, _, cx| {
            view.markup_toggle(MarkupTool::Delete);
            cx.notify();
        }));
        let modify_button = ui::secondary_button("markup-modify", t("Modify (Double Click)"), cx)
            .icon(OkIcon::Settings.icon())
            .disabled(self.markup.selected.is_none())
            .on_click(cx.listener(|view, _, _, cx| {
                if let Some(index) = view.markup.selected {
                    let bbox = view
                        .markup_document()
                        .and_then(|document| document.annotations.get(index).cloned())
                        .map(|annotation| annotation_bbox(&annotation))
                        .unwrap_or([0.0, 0.0, MIN_BOX_SIZE, MIN_BOX_SIZE]);
                    view.markup_open_editor(Some(index), bbox, cx);
                }
            }));

        let probe_swatch: Hsla = match self.markup.probe {
            Some(probe) => gpui::rgb(
                (probe.r as u32) << 16 | (probe.g as u32) << 8 | probe.b as u32,
            )
            .into(),
            None => gpui::rgba(0x00000000).into(),
        };
        let probe_label = match self.markup.probe {
            Some(probe) => format!(
                "R:{} G:{} B:{}  Abs: ({}, {})  Rel: ({:.3}, {:.3}) ({})",
                probe.r,
                probe.g,
                probe.b,
                probe.x,
                probe.y,
                probe.rel_x,
                probe.rel_y,
                t("Right click to copy color"),
            ),
            None => String::new(),
        };

        let toolbar = div()
            .h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .p(px(8.0))
            .flex_shrink_0()
            .child(draw_button)
            .child(delete_button)
            .child(modify_button)
            .child(
                div()
                    .flex_none()
                    .size(px(16.0))
                    .rounded(px(2.0))
                    .border_1()
                    .border_color(gpui::rgb(0x888888))
                    .bg(probe_swatch),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(px(ui::FS_TINY))
                    .text_color(cx.theme().muted_foreground)
                    .child(probe_label),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(ui::FS_TINY))
                    .text_color(cx.theme().muted_foreground)
                    .child(if saving { t("Saving...") } else { String::new() }),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(ui::FS_SMALL))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} ({}x{})",
                        self.markup.image,
                        image_width.round() as i64,
                        image_height.round() as i64
                    )),
            );

        // ---- annotation layers
        let mut layers: Vec<AnyElement> = Vec::new();
        for (index, annotation) in annotations.iter().enumerate() {
            let bbox = annotation_bbox(annotation);
            let (sx, sy, sw, sh) = rect_on_screen(bbox, view, scale, offset_x, offset_y);
            if sw <= 0.0 || sh <= 0.0 {
                continue;
            }
            let selected = self.markup.selected == Some(index);
            let hovered = self.markup.hovered == Some(index);
            let color = box_color(annotation, index, selected, hovered);
            let label = annotation.category.clone();
            let bbox_for_editor = bbox;
            layers.push(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "markup-box-{index}"
                    ))))
                    .absolute()
                    .left(px(sx - BOX_HIT_PADDING as f32))
                    .top(px(sy - BOX_HIT_PADDING as f32))
                    .w(px(sw + BOX_HIT_PADDING as f32 * 2.0))
                    .h(px(sh + BOX_HIT_PADDING as f32 * 2.0))
                    .p(px(BOX_HIT_PADDING as f32))
                    .cursor_move()
                    .on_mouse_move(cx.listener(move |view, _, _, cx| {
                        if view.markup.hovered != Some(index) {
                            view.markup.hovered = Some(index);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &gpui::MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                            if view.markup.mode == MarkupTool::Delete {
                                view.markup.confirm_delete = Some(index);
                                cx.notify();
                                return;
                            }
                            if view.markup.mode == MarkupTool::Draw {
                                return;
                            }
                            view.markup.selected = Some(index);
                            if event.click_count >= 2 {
                                view.markup_open_editor(Some(index), bbox_for_editor, cx);
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(MouseButton::Right, cx.listener(|view, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(probe) = view.markup.probe {
                            let text = format!("{},{},{}", probe.r, probe.g, probe.b);
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                        }
                    }))
                    .child(
                        div()
                            .size_full()
                            .border_2()
                            .border_color(color)
                            .bg(box_fill(color))
                            .rounded(px(1.0)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(BOX_HIT_PADDING as f32 + 4.0))
                            .top(px(BOX_HIT_PADDING as f32 - LABEL_SIZE - 2.0))
                            .text_size(px(LABEL_SIZE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(color)
                            .child(label),
                    )
                    .into_any_element(),
            );
        }

        if let (Some(start), Some(preview)) = (self.markup.draw_start, self.markup.draw_preview) {
            let bbox = [
                start.0.min(preview.0),
                start.1.min(preview.1),
                (preview.0 - start.0).abs(),
                (preview.1 - start.1).abs(),
            ];
            let (sx, sy, sw, sh) = rect_on_screen(bbox, view, scale, offset_x, offset_y);
            layers.push(
                div()
                    .absolute()
                    .left(px(sx))
                    .top(px(sy))
                    .w(px(sw.max(1.0)))
                    .h(px(sh.max(1.0)))
                    .border_2()
                    .border_dashed()
                    .border_color(gpui::rgb(0x00c800))
                    .bg(gpui::rgba(0x00c80024))
                    .into_any_element(),
            );
        }

        let selected_box = self.markup.selected.and_then(|index| {
            annotations
                .get(index)
                .map(|annotation| annotation_bbox(annotation))
        });
        if let Some(bbox) = selected_box {
            let (sx, sy, sw, sh) = rect_on_screen(bbox, view, scale, offset_x, offset_y);
            for (handle, x, y) in [
                ("tl", sx, sy),
                ("tr", sx + sw, sy),
                ("bl", sx, sy + sh),
                ("br", sx + sw, sy + sh),
            ] {
                let index = self.markup.selected.unwrap_or(0);
                let bbox = bbox;
                layers.push(
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "markup-handle-{handle}"
                        ))))
                        .absolute()
                        .left(px(x - HANDLE_SIZE / 2.0))
                        .top(px(y - HANDLE_SIZE / 2.0))
                        .size(px(HANDLE_SIZE))
                        .rounded_full()
                        .border_1()
                        .border_color(gpui::rgb(0xffffff))
                        .bg(gpui::rgb(0x00c800))
                        .when(handle == "tl" || handle == "br", |this| {
                            this.cursor_nwse_resize()
                        })
                        .when(handle == "tr" || handle == "bl", |this| {
                            this.cursor_nesw_resize()
                        })
                        .on_mouse_down(MouseButton::Left, cx.listener(
                            move |view, event: &gpui::MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                                if view.markup.mode != MarkupTool::None {
                                    return;
                                }
                                let Some(bounds) = view.markup.viewport.borrow().clone() else {
                                    return;
                                };
                                let local = (
                                    f32::from(event.position.x) - f32::from(bounds.origin.x),
                                    f32::from(event.position.y) - f32::from(bounds.origin.y),
                                );
                                let point = view.markup_image_point(local);
                                view.markup.selected = Some(index);
                                view.markup.drag = Some(BoxDrag {
                                    kind: DragKind::Resize,
                                    index,
                                    handle: Some(handle),
                                    start: point,
                                    original_box: bbox,
                                    original_view: view.markup.view,
                                    changed: false,
                                });
                                cx.notify();
                            },
                        ))
                        .into_any_element(),
                );
            }
        }

        // ---- measuring layer + viewport
        let bounds_for_canvas = self.markup.viewport.clone();
        let weak = cx.entity().downgrade();
        let measurer = canvas(
            move |bounds, _window, cx| {
                let changed = {
                    let current = bounds_for_canvas.borrow();
                    current.as_ref() != Some(&bounds)
                };
                if changed {
                    *bounds_for_canvas.borrow_mut() = Some(bounds);
                    let _ = weak.update(cx, |_view, cx| cx.notify());
                }
            },
            |_bounds, _state, _window, _cx| {},
        )
        .absolute()
        .inset_0();

        let mut viewport_div = div()
            .id("markup-viewport")
            .relative()
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .overflow_hidden()
            .rounded(px(5.0))
            .bg(if dark {
                gpui::rgb(0x1e1e1e)
            } else {
                gpui::rgb(0xe8e5e7)
            })
            .when(self.markup.mode == MarkupTool::Draw, |this| {
                this.cursor_crosshair()
            })
            .when(
                self.markup.mode == MarkupTool::None && zoomed,
                |this| this.cursor_grab(),
            )
            .child(measurer)
            .when_some(
                self.images.get(&url).cloned(),
                |this, image_handle| {
                    this.child(
                        div()
                            .absolute()
                            .left(px(offset_x as f32))
                            .top(px(offset_y as f32))
                            .w(px((view[2] * scale) as f32))
                            .h(px((view[3] * scale) as f32))
                            .child(
                                img(image_handle)
                                    .size_full()
                                    .object_fit(ObjectFit::Fill),
                            ),
                    )
                },
            )
            .children(layers)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &gpui::MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                    let Some(bounds) = view.markup.viewport.borrow().clone() else {
                        return;
                    };
                    let local = (
                        f32::from(event.position.x) - f32::from(bounds.origin.x),
                        f32::from(event.position.y) - f32::from(bounds.origin.y),
                    );
                    let point = view.markup_image_point(local);
                    match view.markup.mode {
                        MarkupTool::Draw => {
                            if let Some(start) = view.markup.draw_start {
                                let bbox = clamp_box(
                                    [
                                        start.0.min(point.0),
                                        start.1.min(point.1),
                                        (point.0 - start.0).abs(),
                                        (point.1 - start.1).abs(),
                                    ],
                                    image.0,
                                    image.1,
                                );
                                view.markup.draw_start = None;
                                view.markup.draw_preview = None;
                                view.markup.mode = MarkupTool::None;
                                if bbox[2] >= MIN_BOX_SIZE && bbox[3] >= MIN_BOX_SIZE {
                                    view.markup_open_editor(None, bbox, cx);
                                }
                            } else {
                                view.markup.draw_start = Some(point);
                                view.markup.draw_preview = Some(point);
                            }
                        }
                        MarkupTool::Delete => {
                            view.markup.selected = None;
                        }
                        MarkupTool::None => {
                            view.markup.selected = None;
                            let zoomed = view.markup.view[2] < image.0 - 0.5
                                || view.markup.view[3] < image.1 - 0.5;
                            if zoomed {
                                view.markup.drag = Some(BoxDrag {
                                    kind: DragKind::Pan,
                                    index: 0,
                                    handle: None,
                                    start: point,
                                    original_box: [0.0, 0.0, 0.0, 0.0],
                                    original_view: view.markup.view,
                                    changed: false,
                                });
                            }
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |view, event: &gpui::MouseMoveEvent, _window, cx| {
                let Some(bounds) = view.markup.viewport.borrow().clone() else {
                    return;
                };
                let local = (
                    f32::from(event.position.x) - f32::from(bounds.origin.x),
                    f32::from(event.position.y) - f32::from(bounds.origin.y),
                );
                let point = view.markup_image_point(local);
                if point.0 >= 0.0
                    && point.1 >= 0.0
                    && point.0 < image.0
                    && point.1 < image.1
                {
                    if let Some(pixels) = view.pixels.get(&url) {
                        let x = point.0.floor() as i64;
                        let y = point.1.floor() as i64;
                        if let Some((r, g, b)) = read_pixel(pixels, x, y) {
                            view.markup.probe = Some(Probe {
                                r,
                                g,
                                b,
                                x,
                                y,
                                rel_x: point.0 / image.0,
                                rel_y: point.1 / image.1,
                            });
                        }
                    }
                }
                if view.markup.mode == MarkupTool::Draw && view.markup.draw_start.is_some() {
                    view.markup.draw_preview = Some(point);
                }
                view.markup_drag_to(point, image);
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.markup_finish_drag(cx);
                    cx.notify();
                }),
            )
            .on_mouse_down(MouseButton::Right, cx.listener(|view, _, _, cx| {
                cx.stop_propagation();
                if let Some(probe) = view.markup.probe {
                    let text = format!("{},{},{}", probe.r, probe.g, probe.b);
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }))
            .on_scroll_wheel(cx.listener(move |view, event: &gpui::ScrollWheelEvent, _window, cx| {
                let Some(bounds) = view.markup.viewport.borrow().clone() else {
                    return;
                };
                let local = (
                    f32::from(event.position.x) - f32::from(bounds.origin.x),
                    f32::from(event.position.y) - f32::from(bounds.origin.y),
                );
                let delta = match event.delta {
                    ScrollDelta::Pixels(point) => f32::from(point.y),
                    ScrollDelta::Lines(point) => point.y,
                };
                let factor = if delta < 0.0 { 0.9 } else { 1.1 };
                view.markup_zoom(local, factor, image);
                cx.notify();
            }))
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _window, cx| {
                if view.markup.draft.is_some() || view.markup.confirm_delete.is_some() {
                    return;
                }
                let modifiers = event.keystroke.modifiers;
                let key = event.keystroke.key.to_ascii_lowercase();
                if modifiers.control && key == "c" {
                    if view.markup.selected.is_some() {
                        view.markup_duplicate_selected(cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if modifiers.control || modifiers.alt || modifiers.platform {
                    return;
                }
                match key.as_str() {
                    "r" => {
                        view.markup_toggle(MarkupTool::Draw);
                        cx.stop_propagation();
                        cx.notify();
                    }
                    "d" => {
                        view.markup_toggle(MarkupTool::Delete);
                        cx.stop_propagation();
                        cx.notify();
                    }
                    "delete" | "backspace" => {
                        if let Some(index) = view.markup.selected {
                            view.markup.confirm_delete = Some(index);
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }
                    "left" | "arrowleft" => {
                        let index = view.markup_current_index() - 1;
                        view.markup_load_image(index, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                    "right" | "arrowright" => {
                        let index = view.markup_current_index() + 1;
                        view.markup_load_image(index, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                    _ => {}
                }
            }));
        if let Some(handle) = focus.clone() {
            viewport_div = viewport_div.track_focus(&handle);
        }

        let current_index = self.markup_current_index();
        let template_count = self.state.templates.len() as i64;
        let prev_button = nav_button("markup-prev", OkIcon::ArrowLeft, current_index <= 0, cx)
            .on_click(cx.listener(move |view, _, _, cx| {
                view.markup_load_image(current_index - 1, cx);
            }));
        let next_button = nav_button(
            "markup-next",
            OkIcon::ArrowRight,
            current_index >= template_count - 1,
            cx,
        )
        .on_click(cx.listener(move |view, _, _, cx| {
            view.markup_load_image(current_index + 1, cx);
        }));

        let content = div()
            .relative()
            .h_flex()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .items_center()
            .gap_2()
            .px(px(4.0))
            .pb(px(4.0))
            .child(prev_button)
            .child(viewport_div)
            .child(next_button);

        let header = ui::modal_header(t("Markup Editor"), cx).child(
            ui::icon_button("markup-close", OkIcon::Close, cx)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.modal = None;
                    cx.notify();
                })),
        );

        let mut frame = div()
            .id("markup-content")
            .v_flex()
            .size_full()
            .m(px(3.0))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(Tokens::modal(dark))
            .text_color(cx.theme().foreground)
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| {
                cx.stop_propagation()
            }))
            .child(header)
            .child(toolbar)
            .child(content);

        if document.is_none() {
            frame = frame.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x00000080))
                    .child(ui::busy_spinner(cx)),
            );
        }

        if let Some(index) = self.markup.confirm_delete {
            let name = annotations
                .get(index)
                .map(|annotation| annotation.category.clone())
                .unwrap_or_default();
            frame = frame.child(
                div()
                    .id("markup-confirm")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x0c070aad))
                    .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                    .child(
                        ui::modal_frame(420.0, cx)
                            .child(ui::modal_header(t("Confirm Delete"), cx))
                            .child(ui::modal_body().child(
                                div()
                                    .whitespace_normal()
                                    .text_size(px(ui::FS_BODY))
                                    .child(crate::i18n::tv(
                                        "Are you sure you want to delete '{name}'?",
                                        &[("name", &name)],
                                    )),
                            ))
                            .child(
                                ui::modal_footer()
                                    .child(
                                        ui::secondary_button("markup-cancel-delete", t("Cancel"), cx)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.markup.confirm_delete = None;
                                                view.markup.focused = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        ui::primary_button("markup-confirm-delete", t("Delete"))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.markup_delete(index, cx);
                                                view.markup.focused = false;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            );
        }

        if self.markup.draft.is_some() {
            frame = frame.child(self.render_bbox_editor(window, cx));
        }

        frame.into_any_element()
    }

    /// The bounding-box form (`BBoxEditor` in the web editor).
    fn render_bbox_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let id = self.markup.draft_id;
        let document = self.markup_document();
        let (image_width, image_height) = self.markup_image_size(document.as_ref());
        let category = self.ensure_input(
            window,
            cx,
            &format!("markup-category-{id}"),
            &t("Category name"),
            "",
            false,
        );
        let x = self.ensure_input(window, cx, &format!("markup-x-{id}"), "X", "0", false);
        let y = self.ensure_input(window, cx, &format!("markup-y-{id}"), "Y", "0", false);
        let width = self.ensure_input(
            window,
            cx,
            &format!("markup-width-{id}"),
            &t("Width"),
            "0",
            false,
        );
        let height = self.ensure_input(
            window,
            cx,
            &format!("markup-height-{id}"),
            &t("Height"),
            "0",
            false,
        );
        let error = self.markup_draft_error();
        let _ = (image_width, image_height);

        let field = |label: String, input: &gpui::Entity<gpui_component::input::InputState>| {
            div()
                .h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .text_size(px(ui::FS_SMALL))
                .child(div().w(px(92.0)).flex_none().child(label))
                .child(div().flex_1().child(Input::new(input)))
        };

        let form = div()
            .v_flex()
            .w_full()
            .gap_2()
            .child(field(format!("{}:", t("Category:")), &category))
            .when_some(error.clone(), |this, error| {
                this.child(
                    div()
                        .pl(px(92.0))
                        .text_size(px(ui::FS_TINY))
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(field("X:".to_owned(), &x))
            .child(field("Y:".to_owned(), &y))
            .child(field(format!("{}:", t("Width")), &width))
            .child(field(format!("{}:", t("Height")), &height));

        let document = div()
            .id("markup-bbox")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x0c070aad))
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(
                ui::modal_frame(430.0, cx)
                    .child(
                        ui::modal_header(t("Bounding Box"), cx).child(
                            ui::icon_button("markup-bbox-close", OkIcon::Close, cx).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.markup.draft = None;
                                    view.markup.focused = false;
                                    cx.notify();
                                }),
                            ),
                        ),
                    )
                    .child(ui::modal_body().child(form))
                    .child(
                        ui::modal_footer()
                            .child(
                                ui::secondary_button("markup-bbox-cancel", t("Cancel"), cx)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.markup.draft = None;
                                        view.markup.focused = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                ui::primary_button("markup-bbox-ok", t("OK"))
                                    .disabled(error.is_some())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.markup_apply_draft(cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        document.into_any_element()
    }
}

fn nav_button(
    id: &'static str,
    icon: OkIcon,
    disabled: bool,
    cx: &mut Context<OkApp>,
) -> Button {
    Button::new(id)
        .icon(icon.icon())
        .ghost()
        .w(px(36.0))
        .h(px(36.0))
        .rounded_full()
        .bg(ui::button_surface(cx))
        .disabled(disabled)
        .when(disabled, |this| this.opacity(0.25))
}
