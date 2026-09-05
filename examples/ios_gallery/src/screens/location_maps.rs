//! "Location & maps": location permission + one-shot/streamed position,
//! an embedded MapKit view, maps_launcher, contacts, calendar, and deeplink.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::components::platform_view_element::platform_view_element;
use gpui_mobile::packages::calendar;
use gpui_mobile::packages::contacts;
use gpui_mobile::packages::deeplink;
use gpui_mobile::packages::location::{self, LocationSettings, Position};
use gpui_mobile::packages::maps::{LatLng, MapMarker, MapSettings, MapType, MapView};
use gpui_mobile::packages::maps_launcher;
use gpui_mobile::packages::permission_handler::{self, Permission, PermissionStatus};
use std::time::Duration;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "location_maps",
        title: "Location & maps",
        category: "Hardware & media",
        blurb: "location, maps, maps_launcher, contacts, calendar, deeplink",
        build,
    }
}

const SF: LatLng = LatLng {
    latitude: 37.7749,
    longitude: -122.4194,
};

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| LocationMapsScreen {
        location_status: String::new(),
        last_position: None,
        streaming: false,
        stream_samples: 0,
        map: MapView::new(MapSettings {
            center: SF,
            ..Default::default()
        })
        .ok(),
        map_status: String::new(),
        launcher_status: String::new(),
        contacts_status: String::new(),
        contact_names: Vec::new(),
        calendar_status: String::new(),
        event_count: None,
    })
    .into()
}

