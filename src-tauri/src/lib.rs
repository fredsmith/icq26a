#![recursion_limit = "512"]

mod commands;
mod matrix_client;

use matrix_client::MatrixState;

#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use objc2::MainThreadMarker;
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    unsafe {
        let bytes = include_bytes!("../icons/icon.png");
        let data = NSData::with_bytes(bytes);
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
            // Safe: setup runs on the main thread
            let mtm = MainThreadMarker::new_unchecked();
            let app = NSApplication::sharedApplication(mtm);
            app.setApplicationIconImage(Some(&image));
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(MatrixState::new())
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            set_dock_icon();
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
