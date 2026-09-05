//! "Components": a gallery of `gpui_mobile::components::material` and
//! `components::glass` widgets, each in a labelled section. Where a
//! component supports interaction, a live instance bound to this screen's
//! state sits above a static full-catalog reference render.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, ClickEvent, Context, MouseButton, Window, div, prelude::*, px, rgb};
use gpui_mobile::components::{glass, material};
use material::button::{ElevatedButton, FilledButton, OutlinedButton};
use material::card::Card;
use material::controls::{Checkbox, Radio, Switch};
use material::dialog::BasicDialog;
use material::fab::FloatingActionButton;
use material::list_tile::{ListTile, chip};
use material::menu::{Menu, MenuAnchor};
use material::navigation_bar::NavigationBarBuilder;
use material::progress_indicator::{CircularProgressIndicator, LinearProgressIndicator};
use material::search_bar::SearchBar;
use material::tab_bar::TabBar;
use material::text_input::TextInput;
use material::theme::MaterialTheme;
use std::time::Duration;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "components",
        title: "Components",
        category: "Meta",
        blurb: "material + glass component gallery",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| {
        let this = ComponentsScreen::default();
        this.start_progress_animation(cx);
        this
    })
    .into()
}

#[derive(Default)]
struct ComponentsScreen {
    dark: bool,
    button_taps: u32,
    card_taps: u32,
    checkbox_checked: bool,
    switch_on: bool,
    radio_selected: usize,
    dialog_open: bool,
    dialog_result: String,
    fab_taps: u32,
    list_tile_taps: u32,
    menu_open: bool,
    menu_result: String,
    progress: f32,
    search_query: String,
    snackbar_message: Option<String>,
    tab_index: usize,
    nav_index: usize,
    chip_selected: [bool; 3],
    text_value: String,
    text_focused: bool,
    glass_toggle: bool,
}

