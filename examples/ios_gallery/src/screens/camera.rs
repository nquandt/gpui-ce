//! "Camera": camera enumeration, live preview platform view, still capture,
//! flash/focus/zoom controls, image_picker, and permission_handler status.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, img, prelude::*, px, rgb};
use gpui_mobile::components::platform_view_element::platform_view_element;
use gpui_mobile::packages::camera::{
    self, CameraDescription, CameraHandle, CameraLensDirection, CapturedImage, FlashMode,
    FocusMode, ResolutionPreset,
};
use gpui_mobile::packages::image_picker::{self, ImagePickerOptions, ImageSource, PickedFile};
use gpui_mobile::packages::permission_handler::{self, Permission, PermissionStatus};
use std::path::PathBuf;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "camera",
        title: "Camera",
        category: "Hardware & media",
        blurb: "capture, preview, image_picker, permissions",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| CameraScreen {
        cameras: camera::available_cameras(),
        handle: None,
        preview_active: false,
        status: String::new(),
        captured: None,
        picked: None,
        zoom: 1.0,
    })
    .into()
}

struct CameraScreen {
    cameras: Result<Vec<CameraDescription>, String>,
    handle: Option<CameraHandle>,
    preview_active: bool,
    status: String,
    captured: Option<CapturedImage>,
    picked: Option<PickedFile>,
    zoom: f64,
}

fn permission_label(p: PermissionStatus) -> &'static str {
    match p {
        PermissionStatus::Granted => "Granted",
        PermissionStatus::Denied => "Denied",
        PermissionStatus::Restricted => "Restricted",
        PermissionStatus::PermanentlyDenied => "PermanentlyDenied",
        PermissionStatus::Limited => "Limited",
        PermissionStatus::Provisional => "Provisional",
    }
}

impl CameraScreen {
    fn find(&self, direction: CameraLensDirection) -> Option<CameraDescription> {
        self.cameras
            .as_ref()
            .ok()?
            .iter()
            .find(|c| c.lens_direction == direction)
            .cloned()
    }

    fn create(&mut self, direction: CameraLensDirection) {
        // Dispose any existing session first.
        if let Some(handle) = self.handle.take() {
            let _ = camera::dispose(handle);
        }
        self.preview_active = false;
        let Some(desc) = self.find(direction) else {
            self.status = format!("no camera found for {direction:?}");
            return;
        };
        match camera::create_camera(&desc, ResolutionPreset::Medium, false) {
            Ok(h) => {
                self.status = format!("created camera session (id {})", h.id);
                self.handle = Some(h);
                gallery_log::push(format!("camera: created {direction:?} session"));
            }
            Err(e) => self.status = format!("create_camera error: {e}"),
        }
    }

    fn start_preview(&mut self) {
        let Some(handle) = self.handle.as_mut() else {
            self.status = "no camera session — create one first".into();
            return;
        };
        match camera::start_preview(handle) {
            Ok(()) => {
                self.preview_active = true;
                self.status = "preview started".into();
                gallery_log::push("camera: preview started");
            }
            Err(e) => self.status = format!("start_preview error: {e}"),
        }
    }

    fn stop_preview(&mut self) {
        let Some(handle) = self.handle.as_mut() else {
            return;
        };
        match camera::stop_preview(handle) {
            Ok(()) => {
                self.preview_active = false;
                self.status = "preview stopped".into();
                gallery_log::push("camera: preview stopped");
            }
            Err(e) => self.status = format!("stop_preview error: {e}"),
        }
    }

    fn take_picture(&mut self) {
        let Some(handle) = self.handle.as_ref() else {
            self.status = "no camera session — create one first".into();
            return;
        };
        match camera::take_picture(handle) {
            Ok(image) => {
                self.status = format!(
                    "captured {}x{} -> {}",
                    image.width, image.height, image.path
                );
                gallery_log::push(format!("camera: captured photo -> {}", image.path));
                self.captured = Some(image);
            }
            Err(e) => self.status = format!("take_picture error: {e}"),
        }
    }
}

