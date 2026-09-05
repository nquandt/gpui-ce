//! "Device info": device_info, package_info, battery (polled every 5s),
//! connectivity, and network_info (WiFi name/IP — needs entitlement).

use super::ScreenDescriptor;
use super::common::{kv, note, section};
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::{battery, connectivity, device_info, network_info, package_info};
use std::time::Duration;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "device_info",
        title: "Device info",
        category: "Hardware & media",
        blurb: "device, battery, connectivity, network, package",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| {
        let this = DeviceInfoScreen {
            battery: battery::battery_info(),
            connectivity: connectivity::check_connectivity(),
        };
        // connectivity package has no change-listener API in gpui_mobile
        // today, so we just re-poll it alongside battery every 5s.
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                if weak
                    .update(cx, |this: &mut DeviceInfoScreen, cx| {
                        this.battery = battery::battery_info();
                        this.connectivity = connectivity::check_connectivity();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        this
    })
    .into()
}

struct DeviceInfoScreen {
    battery: battery::BatteryInfo,
    connectivity: connectivity::ConnectivityStatus,
}

fn battery_state_label(state: battery::BatteryState) -> &'static str {
    match state {
        battery::BatteryState::Charging => "Charging",
        battery::BatteryState::Discharging => "Discharging",
        battery::BatteryState::Full => "Full",
        battery::BatteryState::Unknown => "Unknown",
    }
}

fn connectivity_label(status: connectivity::ConnectivityStatus) -> &'static str {
    match status {
        connectivity::ConnectivityStatus::Wifi => "WiFi",
        connectivity::ConnectivityStatus::Cellular => "Cellular",
        connectivity::ConnectivityStatus::None => "None",
    }
}

impl Render for DeviceInfoScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let device = device_info::get_device_info();
        let package = package_info::get_package_info();
        let network = network_info::get_network_info();

        div()
            .id("device_info-scroll")
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
                    .child("Device info"),
            )
            .child(note(
                "Static and live device information. Battery and connectivity re-poll every 5s.",
            ))
            .child(section("device_info").children(match device {
                Ok(d) => vec![
                    kv("model", d.model),
                    kv("manufacturer", d.manufacturer),
                    kv("os_version", d.os_version),
                    kv("device_name", d.device_name),
                    kv(
                        "is_physical_device",
                        if d.is_physical_device {
                            "yes"
                        } else {
                            "no (simulator)"
                        },
                    ),
                ],
                Err(e) => vec![kv("error", e)],
            }))
            .child(section("package_info").children(match package {
                Ok(p) => vec![
                    kv("app_name", p.app_name),
                    kv("package_name", p.package_name),
                    kv("version", p.version),
                    kv("build_number", p.build_number),
                ],
                Err(e) => vec![kv("error", e)],
            }))
            .child(
                section("battery")
                    .child(kv("level", format!("{}%", self.battery.level)))
                    .child(kv("state", battery_state_label(self.battery.state)))
                    .child(kv(
                        "low_power_mode",
                        if self.battery.is_battery_save_mode {
                            "yes"
                        } else {
                            "no"
                        },
                    )),
            )
            .child(
                section("connectivity")
                    .child(kv("status", connectivity_label(self.connectivity)))
                    .child(note(
                        "gpui_mobile's connectivity package exposes only a one-shot check — \
                         no change-listener API — so this re-polls on the same 5s timer as \
                         battery instead of reacting to events.",
                    )),
            )
            .child(section("network_info").children(match network {
                Ok(n) => vec![
                    kv("wifi_name", n.wifi_name.unwrap_or_else(|| "(none)".into())),
                    kv(
                        "wifi_bssid",
                        n.wifi_bssid.unwrap_or_else(|| "(none)".into()),
                    ),
                    kv("wifi_ip", n.wifi_ip.unwrap_or_else(|| "(none)".into())),
                ],
                Err(e) => vec![kv("error", e)],
            }))
            .child(note(
                "Reading the WiFi SSID on iOS 12+ requires the \"Access WiFi Information\" \
                 entitlement plus location permission granted; without them wifi_name/bssid \
                 typically read back empty or \"unknown\" even on WiFi.",
            ))
    }
}
