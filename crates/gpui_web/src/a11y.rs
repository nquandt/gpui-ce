//! Accessibility bridge for the browser.
//!
//! GPUI draws into a canvas, which is invisible to screen readers. This
//! module mirrors the AccessKit tree that GPUI builds every frame into a
//! hidden DOM subtree: one absolutely positioned element per node, carrying
//! an ARIA role, a name, a description, state attributes and the node's
//! bounds in CSS pixels. Assistive technology reads that subtree the same
//! way it reads any web page.
//!
//! Interaction flows back the other way. Elements for interactive nodes are
//! focusable; a click, `Enter`/`Space`, or a focus change on them is sent to
//! GPUI as an AccessKit `ActionRequest`, which GPUI dispatches to the
//! element's `on_a11y_action` listeners or synthesizes a click at the node's
//! centre.
//!
//! The mirror sits above the canvas with `pointer-events: none`, so mouse
//! and touch input still reach the canvas; only keyboard focus and
//! assistive-technology actions go through the mirror.

use accesskit::{Action, ActionRequest, NodeId, Role, TreeId, TreeUpdate};
use gpui::A11yCallbacks;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;

const ROOT: NodeId = NodeId(0);

struct MirrorNode {
    element: web_sys::HtmlElement,
    _listeners: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}

struct Shared {
    callbacks: RefCell<Option<Rc<A11yCallbacks>>>,
    /// Set while the mirror itself moves DOM focus, so the resulting focus
    /// events are not echoed back to GPUI.
    syncing_focus: Cell<bool>,
}

impl Shared {
    fn send(&self, action: Action, target: NodeId) {
        if let Some(callbacks) = self.callbacks.borrow().as_ref() {
            (callbacks.action)(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: target,
                data: None,
            });
        }
    }
}

/// Mirrors the GPUI accessibility tree into hidden DOM nodes.
pub(crate) struct A11yMirror {
    document: web_sys::Document,
    container: web_sys::HtmlElement,
    nodes: RefCell<HashMap<NodeId, MirrorNode>>,
    focused: Cell<Option<NodeId>>,
    shared: Rc<Shared>,
}

impl A11yMirror {
    /// Create the hidden container after `canvas` in the document.
    pub(crate) fn new(
        document: &web_sys::Document,
        canvas: &web_sys::HtmlCanvasElement,
    ) -> Option<Self> {
        let container: web_sys::HtmlElement =
            document.create_element("div").ok()?.dyn_into().ok()?;
        container.set_id("gpui-a11y");
        let _ = container.set_attribute("aria-live", "off");
        let style = container.style();
        for (name, value) in [
            ("position", "fixed"),
            ("left", "0"),
            ("top", "0"),
            ("width", "100%"),
            ("height", "100%"),
            ("overflow", "hidden"),
            ("pointer-events", "none"),
            ("z-index", "1"),
            ("color", "transparent"),
        ] {
            let _ = style.set_property(name, value);
        }
        let parent = canvas.parent_node()?;
        parent
            .insert_before(&container, canvas.next_sibling().as_ref())
            .ok()?;
        Some(Self {
            document: document.clone(),
            container,
            nodes: RefCell::new(HashMap::new()),
            focused: Cell::new(None),
            shared: Rc::new(Shared {
                callbacks: RefCell::new(None),
                syncing_focus: Cell::new(false),
            }),
        })
    }

    /// Store the callbacks and activate accessibility right away.
    ///
    /// Browsers give no signal for "a screen reader is connected", and the
    /// mirror is cheap, so the tree is always built.
    pub(crate) fn init(&self, callbacks: A11yCallbacks) {
        let callbacks = Rc::new(callbacks);
        *self.shared.callbacks.borrow_mut() = Some(callbacks.clone());
        if let Some(initial) = (callbacks.activation)() {
            self.apply(initial, 1.0);
        }
    }

