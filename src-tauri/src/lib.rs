#![recursion_limit = "512"]

mod commands;
mod matrix_client;

use matrix_client::MatrixState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(MatrixState::new())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
