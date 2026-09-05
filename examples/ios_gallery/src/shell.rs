//! The gallery shell: navigation stack, header, bottom verdict/notes bar,
//! and the home screen list.

use crate::log as gallery_log;
use crate::screens::{self, ScreenDescriptor};
use crate::store::{self, Verdict};
use gpui::{AnyView, App, Context, Entity, Window, div, prelude::*, px, rgb};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, text_area};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A URL scheme deep link or `--screen` launch argument, delivered to the
/// shell off the GPUI main thread; polled once per frame.
static PENDING_NAVIGATION: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn pending_navigation() -> &'static Mutex<Option<String>> {
    PENDING_NAVIGATION.get_or_init(|| Mutex::new(None))
}

/// Called from the deep link handler (off the GPUI thread) or from the
/// `--screen` launch-argument scan (on the main thread before the app
/// callback runs).
pub fn request_navigation(screen_id: impl Into<String>) {
    *pending_navigation().lock().unwrap() = Some(screen_id.into());
}

fn take_pending_navigation() -> Option<String> {
    pending_navigation().lock().unwrap().take()
}

pub struct Gallery {
    stack: Vec<&'static str>,
    views: HashMap<&'static str, AnyView>,
    descriptors: Vec<ScreenDescriptor>,
    verdicts: HashMap<&'static str, Verdict>,
    taps: u64,
    notes_open: bool,
    notes_state: Option<Entity<EditableTextState>>,
    notes_screen: Option<&'static str>,
}

impl Gallery {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let descriptors = screens::all();
        let ids: Vec<&'static str> = descriptors.iter().map(|d| d.id).collect();
        let verdicts = store::load_all(&ids)
            .into_iter()
            .map(|(id, (verdict, _notes))| (id, verdict))
            .collect();

        gallery_log::push("gallery: launched");