    /// Apply one frame's tree. GPUI sends the whole tree each frame, so any
    /// node missing from `update` is removed. `scale` converts the physical
    /// pixel bounds in the tree to CSS pixels.
    pub(crate) fn apply(&self, update: TreeUpdate, scale: f64) {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let present: HashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();

        // Drop nodes that no longer exist.
        {
            let mut nodes = self.nodes.borrow_mut();
            let gone: Vec<NodeId> = nodes
                .keys()
                .filter(|id| !present.contains(id))
                .copied()
                .collect();
            for id in gone {
                if let Some(node) = nodes.remove(&id) {
                    node.element.remove();
                }
            }
        }

        // Mirror elements nest like the tree, and CSS positions a child
        // relative to its absolutely positioned parent, so each node's bounds
        // are expressed relative to its parent's origin.
        let mut parent_origin: HashMap<NodeId, (f64, f64)> = HashMap::new();
        for (_, node) in &update.nodes {
            let origin = node
                .bounds()
                .map(|bounds| (bounds.x0, bounds.y0))
                .unwrap_or((0.0, 0.0));
            for child in node.children() {
                parent_origin.insert(*child, origin);
            }
        }

        // Create or update every node's element.
        for (id, node) in &update.nodes {
            self.ensure_element(*id, node.role());
            let nodes = self.nodes.borrow();
            let Some(mirror) = nodes.get(id) else {
                continue;
            };
            let origin = parent_origin.get(id).copied().unwrap_or((0.0, 0.0));
            sync_attributes(&mirror.element, node, scale, origin);
        }

        // Rebuild the hierarchy in tree order. `append_child` moves an
        // element that is already attached, so this also fixes ordering.
        {
            let nodes = self.nodes.borrow();
            if let Some(root) = nodes.get(&ROOT) {
                let _ = self.container.append_child(&root.element);
            }
            for (id, node) in &update.nodes {
                let Some(parent) = nodes.get(id) else {
                    continue;
                };
                for child_id in node.children() {
                    if let Some(child) = nodes.get(child_id) {
                        let _ = parent.element.append_child(&child.element);
                    }
                }
            }
        }

        // Move DOM focus to follow GPUI focus.
        let focus = update.focus;
        if self.focused.get() != Some(focus) {
            self.focused.set(Some(focus));
            if focus != ROOT {
                let nodes = self.nodes.borrow();
                if let Some(target) = nodes.get(&focus) {
                    self.shared.syncing_focus.set(true);
                    let _ = target.element.focus();
                    self.shared.syncing_focus.set(false);
                }
            }
        }
    }

    fn ensure_element(&self, id: NodeId, role: Role) {
        let mut nodes = self.nodes.borrow_mut();
        if nodes.contains_key(&id) {
            return;
        }
        let tag = if is_text(role) { "span" } else { "div" };
        let Ok(element) = self.document.create_element(tag) else {
            return;
        };
        let Ok(element) = element.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };
        let _ = element.set_attribute("data-node-id", &id.0.to_string());
        let style = element.style();
        let _ = style.set_property("position", "absolute");
        let _ = style.set_property("outline", "none");
        let _ = style.set_property("overflow", "hidden");
        let _ = style.set_property("white-space", "nowrap");

        let mut listeners = Vec::new();
        if is_interactive(role) {
            let _ = element.set_attribute("tabindex", "0");
            let _ = element.style().set_property("pointer-events", "auto");

            let shared = self.shared.clone();
            let on_click =
                Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
                    event.prevent_default();
                    shared.send(Action::Click, id);
                });
            let _ = element
                .add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
            listeners.push(on_click);

            let shared = self.shared.clone();
            let on_key = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
                let Ok(key) = event.dyn_into::<web_sys::KeyboardEvent>() else {
                    return;
                };
                let name = key.key();
                if name == "Enter" || name == " " {
                    key.prevent_default();
                    shared.send(Action::Click, id);
                }
            });
            let _ = element
                .add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref());
            listeners.push(on_key);
        } else {
            let _ = element.set_attribute("tabindex", "-1");
        }

        let shared = self.shared.clone();
        let on_focus = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
            if !shared.syncing_focus.get() {
                shared.send(Action::Focus, id);
            }
        });
        let _ =
            element.add_event_listener_with_callback("focus", on_focus.as_ref().unchecked_ref());
        listeners.push(on_focus);

        nodes.insert(
            id,
            MirrorNode {
                element,
                _listeners: listeners,
            },
        );
    }
}

