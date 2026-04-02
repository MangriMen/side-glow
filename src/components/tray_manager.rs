use crate::core::SharedSettings;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder, TrayIconEvent,
};

pub struct TrayManager {
    _tray_icon: TrayIcon,
}

impl TrayManager {
    pub fn new(icon: tray_icon::Icon) -> Self {
        let quit_item = MenuItem::with_id("quit", "Quit", true, None);
        let settings_item = MenuItem::with_id("settings", "Settings", true, None);

        let tray_menu = Menu::new();
        let _ = tray_menu.append_items(&[&settings_item, &quit_item]);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("AmbientSide")
            .with_icon(icon)
            .with_menu_on_left_click(false)
            .build()
            .unwrap();

        Self {
            _tray_icon: tray_icon,
        }
    }

    pub fn handle_events(&self, settings: SharedSettings) -> bool {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.as_ref() {
                "quit" => std::process::exit(0),
                "settings" => settings.write().show_settings = true,
                _ => {}
            }
        }

        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    ..
                }
            ) {
                settings.write().show_settings = true;
            }
        }
        false
    }
}
