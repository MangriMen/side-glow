use anyhow::{Context as _, Result};
use eframe::egui::{self, ViewportId};
use std::sync::mpsc::{self, Receiver};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const SETTINGS_ID: &str = "settings";
const PAUSE_ID: &str = "pause";
const QUIT_ID: &str = "quit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    ShowSettings,
    TogglePause,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    pause_item: CheckMenuItem,
    commands: Receiver<TrayCommand>,
}

impl Tray {
    /// Tray events are forwarded to a channel and wake up the (possibly hidden, idle)
    /// root viewport, which is the only place they are processed.
    pub fn new(icon: tray_icon::Icon, ctx: &egui::Context) -> Result<Self> {
        let settings_item = MenuItem::with_id(SETTINGS_ID, "Settings", true, None);
        let pause_item = CheckMenuItem::with_id(PAUSE_ID, "Pause", true, false, None);
        let quit_item = MenuItem::with_id(QUIT_ID, "Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &settings_item,
            &pause_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])
        .context("building tray menu")?;

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("SideGlow")
            .with_icon(icon)
            .with_menu_on_left_click(false)
            .build()
            .context("creating tray icon")?;

        let (sender, commands) = mpsc::channel();
        let menu_sender = sender.clone();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let command = match event.id.as_ref() {
                SETTINGS_ID => TrayCommand::ShowSettings,
                PAUSE_ID => TrayCommand::TogglePause,
                QUIT_ID => TrayCommand::Quit,
                _ => return,
            };
            let _ = menu_sender.send(command);
            menu_ctx.request_repaint_of(ViewportId::ROOT);
        }));

        let icon_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = sender.send(TrayCommand::ShowSettings);
                icon_ctx.request_repaint_of(ViewportId::ROOT);
            }
        }));

        Ok(Self {
            _icon: icon,
            pause_item,
            commands,
        })
    }

    pub fn poll(&self) -> impl Iterator<Item = TrayCommand> + '_ {
        self.commands.try_iter()
    }

    pub fn set_paused(&self, paused: bool) {
        self.pause_item.set_checked(paused);
    }
}