struct LocationMapsScreen {
    location_status: String,
    last_position: Option<Position>,
    streaming: bool,
    stream_samples: u32,
    map: Option<MapView>,
    map_status: String,
    launcher_status: String,
    contacts_status: String,
    contact_names: Vec<String>,
    calendar_status: String,
    event_count: Option<usize>,
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

impl LocationMapsScreen {
    fn start_stream(&mut self, cx: &mut Context<Self>) {
        self.streaming = true;
        self.stream_samples = 0;
        gallery_log::push("location_maps: started position stream (polled)");
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let result = weak.update(cx, |this: &mut LocationMapsScreen, cx| {
                    if !this.streaming {
                        return false;
                    }
                    match location::get_current_position(&LocationSettings::default()) {
                        Ok(pos) => {
                            this.last_position = Some(pos);
                            this.stream_samples += 1;
                        }
                        Err(e) => this.location_status = format!("stream error: {e}"),
                    }
                    cx.notify();
                    true
                });
                match result {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn stop_stream(&mut self, cx: &mut Context<Self>) {
        self.streaming = false;
        gallery_log::push("location_maps: stopped position stream");
        cx.notify();
    }
}

impl Render for LocationMapsScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let permission = permission_handler::check_permission(Permission::LocationWhenInUse);
        let service_enabled = location::is_location_service_enabled();
        let latest_deep_link = deeplink::get_latest_link();

        div()
            .id("location_maps-scroll")
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
                    .child("Location & maps"),
            )
            .child(note(
                "location_maps combines GPS position, an embedded MapKit view, contacts, \
                 calendar and deep links.",
            ))
            .child(
                section("location")
                    .child(kv(
                        "permission",
                        permission.map(permission_label).unwrap_or("error").to_string(),
                    ))
                    .child(kv("service_enabled", format!("{service_enabled:?}")))
                    .child(row().flex_wrap()
                        .child(button(
                            "Request permission",
                            cx.listener(|this, _, _window, cx| {
                                let result = permission_handler::request_permission(
                                    Permission::LocationWhenInUse,
                                );
                                this.location_status = format!("permission -> {result:?}");
                                gallery_log::push(format!(
                                    "location_maps: request permission -> {}",
                                    this.location_status
                                ));
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Get position (one-shot)",
                            cx.listener(|this, _, _window, cx| {
                                match location::get_current_position(&LocationSettings::default())
                                {
                                    Ok(pos) => {
                                        this.location_status = "got one-shot position".into();
                                        this.last_position = Some(pos);
                                    }
                                    Err(e) => this.location_status = format!("error: {e}"),
                                }
                                gallery_log::push(format!(
                                    "location_maps: one-shot -> {}",
                                    this.location_status
                                ));
                                cx.notify();
                            }),
                        )))
                    .child(row().flex_wrap()
                        .child(button(
                            "Start stream",
                            cx.listener(|this, _, _window, cx| this.start_stream(cx)),
                        ))
                        .child(button(
                            "Stop stream",
                            cx.listener(|this, _, _window, cx| this.stop_stream(cx)),
                        )))
                    .child(note(
                        "gpui_mobile's location package has no push-based watch/stream API — \
                         only a one-shot get_current_position() — so \"stream\" here polls it \
                         once per second instead.",
                    ))
                    .child(kv("streaming", self.streaming.to_string()))
                    .child(kv("samples", self.stream_samples.to_string()))
                    .children(self.last_position.map(|p| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(kv("lat, lon", format!("{:.5}, {:.5}", p.latitude, p.longitude)))
                            .child(kv("altitude", format!("{:.1}m", p.altitude)))
                            .child(kv("accuracy", format!("{:.1}m", p.accuracy)))
                            .child(kv("speed", format!("{:.1}m/s", p.speed)))
                            .child(kv("heading", format!("{:.1} deg", p.heading)))
                    }))
                    .child(note(self.location_status.clone())),
            )
            .child(
                section("maps (embedded MapKit)")
                    .child(
                        div()
                            .w_full()
                            .h(px(300.0))
                            .bg(rgb(0x111111))
                            .child(
                                self.map
                                    .as_ref()
                                    .and_then(|m| m.platform_view_handle())
                                    .map(platform_view_element)
                                    .unwrap_or_else(|| div().size_full()),
                            ),
                    )
                    .child(note(
                        "The iOS maps backend (packages/maps/ios.rs) is TODO stubs: set_center, \
                         set_zoom, set_map_type, add_marker and clear_markers all just log and \
                         return Ok(()) without touching the MKMapView, so tapping these buttons \
                         will not visibly change the map.",
                    ))
                    .child(row().flex_wrap()
                        .child(button(
                            "set_center(SF)",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(m) = this.map.as_mut() {
                                    this.map_status = format!("set_center -> {:?}", m.set_center(SF));
                                }
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "set_zoom(16)",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(m) = this.map.as_mut() {
                                    this.map_status = format!("set_zoom -> {:?}", m.set_zoom(16.0));
                                }
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Satellite",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(m) = this.map.as_mut() {
                                    this.map_status =
                                        format!("set_map_type -> {:?}", m.set_map_type(MapType::Satellite));
                                }
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Add marker",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(m) = this.map.as_ref() {
                                    let marker = MapMarker {
                                        id: "sf".into(),
                                        position: SF,
                                        title: Some("San Francisco".into()),
                                        snippet: None,
                                    };
                                    this.map_status = format!("add_marker -> {:?}", m.add_marker(&marker));
                                }
                                cx.notify();
                            }),
                        )))
                    .child(note(self.map_status.clone())),
            )
            .child(
                section("maps_launcher")
                    .child(button(
                        "Open SF in Maps app",
                        cx.listener(|this, _, _window, cx| {
                            this.launcher_status = format!(
                                "{:?}",
                                maps_launcher::open_coordinates(
                                    SF.latitude,
                                    SF.longitude,
                                    Some("San Francisco"),
                                )
                            );
                            gallery_log::push(format!(
                                "location_maps: maps_launcher -> {}",
                                this.launcher_status
                            ));
                            cx.notify();
                        }),
                    ))
                    .child(note(self.launcher_status.clone())),
            )
            .child(
                section("contacts")
                    .child(note("Requires Contacts permission — request it under Security & notify or the OS will prompt on first read."))
                    .child(button(
                        "Load first 3 contacts",
                        cx.listener(|this, _, _window, cx| {
                            match contacts::get_contacts() {
                                Ok(list) => {
                                    this.contacts_status = format!("{} contacts total", list.len());
                                    this.contact_names =
                                        list.iter().take(3).map(|c| c.display_name.clone()).collect();
                                }
                                Err(e) => this.contacts_status = format!("error: {e}"),
                            }
                            gallery_log::push(format!(
                                "location_maps: contacts -> {}",
                                this.contacts_status
                            ));
                            cx.notify();
                        }),
                    ))
                    .child(note(self.contacts_status.clone()))
                    .children(self.contact_names.iter().cloned().map(|n| kv("", n))),
            )
            .child(
                section("calendar")
                    .child(note("Counts events across every calendar in the next 7 days."))
                    .child(button(
                        "Count events (next 7 days)",
                        cx.listener(|this, _, _window, cx| {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as i64)
                                .unwrap_or(0);
                            let week_ms = now_ms + 7 * 24 * 60 * 60 * 1000;
                            match calendar::get_calendars() {
                                Ok(cals) => {
                                    let mut total = 0usize;
                                    for cal in &cals {
                                        if let Ok(events) =
                                            calendar::get_events(&cal.id, now_ms, week_ms)
                                        {
                                            total += events.len();
                                        }
                                    }
                                    this.event_count = Some(total);
                                    this.calendar_status =
                                        format!("{} calendars, {} events", cals.len(), total);
                                }
                                Err(e) => this.calendar_status = format!("error: {e}"),
                            }
                            gallery_log::push(format!(
                                "location_maps: calendar -> {}",
                                this.calendar_status
                            ));
                            cx.notify();
                        }),
                    ))
                    .child(kv(
                        "events (7d)",
                        self.event_count.map(|c| c.to_string()).unwrap_or_else(|| "?".into()),
                    ))
                    .child(note(self.calendar_status.clone())),
            )
            .child(
                section("deeplink")
                    .child(kv(
                        "latest link",
                        latest_deep_link.unwrap_or_else(|| "(none yet)".into()),
                    ))
                    .child(note(
                        "Open Safari on the simulator/device and navigate to \
                         gpuigallery://screen/location_maps to test — the readout above updates \
                         once the app is foregrounded again.",
                    )),
            )
    }
}
