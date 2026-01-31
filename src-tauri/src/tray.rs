//! System tray management with dynamic icons and menus

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, Runtime, Emitter};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::image::Image;

pub fn init<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit LimitsWatcher", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh All", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    
    let menu = Menu::with_items(app, &[&show, &refresh, &separator, &quit])?;
    
    // Load tray icon
    // Note: We use the default icon path for now, but in production this should be bundled
    // and loaded more robustly.
    let icon = Image::from_path("icons/icon.png")
        .unwrap_or_else(|_| Image::from_bytes(include_bytes!("../icons/icon.png")).unwrap());
    
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("LimitsWatcher - AI Usage Tracker")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "refresh" => {
                    // Emit refresh event to frontend
                    let _ = app.emit("refresh-all", ());
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    .. 
                } => {
                    // Left click: show main window
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}

/// Update tray icon based on usage status
pub fn update_icon<R: Runtime>(app: &tauri::AppHandle<R>, status: TrayStatus) {
    // TODO: Generate dynamic icon based on usage bars
    // For now, just update tooltip
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = match status {
            TrayStatus::Ok { summary } => format!("LimitsWatcher\n{}", summary),
            TrayStatus::Warning { message } => format!("⚠️ {}", message),
            TrayStatus::Error { message } => format!("❌ {}", message),
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

pub enum TrayStatus {
    Ok { summary: String },
    Warning { message: String },
    Error { message: String },
}