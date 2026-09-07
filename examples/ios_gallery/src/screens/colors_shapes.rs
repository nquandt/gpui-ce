//! "Colors & shapes" screen: solid colors, gradients, shadows, borders,
//! per-corner radii, opacity, overlapping transparency, a hand-drawn canvas
//! path, and a blurred element.

use super::ScreenDescriptor;
use super::common::{note, row, section};
use gpui::{
    AnyView, App, BoxShadow, ColorExt, Context, PathBuilder, Window, canvas, div,
    linear_color_stop, linear_gradient, point, prelude::*, px, rgb,
};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "colors_shapes",
        title: "Colors & shapes",
        category: "Core UI",
        blurb: "gradients, shadows, borders, radius, opacity",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| ColorsShapesScreen).into()
}

struct ColorsShapesScreen;

fn swatch() -> gpui::Div {
    div().w(px(64.0)).h(px(64.0)).rounded_md()
}

impl Render for ColorsShapesScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("colors-shapes-scroll")
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Colors & shapes"),
            )
            .child(
                section("Solid colors").child(
                    row()
                        .flex_wrap()
                        .child(swatch().bg(rgb(0xff4d4d)))
                        .child(swatch().bg(rgb(0x4dff88)))
                        .child(swatch().bg(rgb(0x4d88ff)))
                        .child(swatch().bg(rgb(0xffe14d))),
                ),
            )
            .child(
                section("Linear gradients (no radial gradient API in gpui today)")
                    .child(note(
                        "Only `linear_gradient` exists (crates/gpui/src/color.rs) — \
                         there is no `radial_gradient` yet.",
                    ))
                    .child(
                        row()
                            .flex_wrap()
                            .child(swatch().bg(linear_gradient(
                                0.0,
                                linear_color_stop(rgb(0xff4d4d), 0.0),
                                linear_color_stop(rgb(0x4d88ff), 1.0),
                            )))
                            .child(swatch().bg(linear_gradient(
                                90.0,
                                linear_color_stop(rgb(0xffe14d), 0.0),
                                linear_color_stop(rgb(0xff4d4d), 1.0),
                            )))
                            .child(swatch().bg(linear_gradient(
                                45.0,
                                linear_color_stop(rgb(0x4dff88), 0.0),
                                linear_color_stop(rgb(0x4d88ff), 1.0),
                            ))),
                    ),
            )
            .child(
                section("Box shadows").child(
                    row()
                        .flex_wrap()
                        .child(swatch().bg(rgb(0x2a2a2a)).shadow(vec![
                                BoxShadow::new(px(0.0), px(1.0), gpui::black().opacity(0.4))
                                    .blur_radius(px(2.0)),
                            ]))
                        .child(swatch().bg(rgb(0x2a2a2a)).shadow(vec![
                                BoxShadow::new(px(0.0), px(4.0), gpui::black().opacity(0.4))
                                    .blur_radius(px(8.0)),
                            ]))
                        .child(swatch().bg(rgb(0x2a2a2a)).shadow(vec![
                                BoxShadow::new(px(0.0), px(10.0), gpui::black().opacity(0.5))
                                    .blur_radius(px(20.0)),
                            ])),
                ),
            )
            .child(
                section("Borders").child(
                    row()
                        .flex_wrap()
                        .child(swatch().border_1().border_color(rgb(0xffffff)))
                        .child(swatch().border_2().border_color(rgb(0x7fb0ff)))
                        .child(swatch().border_4().border_color(rgb(0xff9a3c)))
                        .child(
                            swatch()
                                .border_2()
                                .border_dashed()
                                .border_color(rgb(0x4dff88)),
                        ),
                ),
            )
            .child(
                section("Per-corner radii").child(
                    row().flex_wrap().child(
                        div()
                            .w(px(96.0))
                            .h(px(64.0))
                            .bg(rgb(0x4d88ff))
                            .rounded_tl(px(24.0))
                            .rounded_tr(px(4.0))
                            .rounded_bl(px(4.0))
                            .rounded_br(px(24.0)),
                    ),
                ),
            )
            .child(
                section("Opacity + overlapping transparency")
                    .child(note(
                        "The three circles should visibly blend where they overlap.",
                    ))
                    .child(
                        div()
                            .relative()
                            .h(px(96.0))
                            .w_full()
                            .child(
                                div()
                                    .absolute()
                                    .left(px(0.0))
                                    .top(px(0.0))
                                    .w(px(80.0))
                                    .h(px(80.0))
                                    .rounded_full()
                                    .bg(rgb(0xff4d4d))
                                    .opacity(0.6),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(40.0))
                                    .top(px(0.0))
                                    .w(px(80.0))
                                    .h(px(80.0))
                                    .rounded_full()
                                    .bg(rgb(0x4dff88))
                                    .opacity(0.6),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(80.0))
                                    .top(px(0.0))
                                    .w(px(80.0))
                                    .h(px(80.0))
                                    .rounded_full()
                                    .bg(rgb(0x4d88ff))
                                    .opacity(0.6),
                            ),
                    ),
            )
            .child(
                section("Canvas paths (bezier shape + stroked polyline)").child(
                    div().w_full().h(px(160.0)).child(canvas(
                        |_bounds, _window, _cx| {},
                        |bounds, _state, window, _cx| {
                            let origin = bounds.origin;
                            let mut fill = PathBuilder::fill();
                            fill.move_to(origin + point(px(20.0), px(80.0)));
                            fill.curve_to(
                                origin + point(px(90.0), px(80.0)),
                                origin + point(px(55.0), px(10.0)),
                            );
                            fill.curve_to(
                                origin + point(px(160.0), px(80.0)),
                                origin + point(px(125.0), px(10.0)),
                            );
                            fill.line_to(origin + point(px(160.0), px(120.0)));
                            fill.line_to(origin + point(px(20.0), px(120.0)));
                            fill.close();
                            if let Ok(path) = fill.build() {
                                window.paint_path(path, rgb(0x7fb0ff));
                            }

                            let mut stroke = PathBuilder::stroke(px(3.0));
                            stroke.move_to(origin + point(px(190.0), px(120.0)));
                            stroke.line_to(origin + point(px(220.0), px(20.0)));
                            stroke.line_to(origin + point(px(250.0), px(100.0)));
                            stroke.line_to(origin + point(px(280.0), px(40.0)));
                            if let Ok(path) = stroke.build() {
                                window.paint_path(path, rgb(0xff9a3c));
                            }
                        },
                    )),
                ),
            )
            .child(
                section("Blurred element").child(
                    div()
                        .relative()
                        .h(px(96.0))
                        .w_full()
                        .child(
                            div()
                                .absolute()
                                .left(px(0.0))
                                .top(px(0.0))
                                .w(px(120.0))
                                .h(px(96.0))
                                .bg(rgb(0xff4d4d)),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(px(60.0))
                                .top(px(16.0))
                                .w(px(140.0))
                                .h(px(64.0))
                                .bg(gpui::white().opacity(0.15))
                                .backdrop_blur(px(8.0))
                                .rounded_md(),
                        ),
                ),
            )
    }
}
