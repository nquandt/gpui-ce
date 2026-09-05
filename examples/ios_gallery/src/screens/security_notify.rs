//! "Security & notify": local_auth (biometrics), notifications (schedule /
//! cancel), and a full permission_handler status table.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::local_auth;
use gpui_mobile::packages::notifications::{self, Notification, NotificationChannel};
use gpui_mobile::packages::permission_handler::{self, Permission, PermissionStatus};
use std::time::Duration;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "security_notify",
        title: "Security & notify",
        category: "Hardware & media",
        blurb: "local_auth, notifications, permission_handler",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| SecurityNotifyScreen {
        auth_status: String::new(),
        notif_status: String::new(),
    })
    .into()
}

struct SecurityNotifyScreen {
    auth_status: String,
    notif_status: String,
}

const ALL_PERMISSIONS: &[Permission] = &[
    Permission::Camera,
    Permission::Microphone,
    Permission::LocationWhenInUse,
    Permission::LocationAlways,
    Permission::Contacts,
    Permission::Calendar,
    Permission::Reminders,
    Permission::Photos,
    Permission::MediaLibrary,
    Permission::Sensors,
    Permission::Bluetooth,
    Permission::Notification,
    Permission::Storage,
    Permission::Speech,
    Permission::AppTrackingTransparency,
    Permission::SystemAlertWindow,
    Permission::InstallPackages,
    Permission::AccessNotificationPolicy,
    Permission::Phone,
    Permission::Sms,
    Permission::Videos,
    Permission::Audio,
];

fn permission_status_label(p: PermissionStatus) -> &'static str {
    match p {
        PermissionStatus::Granted => "Granted",
        PermissionStatus::Denied => "Denied",
        PermissionStatus::Restricted => "Restricted",
        PermissionStatus::PermanentlyDenied => "PermanentlyDenied",
        PermissionStatus::Limited => "Limited",
        PermissionStatus::Provisional => "Provisional",
    }
}

impl Render for SecurityNotifyScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let device_supported = local_auth::is_device_supported();
        let can_authenticate = local_auth::can_authenticate();
        let biometrics = local_auth::get_available_biometrics();

        div()
            .id("security_notify-scroll")
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
                    .child("Security & notify"),
            )
            .child(note(
                "Biometric authentication, local notifications, and a table of every runtime \
                 permission this backend can request.",
            ))
            .child(
                section("local_auth")
                    .child(kv(
                        "device_supported",
                        format!("{device_supported:?}"),
                    ))
                    .child(kv("can_authenticate", format!("{can_authenticate:?}")))
                    .child(kv("available_biometrics", format!("{biometrics:?}")))
                    .child(button(
                        "Authenticate (\"unlock gallery\")",
                        cx.listener(|this, _, _window, cx| {
                            this.auth_status = match local_auth::authenticate("unlock gallery") {
                                Ok(result) => format!("{result:?}"),
                                Err(e) => format!("error: {e}"),
                            };
                            gallery_log::push(format!(
                                "security_notify: authenticate -> {}",
                                this.auth_status
                            ));
                            cx.notify();
                        }),
                    ))
                    .child(note(self.auth_status.clone())),
            )
            .child(
                section("notifications")
                    .child(note(
                        "notifications::show() has no scheduling parameter on iOS (its trigger is \
                         always immediate — see packages/notifications/ios.rs), so \"schedule 5s out\" \
                         is emulated here with a background timer that calls show() after 5s. Background \
                         the app right after tapping Schedule to see the banner.",
                    ))
                    .child(row().flex_wrap()
                        .child(button(
                            "Initialize",
                            cx.listener(|this, _, _window, cx| {
                                this.notif_status = match notifications::initialize() {
                                    Ok(()) => "initialized".into(),
                                    Err(e) => format!("initialize error: {e}"),
                                };
                                gallery_log::push(format!(
                                    "security_notify: notifications initialize -> {}",
                                    this.notif_status
                                ));
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Schedule (5s)",
                            cx.listener(|this, _, _window, cx| {
                                gallery_log::push("security_notify: scheduling notification in 5s");
                                this.notif_status = "scheduled — background the app now".into();
                                cx.spawn(async move |_this, cx| {
                                    cx.background_executor()
                                        .timer(Duration::from_secs(5))
                                        .await;
                                    let notification = Notification {
                                        id: 1,
                                        title: "GPUI Gallery".into(),
                                        body: "Scheduled notification fired".into(),
                                        channel: NotificationChannel::default(),
                                        payload: None,
                                    };
                                    let result = notifications::show(&notification);
                                    gallery_log::push(format!(
                                        "security_notify: scheduled notification show -> {result:?}"
                                    ));
                                })
                                .detach();
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Cancel #1",
                            cx.listener(|this, _, _window, cx| {
                                this.notif_status = match notifications::cancel(1) {
                                    Ok(()) => "cancelled #1".into(),
                                    Err(e) => format!("cancel error: {e}"),
                                };
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Cancel all",
                            cx.listener(|this, _, _window, cx| {
                                this.notif_status = match notifications::cancel_all() {
                                    Ok(()) => "cancelled all".into(),
                                    Err(e) => format!("cancel_all error: {e}"),
                                };
                                cx.notify();
                            }),
                        )))
                    .child(note(self.notif_status.clone())),
            )
            .child(
                section("permission_handler")
                    .children(ALL_PERMISSIONS.iter().map(|&p| {
                        let status = permission_handler::check_permission(p);
                        row()
                            .justify_between()
                            .w_full()
                            .child(kv(
                                format!("{p:?}"),
                                status
                                    .map(permission_status_label)
                                    .unwrap_or("error")
                                    .to_string(),
                            ))
                            .child(button(
                                format!("Request {p:?}"),
                                cx.listener(move |_this, _, _window, cx| {
                                    let result = permission_handler::request_permission(p);
                                    gallery_log::push(format!(
                                        "security_notify: request {p:?} -> {result:?}"
                                    ));
                                    cx.notify();
                                }),
                            ))
                    })),
            )
    }
}