impl Drop for A11yMirror {
    fn drop(&mut self) {
        self.container.remove();
    }
}

/// Write role, name, state and bounds onto the mirror element.
fn sync_attributes(
    element: &web_sys::HtmlElement,
    node: &accesskit::Node,
    scale: f64,
    parent_origin: (f64, f64),
) {
    let role = node.role();
    match aria_role(role) {
        Some(aria) => {
            let _ = element.set_attribute("role", aria);
        }
        None => {
            let _ = element.remove_attribute("role");
        }
    }

    // Text nodes expose their label as content so screen readers read it as
    // text; everything else uses aria-label.
    if is_text(role) {
        element.set_text_content(node.label());
        let _ = element.remove_attribute("aria-label");
    } else {
        set_or_remove(element, "aria-label", node.label());
    }
    set_or_remove(element, "aria-description", node.description());
    set_or_remove(element, "aria-placeholder", node.placeholder());

    if matches!(
        role,
        Role::TextInput | Role::MultilineTextInput | Role::SearchInput
    ) {
        element.set_text_content(node.value());
    }

    let checked_like = matches!(
        role,
        Role::CheckBox
            | Role::RadioButton
            | Role::Switch
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
    );
    let toggled = node.toggled().map(|state| match state {
        accesskit::Toggled::True => "true",
        accesskit::Toggled::False => "false",
        accesskit::Toggled::Mixed => "mixed",
    });
    if checked_like {
        set_or_remove(element, "aria-checked", toggled);
        let _ = element.remove_attribute("aria-pressed");
    } else {
        set_or_remove(element, "aria-pressed", toggled);
        let _ = element.remove_attribute("aria-checked");
    }
    set_bool(element, "aria-selected", node.is_selected());
    set_bool(element, "aria-expanded", node.is_expanded());
    set_flag(element, "aria-disabled", node.is_disabled());
    set_flag(element, "aria-hidden", node.is_hidden());
    set_flag(element, "aria-readonly", node.is_read_only());
    set_or_remove(
        element,
        "aria-valuenow",
        node.numeric_value()
            .map(|value| value.to_string())
            .as_deref(),
    );
    set_or_remove(
        element,
        "aria-valuemin",
        node.min_numeric_value()
            .map(|value| value.to_string())
            .as_deref(),
    );
    set_or_remove(
        element,
        "aria-valuemax",
        node.max_numeric_value()
            .map(|value| value.to_string())
            .as_deref(),
    );
    set_or_remove(
        element,
        "aria-level",
        node.level().map(|v| v.to_string()).as_deref(),
    );
    set_or_remove(
        element,
        "aria-posinset",
        node.position_in_set().map(|v| v.to_string()).as_deref(),
    );
    set_or_remove(
        element,
        "aria-setsize",
        node.size_of_set().map(|v| v.to_string()).as_deref(),
    );

    let style = element.style();
    if let Some(bounds) = node.bounds() {
        let left = (bounds.x0 - parent_origin.0) / scale;
        let top = (bounds.y0 - parent_origin.1) / scale;
        let _ = style.set_property("left", &format!("{left}px"));
        let _ = style.set_property("top", &format!("{top}px"));
        let _ = style.set_property("width", &format!("{}px", (bounds.x1 - bounds.x0) / scale));
        let _ = style.set_property("height", &format!("{}px", (bounds.y1 - bounds.y0) / scale));
    } else if role == Role::Window {
        let _ = style.set_property("left", "0");
        let _ = style.set_property("top", "0");
        let _ = style.set_property("width", "100%");
        let _ = style.set_property("height", "100%");
    }
}