        Self {
            stack: Vec::new(),
            views: HashMap::new(),
            descriptors,
            verdicts,
            taps: 0,
            notes_open: false,
            notes_state: None,
            notes_screen: None,
        }
    }

    fn current_id(&self) -> Option<&'static str> {
        self.stack.last().copied()
    }

    fn current_descriptor(&self) -> Option<ScreenDescriptor> {
        let id = self.current_id()?;
        self.descriptors.iter().find(|d| d.id == id).copied()
    }

    fn ensure_view(&mut self, id: &'static str, window: &mut Window, cx: &mut App) {
        if self.views.contains_key(id) {
            return;
        }
        if let Some(descriptor) = self.descriptors.iter().find(|d| d.id == id).copied() {
            let view = (descriptor.build)(window, cx);
            self.views.insert(id, view);
        }
    }

    fn push_screen(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if self.descriptors.iter().all(|d| d.id != id) {
            gallery_log::push(format!("navigation: unknown screen id '{id}'"));
            return;
        }
        self.ensure_view(id, window, cx);
        self.stack.push(id);
        self.notes_open = false;
        self.load_notes_state(window, cx);
        gallery_log::push(format!("navigation: pushed '{id}'"));
        cx.notify();
    }

    fn pop_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.stack.pop() {
            gallery_log::push(format!("navigation: popped '{id}'"));
        }
        self.notes_open = false;
        self.load_notes_state(window, cx);
        cx.notify();
    }

    fn load_notes_state(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.current_id() else {
            self.notes_state = None;
            self.notes_screen = None;
            return;
        };
        if self.notes_screen == Some(id) {
            return;
        }
        let initial = store::load_notes(id);
        let state = cx.new(|cx| EditableTextState::new(StringStorage::from(initial), cx));
        cx.observe(&state, move |this, state, cx| {
            if let Some(id) = this.current_id() {
                let text = state.read(cx).as_str().to_string();
                store::save_notes(id, &text);
            }
        })
        .detach();
        self.notes_state = Some(state);
        self.notes_screen = Some(id);
    }

    fn set_verdict(&mut self, verdict: Verdict, cx: &mut Context<Self>) {
        if let Some(id) = self.current_id() {
            self.verdicts.insert(id, verdict);
            store::save_verdict(id, verdict);
            gallery_log::push(format!("verdict: '{id}' -> {}", verdict.label()));
            cx.notify();
        }
    }

    fn poll_pending_navigation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = take_pending_navigation() {
            let id: &'static str = Box::leak(id.into_boxed_str());
            self.push_screen(id, window, cx);
        }
    }

    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (safe_top, _, _, _) = gpui_mobile::safe_area_insets();
        let title = self
            .current_descriptor()
            .map(|d| d.title)
            .unwrap_or("GPUI Gallery");
        let on_home = self.stack.is_empty();

        div()
            .flex()
            .flex_row()
            .items_center()
            .pt(px(safe_top + 8.0))
            .pb_2()
            .px_3()
            .gap_2()
            .bg(rgb(0x181818))
            .child(
                div()
                    .id("back")
                    .w(px(56.0))
                    .text_color(rgb(0x7fb0ff))
                    .text_size(px(15.0))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pop_screen(window, cx);
                    }))
                    .child(if on_home { "" } else { "‹ Back" }),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .text_size(px(16.0))
                    .child(title),
            )
            .child(
                div()
                    .id("dismiss_keyboard")
                    .when(gpui_mobile::keyboard_height() <= 0.0, |this| {
                        this.invisible()
                    })
                    .px_3()
                    .py_1()
                    .mr_2()
                    .rounded_full()
                    .bg(rgb(0x2c2c34))
                    .text_color(rgb(0x8ab4f8))
                    .text_size(px(12.0))
                    .on_click(cx.listener(|_this, _, window, cx| {
                        gallery_log::push("keyboard: dismissed from header");
                        window.blur(cx);
                    }))
                    .child("keyboard ▾"),
            )
            .child(
                div()
                    .id("tap_counter")
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(rgb(0x2c2c34))
                    .text_color(rgb(0xd0d0d0))
                    .text_size(px(12.0))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.taps += 1;
                        gallery_log::push(format!("tap counter: {}", this.taps));
                        cx.notify();
                    }))
                    .child(format!("taps: {}", self.taps)),
            )
    }

    fn render_home(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut list = div()
            .id("home_list")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .restrict_scroll_to_axis();
        let mut last_category: Option<&'static str> = None;

        for descriptor in self.descriptors.clone() {
            if last_category != Some(descriptor.category) {
                last_category = Some(descriptor.category);
                list = list.child(
                    div()
                        .px_4()
                        .pt_4()
                        .pb_1()
                        .text_color(rgb(0x7a7f87))
                        .text_size(px(12.0))
                        .child(descriptor.category),
                );
            }
            let verdict = self
                .verdicts
                .get(descriptor.id)
                .copied()
                .unwrap_or_default();
            let id = descriptor.id;
            list = list.child(
                div()
                    .id(gpui::ElementId::from(id))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0x2a2a2a))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.push_screen(id, window, cx);
                    }))
                    .child(
                        div()
                            .w(px(20.0))
                            .text_color(rgb(0xd0d0d0))
                            .text_size(px(16.0))
                            .child(verdict.glyph()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(15.0))
                                    .child(descriptor.title),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x8a8f96))
                                    .text_size(px(12.0))
                                    .child(descriptor.blurb),
                            ),
                    ),
            );
        }
        let _ = window;
        list
    }

    fn render_bottom_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (_, safe_bottom, _, _) = gpui_mobile::safe_area_insets();
        let keyboard_height = gpui_mobile::keyboard_height();
        let bottom_gap = if keyboard_height > 0.0 {
            keyboard_height
        } else {
            safe_bottom
        };
        let current = self.current_id().unwrap_or_default();
        let verdict = self.verdicts.get(current).copied().unwrap_or_default();

        let segment = |label: &'static str, target: Verdict, current: Verdict| {
            let active = target == current;
            div()
                .id(gpui::ElementId::from(label))
                .flex_1()
                .flex()
                .justify_center()
                .items_center()
                .py_2()
                .rounded_md()
                .bg(if active { rgb(0x33415c) } else { rgb(0x232323) })
                .text_color(if active { rgb(0xffffff) } else { rgb(0x9aa0a6) })
                .text_size(px(13.0))
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.set_verdict(target, cx);
                }))
                .child(label)
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .pb(px(8.0 + bottom_gap))
            .bg(rgb(0x181818))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(segment("Works", Verdict::Works, verdict))
                    .child(segment("Partial", Verdict::Partial, verdict))
                    .child(segment("Broken", Verdict::Broken, verdict)),
            )
            .child(
                div()
                    .id("notes_toggle")
                    .text_color(rgb(0x7fb0ff))
                    .text_size(px(13.0))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.notes_open = !this.notes_open;
                        cx.notify();
                    }))
                    .child(if self.notes_open {
                        "Notes ▲"
                    } else {
                        "Notes ▼"
                    }),
            )
            .child(if self.notes_open {
                if let Some(state) = self.notes_state.clone() {
                    div().child(
                        text_area(gpui::ElementId::from("gallery_notes"))
                            .state(state.downgrade())
                            .border_1()
                            .rounded_md()
                            .border_color(rgb(0x444444))
                            .bg(rgb(0x232323))
                            .text_color(rgb(0xffffff))
                            .p_2()
                            .w_full()
                            .h(px(80.0))
                            .placeholder("Notes for this screen…"),
                    )
                } else {
                    div()
                }
            } else {
                div()
            })
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.poll_pending_navigation(window, cx);

        let on_home = self.stack.is_empty();
        let content: Option<AnyView> = if on_home {
            None
        } else {
            self.current_id().and_then(|id| self.views.get(id).cloned())
        };
        let header = self.render_header(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .child(header)
            .child(if on_home {
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_home(window, cx))
            } else {
                div().flex_1().overflow_hidden().children(content)
            })
            .children(if on_home {
                None
            } else {
                Some(self.render_bottom_bar(cx))
            })
    }
}
