//! "Typography" screen: sizes, weights, italics, monospace, emoji, CJK,
//! RTL (Arabic/Hebrew), Devanagari, wrapping, ellipsis, line height, and
//! text decoration.

use super::ScreenDescriptor;
use super::common::{note, section};
use gpui::{AnyView, App, Context, FontWeight, Window, div, prelude::*, px, rgb};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "typography",
        title: "Typography",
        category: "Core UI",
        blurb: "sizes, weights, emoji, CJK, RTL, wrapping",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| TypographyScreen).into()
}

struct TypographyScreen;

impl Render for TypographyScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("typography-scroll")
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
                    .child("Typography"),
            )
            .child(note(
                "No fonts are bundled with this example (examples/ios_gallery has no \
                 assets/fonts directory and Info.plist has no UIAppFonts entry) — \
                 everything below renders with the system font (San Francisco) and \
                 whatever fallback fonts iOS ships for CJK/RTL/Devanagari scripts.",
            ))
            .child(
                section("Sizes (10..48)").children((0..=8).map(|i| {
                    let size = 10.0 + i as f32 * 4.75;
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(size))
                        .child(format!("{size:.0}px The quick brown fox"))
                })),
            )
            .child(
                section("Weights (thin -> black)").children(FontWeight::ALL.iter().map(|w| {
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(16.0))
                        .font_weight(*w)
                        .child(format!("{:.0} weight — Sphinx of black quartz", w.0))
                })),
            )
            .child(
                section("Italic / monospace")
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .italic()
                            .child("Italic: the five boxing wizards jump quickly."),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .font_family("Menlo")
                            .child("Monospace (Menlo): fn main() { let x = 1; }"),
                    ),
            )
            .child(
                section("Emoji").child(
                    div()
                        .text_size(px(24.0))
                        .child(
                            "\u{1F600} \u{1F44D} \u{1F3F3}\u{FE0F}\u{200D}\u{1F308} \u{1F1FA}\u{1F1F8} \u{1F468}\u{1F3FD} \u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
                        ),
                ),
            )
            .child(
                section("CJK")
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(18.0))
                            .child("日本語: こんにちは世界"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(18.0))
                            .child("中文（简体）：你好，世界"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(18.0))
                            .child("한국어: 안녕하세요 세계"),
                    ),
            )
            .child(
                section("RTL (Arabic / Hebrew)")
                    .child(note("Both lines should visually flow right-to-left."))
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(18.0))
                            .child("العربية: مرحبا بالعالم"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(18.0))
                            .child("עברית: שלום עולם"),
                    ),
            )
            .child(
                section("Devanagari").child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(18.0))
                        .child("देवनागरी: नमस्ते दुनिया"),
                ),
            )
            .child(
                section("Wrapping paragraph").child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(14.0))
                        .w(px(280.0))
                        .child(
                            "This is a long paragraph meant to exercise line wrapping \
                             inside a narrow, fixed-width container. It should break \
                             across multiple lines cleanly at word boundaries without \
                             overflowing its 280px box, and every line should remain \
                             fully readable.",
                        ),
                ),
            )
            .child(
                section("Ellipsis truncation").child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(14.0))
                        .w(px(160.0))
                        .text_ellipsis()
                        .child(
                            "This single line of text is much too long to fit and should truncate with an ellipsis.",
                        ),
                ),
            )
            .child(
                section("Line height variants")
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(14.0))
                            .line_height(px(14.0))
                            .w(px(240.0))
                            .child("Tight line height (1.0x): line one line two line three."),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(14.0))
                            .line_height(px(28.0))
                            .w(px(240.0))
                            .child("Loose line height (2.0x): line one line two line three."),
                    ),
            )
            .child(
                section("Decoration")
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .text_decoration_solid()
                            .text_decoration_1()
                            .child("Underlined text"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xffffff))
                            .text_size(px(16.0))
                            .text_decoration_wavy()
                            .text_decoration_2()
                            .child("Wavy-underlined text"),
                    ),
            )
    }
}
