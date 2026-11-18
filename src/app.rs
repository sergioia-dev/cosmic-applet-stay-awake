// SPDX-License-Identifier: MPL-2.0

use std::sync::Arc;

use crate::fl;
use crate::inhibitor::{self, InhibitLock};
use crate::lid_display;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{Limits, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget;

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
#[derive(Default)]
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// The popup id.
    popup: Option<Id>,
    /// Whether the system is currently kept awake. Session-only: never persisted, starts off.
    stay_awake: bool,
    /// Held while awake; dropping it releases the logind inhibitor.
    inhibit: Option<Arc<InhibitLock>>,
    /// Task powering the internal panel off while the lid is closed.
    lid_watcher: Option<tokio::task::JoinHandle<()>>,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ToggleStayAwake(bool),
    InhibitAcquired(Option<Arc<InhibitLock>>),
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "com.user.CosmicAppletStayAwake";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let app = AppModel {
            core,
            ..Default::default()
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// The applet's button in the panel will be drawn using the main view method.
    /// This view should emit messages to toggle the applet's popup window, which will
    /// be drawn using the `view_window` method.
    fn view(&self) -> Element<'_, Self::Message> {
        let icon = if self.stay_awake {
            "com.user.CosmicAppletStayAwake-awake-symbolic"
        } else {
            "com.user.CosmicAppletStayAwake-moon-symbolic"
        };

        self.core
            .applet
            .icon_button(icon)
            .on_press(Message::TogglePopup)
            .into()
    }

    /// The applet's popup window will be drawn using this view method. If there are
    /// multiple poups, you may match the id parameter to determine which popup to
    /// create a view for.
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content_list = widget::list_column().add(widget::settings::item(
            fl!("stay-awake-row"),
            widget::toggler(self.stay_awake).on_toggle(Message::ToggleStayAwake),
        ));

        self.core.applet.popup_container(content_list).into()
    }

    /// Register subscriptions for this application.
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::none()
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime. The application will not exit until all
    /// tasks are finished.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::ToggleStayAwake(true) => {
                self.stay_awake = true;
                self.lid_watcher = Some(lid_display::spawn());
                return cosmic::task::future(async {
                    match inhibitor::acquire().await {
                        Ok(lock) => Message::InhibitAcquired(Some(Arc::new(lock))),
                        Err(why) => {
                            eprintln!("failed to acquire logind inhibitor: {why}");
                            Message::InhibitAcquired(None)
                        }
                    }
                });
            }
            Message::ToggleStayAwake(false) => {
                self.stay_awake = false;
                self.inhibit = None;
                self.stop_lid_watcher();
                // The lid may be closed, so make sure the panel comes back on.
                lid_display::enable_internal();
            }
            Message::InhibitAcquired(Some(lock)) => {
                if self.stay_awake {
                    self.inhibit = Some(lock);
                }
            }
            Message::InhibitAcquired(None) => {
                if self.stay_awake {
                    // Without the inhibitor the applet cannot do what the toggler promises.
                    eprintln!("stay-awake could not be enabled; turning it off");
                    self.stay_awake = false;
                    self.stop_lid_watcher();
                }
            }
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(372.0)
                        .min_width(300.0)
                        .min_height(200.0)
                        .max_height(1080.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    fn stop_lid_watcher(&mut self) {
        if let Some(watcher) = self.lid_watcher.take() {
            watcher.abort();
        }
    }
}