impl ComponentsScreen {
    fn start_progress_animation(&self, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = weak.update(cx, |this: &mut ComponentsScreen, cx| {
                    this.progress = (this.progress + 0.03) % 1.0;
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}

impl Render for ComponentsScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = MaterialTheme::from_appearance(self.dark);
        let weak = cx.entity().downgrade();

        div()
            .id("components-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Components"),
            )
            .child(note(
                "Live, interactive instances of the components each section covers, with a \
                 static catalog reference underneath where one exists in gpui_mobile.",
            ))
            .child(button(
                "Toggle light/dark theme",
                cx.listener(|this, _, _window, cx| {
                    this.dark = !this.dark;
                    cx.notify();
                }),
            ))
            // ── buttons ──────────────────────────────────────────────────
            .child(
                section("material::button").child(
                    row()
                        .child(
                            FilledButton::new("Filled", theme)
                                .id("mtl-filled")
                                .on_click({
                                    let weak = weak.clone();
                                    move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.button_taps += 1;
                                            gallery_log::push("components: filled button tapped");
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(
                            OutlinedButton::new("Outlined", theme)
                                .id("mtl-outlined")
                                .on_click({
                                    let weak = weak.clone();
                                    move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.button_taps += 1;
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(
                            ElevatedButton::new("Elevated", theme)
                                .id("mtl-elevated")
                                .on_click({
                                    let weak = weak.clone();
                                    move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.button_taps += 1;
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(kv("taps", self.button_taps.to_string())),
                ),
            )
            // ── cards ────────────────────────────────────────────────────
            .child(
                section("material::card").child(
                    Card::new(theme, material::card::CardVariant::Elevated)
                        .id("mtl-card")
                        .on_click({
                            let weak = weak.clone();
                            move |_e, _window, app| {
                                weak.update(app, |this, cx| {
                                    this.card_taps += 1;
                                    cx.notify();
                                })
                                .ok();
                            }
                        })
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(super::common::label("Tap this card"))
                                .child(kv("taps", self.card_taps.to_string())),
                        ),
                ),
            )
            // ── chips ────────────────────────────────────────────────────
            .child(
                section("material::list_tile::chip").child(row().children((0..3).map(|i| {
                    let selected = self.chip_selected[i];
                    let weak = weak.clone();
                    div()
                        .id(("chip", i))
                        .on_mouse_down(MouseButton::Left, move |_e, _window, app| {
                            weak.update(app, |this, cx| {
                                this.chip_selected[i] = !this.chip_selected[i];
                                cx.notify();
                            })
                            .ok();
                        })
                        .child(chip(
                            &format!("Chip {}", i + 1),
                            selected,
                            theme.outline,
                            theme.on_surface,
                            theme.secondary_container,
                        ))
                }))),
            )
            // ── controls ─────────────────────────────────────────────────
            .child(
                section("material::controls").child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            Checkbox::new(theme)
                                .checked(self.checkbox_checked)
                                .label("Notifications")
                                .id("mtl-checkbox")
                                .on_toggle({
                                    let weak = weak.clone();
                                    move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.checkbox_checked = !this.checkbox_checked;
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(
                            Switch::new(theme)
                                .on(self.switch_on)
                                .label("Airplane mode")
                                .id("mtl-switch")
                                .on_toggle({
                                    let weak = weak.clone();
                                    move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.switch_on = !this.switch_on;
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(row().children((0..3).map(|i| {
                            let weak = weak.clone();
                            Radio::new(theme)
                                .selected(self.radio_selected == i)
                                .label(format!("Option {}", i + 1))
                                .id(("mtl-radio", i))
                                .on_select(move |_e, _window, app| {
                                    weak.update(app, |this, cx| {
                                        this.radio_selected = i;
                                        cx.notify();
                                    })
                                    .ok();
                                })
                        }))),
                ),
            )
            // ── dialog ───────────────────────────────────────────────────
            .child(
                section("material::dialog")
                    .child(button(
                        "Show dialog",
                        cx.listener(|this, _, _window, cx| {
                            this.dialog_open = true;
                            cx.notify();
                        }),
                    ))
                    .child(kv("last result", self.dialog_result.clone()))
                    .child(if self.dialog_open {
                        let weak_confirm = weak.clone();
                        let weak_cancel = weak.clone();
                        div().relative().h(px(220.0)).w_full().child(
                            BasicDialog::new(theme)
                                .icon("\u{1F5D1}\u{FE0F}")
                                .title("Delete item?")
                                .content("This cannot be undone.")
                                .dismiss_button("Cancel", move |_e, _window, app| {
                                    weak_cancel
                                        .update(app, |this, cx| {
                                            this.dialog_open = false;
                                            this.dialog_result = "cancelled".into();
                                            cx.notify();
                                        })
                                        .ok();
                                })
                                .confirm_button("Delete", move |_e, _window, app| {
                                    weak_confirm
                                        .update(app, |this, cx| {
                                            this.dialog_open = false;
                                            this.dialog_result = "confirmed".into();
                                            cx.notify();
                                        })
                                        .ok();
                                }),
                        )
                    } else {
                        div()
                    }),
            )
            // ── fab ──────────────────────────────────────────────────────
            .child(
                section("material::fab").child(
                    row()
                        .child(
                            FloatingActionButton::new("+", theme)
                                .id("mtl-fab")
                                .on_click({
                                    let weak = weak.clone();
                                    move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.fab_taps += 1;
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(kv("taps", self.fab_taps.to_string())),
                ),
            )
            // ── list_tile ────────────────────────────────────────────────
            .child(
                section("material::list_tile").child(
                    ListTile::new(theme)
                        .leading_icon("\u{1F4E7}")
                        .title("Inbox")
                        .subtitle(format!("{} taps", self.list_tile_taps))
                        .id("mtl-list-tile")
                        .on_click({
                            let weak = weak.clone();
                            move |_e, _window, app| {
                                weak.update(app, |this, cx| {
                                    this.list_tile_taps += 1;
                                    cx.notify();
                                })
                                .ok();
                            }
                        }),
                ),
            )
            // ── menu ─────────────────────────────────────────────────────
            .child(
                section("material::menu")
                    .child(kv("last selection", self.menu_result.clone()))
                    .child({
                        let weak_open = weak.clone();
                        let weak_item1 = weak.clone();
                        let weak_item2 = weak.clone();
                        MenuAnchor::new(theme)
                            .id("mtl-menu-anchor")
                            .anchor(button(
                                "Open menu",
                                move |_e: &ClickEvent, _window: &mut Window, app: &mut App| {
                                    weak_open
                                        .update(app, |this, cx| {
                                            this.menu_open = !this.menu_open;
                                            cx.notify();
                                        })
                                        .ok();
                                },
                            ))
                            .menu(
                                Menu::new(theme)
                                    .item("Share", "\u{1F4E4}", move |_e, _window, app| {
                                        weak_item1
                                            .update(app, |this, cx| {
                                                this.menu_result = "Share".into();
                                                this.menu_open = false;
                                                cx.notify();
                                            })
                                            .ok();
                                    })
                                    .item("Delete", "\u{1F5D1}", move |_e, _window, app| {
                                        weak_item2
                                            .update(app, |this, cx| {
                                                this.menu_result = "Delete".into();
                                                this.menu_open = false;
                                                cx.notify();
                                            })
                                            .ok();
                                    }),
                            )
                            .open(self.menu_open)
                    }),
            )
            // ── navigation_bar ───────────────────────────────────────────
            .child(
                section("material::navigation_bar")
                    .child(kv("selected", self.nav_index.to_string()))
                    .child({
                        let mut builder = NavigationBarBuilder::new(self.dark);
                        for (i, (icon, label)) in [
                            ("\u{1F3E0}", "Home"),
                            ("\u{1F50D}", "Search"),
                            ("\u{2699}\u{FE0F}", "Settings"),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let weak = weak.clone();
                            builder = builder.item(
                                icon,
                                label,
                                self.nav_index == i,
                                move |_e, _window, app| {
                                    weak.update(app, |this, cx| {
                                        this.nav_index = i;
                                        cx.notify();
                                    })
                                    .ok();
                                },
                            );
                        }
                        builder.build()
                    }),
            )
            // ── progress_indicator ───────────────────────────────────────
            .child(
                section("material::progress_indicator")
                    .child(note("Animated on a 100ms timer to prove liveness."))
                    .child(
                        LinearProgressIndicator::new(theme)
                            .progress(self.progress)
                            .id("mtl-linear-progress"),
                    )
                    .child(
                        CircularProgressIndicator::new(theme)
                            .progress(self.progress)
                            .id("mtl-circular-progress"),
                    ),
            )
            // ── search_bar ───────────────────────────────────────────────
            .child(
                section("material::search_bar")
                    .child(
                        SearchBar::new(theme)
                            .query(self.search_query.clone())
                            .placeholder("Search gallery…")
                            .id("mtl-search-bar"),
                    )
                    .child(
                        row()
                            .child(button(
                                "Type \"gpui\"",
                                cx.listener(|this, _, _window, cx| {
                                    this.search_query = "gpui".into();
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Clear",
                                cx.listener(|this, _, _window, cx| {
                                    this.search_query.clear();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            // ── snackbar ─────────────────────────────────────────────────
            .child(
                section("material::snackbar")
                    .child(button(
                        "Show snackbar",
                        cx.listener(|this, _, _window, cx| {
                            this.snackbar_message = Some("Item archived".into());
                            gallery_log::push("components: snackbar shown");
                            cx.notify();
                            let weak = cx.entity().downgrade();
                            cx.spawn(async move |_this, cx| {
                                cx.background_executor().timer(Duration::from_secs(3)).await;
                                weak.update(cx, |this: &mut ComponentsScreen, cx| {
                                    this.snackbar_message = None;
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }),
                    ))
                    .child(if let Some(msg) = &self.snackbar_message {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_3()
                            .rounded_md()
                            .bg(rgb(0x332d41))
                            .text_color(rgb(0xe6e1e5))
                            .child(msg.clone())
                            .child(div().text_color(rgb(0xd0bcff)).child("UNDO"))
                    } else {
                        div()
                    }),
            )
            // ── tab_bar ──────────────────────────────────────────────────
            .child(section("material::tab_bar").child({
                let mut bar = TabBar::primary(theme);
                for (i, label) in ["One", "Two", "Three"].into_iter().enumerate() {
                    let weak = weak.clone();
                    bar = bar.text_tab(label, self.tab_index == i, move |_e, _window, app| {
                        weak.update(app, |this, cx| {
                            this.tab_index = i;
                            cx.notify();
                        })
                        .ok();
                    });
                }
                bar
            }))
            // ── text_field / text_input ──────────────────────────────────
            .child(
                section("material::text_field / text_input")
                    .child(note(
                        "material::text_input drives the software keyboard through \
                         gpui_mobile::set_text_input_callback, a deprecated global stopgap (see \
                         crates/gpui_mobile/src/lib.rs) predating EntityInputHandler — kept only \
                         for callers like this that haven't migrated to native UITextInput yet.",
                    ))
                    .child(self.render_text_input(theme, cx)),
            )
            // ── static catalog reference ─────────────────────────────────
            .child(section("full static catalog (material)").children([
                catalog_row("buttons", material::button::button_demo(self.dark)),
                catalog_row("cards", material::card::card_demo(self.dark)),
                catalog_row("chips", material::list_tile::chips(self.dark)),
                catalog_row("controls", material::controls::controls_demo(self.dark)),
                catalog_row("dialog", material::dialog::dialog_demo(self.dark)),
                catalog_row("fab", material::fab::fab_demo(self.dark)),
                catalog_row("list_tile", material::list_tile::list_tile_demo(self.dark)),
                catalog_row("menu", material::menu::menu_demo(self.dark)),
                catalog_row(
                    "navigation_bar",
                    material::navigation_bar::navigation_bar_demo(self.dark),
                ),
                catalog_row(
                    "progress_indicator",
                    material::progress_indicator::progress_indicator_demo(self.dark),
                ),
                catalog_row(
                    "search_bar",
                    material::search_bar::search_bar_demo(self.dark),
                ),
                catalog_row("snackbar", material::snackbar(self.dark)),
                catalog_row("tab_bar", material::tab_bar::tab_bar_demo(self.dark)),
                catalog_row("text_fields", material::text_fields(self.dark)),
            ]))
            // ── glass ────────────────────────────────────────────────────
            .child(
                section("glass").children([
                    catalog_row(
                        "toggle (live)",
                        row()
                            .child({
                                let weak = weak.clone();
                                div()
                                    .id("glass-toggle")
                                    .on_mouse_down(MouseButton::Left, move |_e, _window, app| {
                                        weak.update(app, |this, cx| {
                                            this.glass_toggle = !this.glass_toggle;
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                                    .child(glass::settings_list::toggle(
                                        self.glass_toggle,
                                        self.dark,
                                    ))
                            })
                            .child(kv("on", self.glass_toggle.to_string())),
                    ),
                    catalog_row("buttons_row", glass::buttons_row(self.dark)),
                    catalog_row("hero_card", glass::hero_card(self.dark)),
                    catalog_row(
                        "notification_banners",
                        glass::notification_banners(self.dark),
                    ),
                    catalog_row("segmented_control", glass::segmented_control(self.dark)),
                    catalog_row("search_bar", glass::search_bar(self.dark)),
                    catalog_row("settings_list", glass::settings_list(self.dark)),
                    catalog_row("sliders", glass::sliders(self.dark)),
                    catalog_row("tab_bar", glass::tab_bar(self.dark)),
                ]),
            )
    }
}

fn catalog_row(label: &str, element: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_color(rgb(0x9aa0a6))
                .text_size(px(11.0))
                .child(label.to_string()),
        )
        .child(element)
}

/// Text typed through the deprecated global text-input callback, drained on
/// the next render. The callback runs from the platform layer outside of
/// GPUI's update cycle (no `App`/`Window` access), so — mirroring the
/// `webview` screen's `LAST_URL_CHANGE` pattern — it just stashes the text
/// here; `TEXT_INPUT_DIRTY` (set by `dispatch_text_input`) forces the next
/// render, which drains it.
static PENDING_TEXT_INPUT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

impl ComponentsScreen {
    #[allow(deprecated)]
    fn render_text_input(
        &mut self,
        theme: MaterialTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if let Some(text) = PENDING_TEXT_INPUT.lock().unwrap().take() {
            self.text_value.push_str(&text);
        }
        let value = self.text_value.clone();
        let focused = self.text_focused;
        TextInput::<Self>::new("mtl-text-input", theme)
            .label("Comment")
            .value(&value)
            .placeholder("Type something…")
            .focused(focused)
            .cursor(value.len())
            .on_tap(|this, _event, _window, cx| {
                this.text_focused = true;
                gpui_mobile::set_text_input_callback(Some(Box::new(|text: &str| {
                    *PENDING_TEXT_INPUT.lock().unwrap() = Some(text.to_string());
                })));
                gpui_mobile::show_keyboard();
                cx.notify();
            })
            .render(cx)
    }
}
