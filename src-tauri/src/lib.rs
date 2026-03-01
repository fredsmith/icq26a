#![recursion_limit = "512"]

mod commands;
mod matrix_client;

use matrix_client::{MatrixState, TrayAnimationState};
use tauri::{Emitter, Manager};

/// Create a tray icon image from the green flower PNG, optionally tinting it red.
fn flower_icon(red_tint: bool) -> tauri::image::Image<'static> {
    let original = tauri::image::Image::from_bytes(include_bytes!("../../public/loaded-flower.png"))
        .expect("failed to decode flower icon");
    if !red_tint {
        return original;
    }
    // Swap R and G channels to turn the green flower red
    let width = original.width();
    let height = original.height();
    let mut rgba = original.rgba().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        let r = pixel[0];
        let g = pixel[1];
        pixel[0] = g; // R ← G (the green component becomes red)
        pixel[1] = r / 3; // dim the green
        pixel[2] = pixel[2] / 3; // dim the blue
    }
    tauri::image::Image::new_owned(rgba, width, height)
}

#[cfg(target_os = "macos")]
fn hide_dock_icon() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    unsafe {
        let mtm = MainThreadMarker::new_unchecked();
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    // On macOS with Accessory policy, also activate the app
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSApplication;
        if let Some(mtm) = MainThreadMarker::new() {
            let ns_app = NSApplication::sharedApplication(mtm);
            #[allow(deprecated)]
            ns_app.activateIgnoringOtherApps(true);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(MatrixState::new())
        .manage(TrayAnimationState::new())
        .setup(|app| {
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
            use tauri::tray::TrayIconBuilder;

            // Build tray menu
            let menu = Menu::with_items(app, &[
                &MenuItem::with_id(app, "buddy_list", "Buddy List", true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "toggle_status", "Go Offline", true, None::<&str>)?,
                &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "quit", "Quit ICQ26a", true, None::<&str>)?,
            ])?;

            // Start with red (disconnected) icon
            let icon = flower_icon(true);
            TrayIconBuilder::with_id("tray_main")
                .icon(icon)
                .tooltip("ICQ26a")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "buddy_list" => {
                            show_main_window(app);
                        }
                        "toggle_status" => {
                            let _ = app.emit("tray_toggle_status", ());
                        }
                        "settings" => {
                            let _ = app.emit("tray_open_settings", ());
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Hide from Dock on macOS — app lives in the menu bar
            #[cfg(target_os = "macos")]
            hide_dock_icon();

            Ok(())
        })
        // Hide main window instead of quitting when closed
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::matrix_login,
            commands::matrix_register,
            commands::matrix_logout,
            commands::matrix_disconnect,
            commands::try_restore_session,
            commands::get_buddy_list,
            commands::get_room_members,
            commands::get_rooms,
            commands::get_spaces,
            commands::get_room_messages,
            commands::send_message,
            commands::set_presence,
            commands::start_sync,
            commands::upload_file,
            commands::fetch_media,
            commands::get_server_log,
            commands::accept_verification,
            commands::confirm_verification,
            commands::cancel_verification,
            commands::get_user_profile,
            commands::get_room_info,
            commands::create_dm_room,
            commands::search_users,
            commands::join_room,
            commands::create_room,
            commands::leave_room,
            commands::remove_buddy,
            commands::send_typing,
            commands::mark_as_read,
            commands::edit_message,
            commands::delete_message,
            commands::send_reaction,
            commands::get_pending_invites,
            commands::accept_invite,
            commands::reject_invite,
            commands::get_room_tags,
            commands::set_room_tag,
            commands::remove_room_tag,
            commands::set_dock_badge,
            commands::search_spaces,
            commands::get_space_hierarchy,
            commands::update_tray_icon,
            commands::show_emoji_picker,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