impl Render for CameraScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let camera_status = permission_handler::check_permission(Permission::Camera);
        let photos_status = permission_handler::check_permission(Permission::Photos);

        div()
            .id("camera-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(div().text_color(rgb(0xffffff)).text_size(px(20.0)).child("Camera"))
            .child(note(
                "Preview, capture and pick images. Everything here runs from a button tap so \
                 permission prompts happen at the right time.",
            ))
            .child(
                section("permission_handler")
                    .child(kv(
                        "camera",
                        camera_status
                            .map(permission_label)
                            .unwrap_or("error")
                            .to_string(),
                    ))
                    .child(kv(
                        "photos",
                        photos_status
                            .map(permission_label)
                            .unwrap_or("error")
                            .to_string(),
                    ))
                    .child(row().flex_wrap()
                        .child(button(
                            "Request camera",
                            cx.listener(|this, _, _window, cx| {
                                match permission_handler::request_permission(Permission::Camera) {
                                    Ok(s) => this.status = format!("camera permission -> {}", permission_label(s)),
                                    Err(e) => this.status = format!("error: {e}"),
                                }
                                gallery_log::push(format!("camera: request camera permission -> {}", this.status));
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Request photos",
                            cx.listener(|this, _, _window, cx| {
                                match permission_handler::request_permission(Permission::Photos) {
                                    Ok(s) => this.status = format!("photos permission -> {}", permission_label(s)),
                                    Err(e) => this.status = format!("error: {e}"),
                                }
                                gallery_log::push(format!("camera: request photos permission -> {}", this.status));
                                cx.notify();
                            }),
                        ))),
            )
            .child(section("available_cameras()").children(match &self.cameras {
                Ok(cams) if !cams.is_empty() => cams
                    .iter()
                    .map(|c| kv(c.name.clone(), format!("{:?} (orientation {})", c.lens_direction, c.sensor_orientation)))
                    .collect::<Vec<_>>(),
                Ok(_) => vec![div().child(note("no cameras reported"))],
                Err(e) => vec![kv("error", e.clone())],
            }))
            .child(
                section("session")
                    .child(row().flex_wrap()
                        .child(button(
                            "Create front",
                            cx.listener(|this, _, _window, cx| {
                                this.create(CameraLensDirection::Front);
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Create back",
                            cx.listener(|this, _, _window, cx| {
                                this.create(CameraLensDirection::Back);
                                cx.notify();
                            }),
                        )))
                    .child(row().flex_wrap()
                        .child(button(
                            "Start preview",
                            cx.listener(|this, _, _window, cx| {
                                this.start_preview();
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Stop preview",
                            cx.listener(|this, _, _window, cx| {
                                this.stop_preview();
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Take picture",
                            cx.listener(|this, _, _window, cx| {
                                this.take_picture();
                                cx.notify();
                            }),
                        )))
                    .child(
                        div()
                            .w(px(300.0))
                            .h(px(400.0))
                            .bg(rgb(0x111111))
                            .child(
                                self.handle
                                    .as_ref()
                                    .filter(|_| self.preview_active)
                                    .and_then(camera::preview_platform_view_handle)
                                    .map(platform_view_element)
                                    .unwrap_or_else(|| {
                                        div()
                                            .size_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(note("(no active preview)"))
                                    }),
                            ),
                    )
                    .child(if let Some(img_info) = &self.captured {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(note(format!("captured: {}", img_info.path)))
                            .child(img(PathBuf::from(&img_info.path)).w(px(200.0)).h(px(200.0)))
                    } else {
                        div()
                    }),
            )
            .child(
                section("flash / focus / zoom")
                    .child(row().flex_wrap()
                        .child(button(
                            "Flash off",
                            cx.listener(|this, _, _window, cx| this.apply_flash(FlashMode::Off, cx)),
                        ))
                        .child(button(
                            "Flash auto",
                            cx.listener(|this, _, _window, cx| this.apply_flash(FlashMode::Auto, cx)),
                        ))
                        .child(button(
                            "Flash on",
                            cx.listener(|this, _, _window, cx| this.apply_flash(FlashMode::Always, cx)),
                        )))
                    .child(row().flex_wrap()
                        .child(button(
                            "Focus auto",
                            cx.listener(|this, _, _window, cx| this.apply_focus(FocusMode::Auto, cx)),
                        ))
                        .child(button(
                            "Focus locked",
                            cx.listener(|this, _, _window, cx| this.apply_focus(FocusMode::Locked, cx)),
                        )))
                    .child(row().flex_wrap()
                        .child(button(
                            "Zoom -",
                            cx.listener(|this, _, _window, cx| this.apply_zoom(this.zoom - 0.5, cx)),
                        ))
                        .child(kv("zoom", format!("{:.1}x", self.zoom)))
                        .child(button(
                            "Zoom +",
                            cx.listener(|this, _, _window, cx| this.apply_zoom(this.zoom + 0.5, cx)),
                        ))),
            )
            .child(
                section("image_picker")
                    .child(note("Opens the system photo picker (PHPickerViewController). Runs synchronously from this tap."))
                    .child(button(
                        "Pick from library",
                        cx.listener(|this, _, _window, cx| {
                            let options = ImagePickerOptions {
                                source: ImageSource::Gallery,
                                ..Default::default()
                            };
                            match image_picker::pick_image(&options) {
                                Ok(Some(file)) => {
                                    this.status = format!("picked {}", file.name);
                                    gallery_log::push(format!("camera: image_picker picked {}", file.path));
                                    this.picked = Some(file);
                                }
                                Ok(None) => this.status = "picker cancelled".into(),
                                Err(e) => this.status = format!("pick_image error: {e}"),
                            }
                            cx.notify();
                        }),
                    ))
                    .child(if let Some(file) = &self.picked {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(note(format!("picked: {} ({})", file.name, file.path)))
                            .child(img(PathBuf::from(&file.path)).w(px(200.0)).h(px(200.0)))
                    } else {
                        div()
                    }),
            )
            .child(if self.status.is_empty() {
                div()
            } else {
                section("status").child(note(self.status.clone()))
            })
    }
}

impl CameraScreen {
    fn apply_flash(&mut self, mode: FlashMode, cx: &mut Context<Self>) {
        if let Some(handle) = self.handle.as_ref() {
            match camera::set_flash_mode(handle, mode) {
                Ok(()) => self.status = format!("flash -> {mode:?}"),
                Err(e) => self.status = format!("set_flash_mode error: {e}"),
            }
        } else {
            self.status = "no camera session".into();
        }
        cx.notify();
    }

    fn apply_focus(&mut self, mode: FocusMode, cx: &mut Context<Self>) {
        if let Some(handle) = self.handle.as_ref() {
            match camera::set_focus_mode(handle, mode) {
                Ok(()) => self.status = format!("focus -> {mode:?}"),
                Err(e) => self.status = format!("set_focus_mode error: {e}"),
            }
        } else {
            self.status = "no camera session".into();
        }
        cx.notify();
    }

    fn apply_zoom(&mut self, zoom: f64, cx: &mut Context<Self>) {
        let zoom = zoom.max(1.0);
        if let Some(handle) = self.handle.as_ref() {
            match camera::set_zoom(handle, zoom) {
                Ok(()) => {
                    self.zoom = zoom;
                    self.status = format!("zoom -> {zoom:.1}x");
                }
                Err(e) => self.status = format!("set_zoom error: {e}"),
            }
        } else {
            self.status = "no camera session".into();
        }
        cx.notify();
    }
}