fn set_or_remove(element: &web_sys::HtmlElement, name: &str, value: Option<&str>) {
    match value {
        Some(value) if !value.is_empty() => {
            let _ = element.set_attribute(name, value);
        }
        _ => {
            let _ = element.remove_attribute(name);
        }
    }
}

fn set_bool(element: &web_sys::HtmlElement, name: &str, value: Option<bool>) {
    set_or_remove(
        element,
        name,
        value.map(|v| if v { "true" } else { "false" }),
    );
}

fn set_flag(element: &web_sys::HtmlElement, name: &str, on: bool) {
    set_or_remove(element, name, on.then_some("true"));
}

fn is_text(role: Role) -> bool {
    matches!(role, Role::Label | Role::TextRun | Role::Paragraph)
}

fn is_interactive(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::DefaultButton
            | Role::CheckBox
            | Role::RadioButton
            | Role::Switch
            | Role::Link
            | Role::Tab
            | Role::MenuItem
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::ListBoxOption
            | Role::TreeItem
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::ComboBox
            | Role::Slider
            | Role::SpinButton
            | Role::Cell
            | Role::Row
            | Role::ListItem
            | Role::DisclosureTriangle
    )
}

/// Map an AccessKit role to an ARIA role. `None` means plain text or a
/// generic element with no role attribute.
fn aria_role(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Window => "document",
        Role::Label | Role::TextRun | Role::Paragraph | Role::GenericContainer => return None,
        Role::Button | Role::DefaultButton => "button",
        Role::CheckBox => "checkbox",
        Role::RadioButton => "radio",
        Role::RadioGroup => "radiogroup",
        Role::Switch => "switch",
        Role::Link => "link",
        Role::Image => "img",
        Role::TextInput | Role::MultilineTextInput => "textbox",
        Role::SearchInput => "searchbox",
        Role::ComboBox => "combobox",
        Role::ListBox => "listbox",
        Role::ListBoxOption => "option",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::Menu => "menu",
        Role::MenuBar => "menubar",
        Role::MenuItem => "menuitem",
        Role::MenuItemCheckBox => "menuitemcheckbox",
        Role::MenuItemRadio => "menuitemradio",
        Role::Tab => "tab",
        Role::TabList => "tablist",
        Role::TabPanel => "tabpanel",
        Role::Tree => "tree",
        Role::TreeItem => "treeitem",
        Role::Grid => "grid",
        Role::Table => "table",
        Role::Row => "row",
        Role::Cell => "cell",
        Role::ColumnHeader => "columnheader",
        Role::RowHeader => "rowheader",
        Role::Heading => "heading",
        Role::Slider => "slider",
        Role::SpinButton => "spinbutton",
        Role::ProgressIndicator => "progressbar",
        Role::ScrollBar => "scrollbar",
        Role::Dialog => "dialog",
        Role::AlertDialog => "alertdialog",
        Role::Alert => "alert",
        Role::Status => "status",
        Role::Toolbar => "toolbar",
        Role::Tooltip => "tooltip",
        Role::Navigation => "navigation",
        Role::Main => "main",
        Role::Banner => "banner",
        Role::ContentInfo => "contentinfo",
        Role::Article => "article",
        Role::Region => "region",
        Role::Group => "group",
        Role::Log => "log",
        Role::Timer => "timer",
        Role::Marquee => "marquee",
        Role::Meter => "meter",
        Role::Term => "term",
        Role::Definition => "definition",
        Role::Figure => "figure",
        Role::Math => "math",
        Role::Note => "note",
        Role::Form => "form",
        Role::Application => "application",
        _ => "group",
    })
}
