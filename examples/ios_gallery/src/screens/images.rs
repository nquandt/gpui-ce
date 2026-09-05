//! "Images & SVG" screen: an embedded PNG, a remote image, an inline SVG
//! (rendered from raw bytes, since no `AssetSource` is registered for this
//! app), an animated GIF, sizing modes, rounded/clipped images, and
//! loading/error states.

use super::ScreenDescriptor;
use super::common::{note, row, section};
use gpui::{
    AnyView, App, Context, Image, ImageFormat, ObjectFit, Window, div, img, prelude::*, px, rgb,
};
use std::sync::Arc;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "images",
        title: "Images & SVG",
        category: "Core UI",
        blurb: "embedded PNG, remote image, SVG, GIF",
        build,
    }
}

const EMBEDDED_PNG: &[u8] = include_bytes!("../../assets/gradient64.png");

const INLINE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
  <rect width="64" height="64" rx="12" fill="#2c2c34"/>
  <circle cx="32" cy="32" r="22" fill="#7fb0ff"/>
  <path d="M12 48 L32 16 L52 48 Z" fill="none" stroke="#ff9a3c" stroke-width="3"/>
</svg>"##;

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| ImagesScreen).into()
}

struct ImagesScreen;

impl Render for ImagesScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let embedded = Arc::new(Image::from_bytes(ImageFormat::Png, EMBEDDED_PNG.to_vec()));
        let svg_image = Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            INLINE_SVG.as_bytes().to_vec(),
        ));

        div()
            .id("images-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Images & SVG"),
            )
            .child(
                section("Embedded PNG").child(
                    img(embedded)
                        .w(px(64.0))
                        .h(px(64.0))
                        .with_fallback(|| div().child("failed to decode").into_any_element()),
                ),
            )
            .child(
                section("Remote image (HTTPS)")
                    .child(note(
                        "Loads https://picsum.photos/300/200 over the network — requires \
                         connectivity. Shows a placeholder while loading and an error \
                         message if the request fails.",
                    ))
                    .child(
                        img("https://picsum.photos/300/200")
                            .w(px(200.0))
                            .h(px(133.0))
                            .rounded_md()
                            .with_loading(|| {
                                div()
                                    .text_color(rgb(0x9aa0a6))
                                    .child("loading…")
                                    .into_any_element()
                            })
                            .with_fallback(|| {
                                div()
                                    .text_color(rgb(0xff8080))
                                    .child("failed to load remote image")
                                    .into_any_element()
                            }),
                    ),
            )
            .child(
                section("Inline SVG")
                    .child(note(
                        "Rendered from raw bytes via `Image::from_bytes(ImageFormat::Svg, \
                         ..)` — this app registers no `AssetSource` \
                         (crates/gpui/src/assets.rs), so `svg().path(\"...\")` (which \
                         loads from an asset source) would not work here; constructing \
                         the `Image` directly and passing it to `img()` sidesteps that.",
                    ))
                    .child(img(svg_image).w(px(64.0)).h(px(64.0))),
            )
            .child(
                section("Animated GIF (HTTPS)")
                    .child(note(
                        "Loads a small animated GIF over the network; frames should \
                         cycle if multi-frame decoding + redraw scheduling works.",
                    ))
                    .child(
                        img("https://upload.wikimedia.org/wikipedia/commons/2/2c/Rotating_earth_%28large%29.gif")
                            .w(px(120.0))
                            .h(px(120.0))
                            .with_loading(|| {
                                div()
                                    .text_color(rgb(0x9aa0a6))
                                    .child("loading…")
                                    .into_any_element()
                            })
                            .with_fallback(|| {
                                div()
                                    .text_color(rgb(0xff8080))
                                    .child("failed to load gif")
                                    .into_any_element()
                            }),
                    ),
            )
            .child(
                section("Sizing modes (object_fit)").child(
                    row()
                        .flex_wrap()
                        .child(
                            img("https://picsum.photos/id/237/200/100")
                                .w(px(96.0))
                                .h(px(96.0))
                                .object_fit(ObjectFit::Fill),
                        )
                        .child(
                            img("https://picsum.photos/id/237/200/100")
                                .w(px(96.0))
                                .h(px(96.0))
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(
                            img("https://picsum.photos/id/237/200/100")
                                .w(px(96.0))
                                .h(px(96.0))
                                .object_fit(ObjectFit::Cover),
                        ),
                ),
            )
            .child(
                section("Rounded / clipped image").child(
                    img("https://picsum.photos/id/1015/200/200")
                        .w(px(96.0))
                        .h(px(96.0))
                        .rounded_full()
                        .object_fit(ObjectFit::Cover),
                ),
            )
    }
}
