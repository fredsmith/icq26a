use crate::matrix_client::{
    Buddy, LogEntry, LoginCredentials, MatrixState, Message, PersistedSession, Room,
    ServerLog, VerificationEmoji, VerificationEmojisEvent, VerificationEvent,
};
use matrix_sdk::{Client, ServerName};
use tauri::{Emitter, State};

/// Log helper — pushes to buffer and emits real-time event to frontend.
fn slog(app: &tauri::AppHandle, log: &ServerLog, level: &str, message: String) {
    log.push(level, message.clone());
    let entry = LogEntry {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        level: level.to_string(),
        message,
    };
    let _ = app.emit("server_log", &entry);
}

/// Log helper for contexts without an AppHandle (just buffer).
fn slog_buf(log: &ServerLog, level: &str, message: String) {
    log.push(level, message);
}

/// Wraps a future with periodic heartbeat log messages if it takes longer than 5s.
async fn with_heartbeat<F, T>(
    app: &tauri::AppHandle,
    log: &std::sync::Arc<ServerLog>,
    label: &str,
    fut: F,
) -> T
where
    F: std::future::Future<Output = T>,
{
    let app = app.clone();
    let log = log.clone();
    let label = label.to_string();
    tokio::pin!(fut);

    let mut elapsed = 0u64;
    loop {
        tokio::select! {
            result = &mut fut => {
                if elapsed >= 5 {
                    slog(&app, &log, "info", format!("{} completed after {}s", label, elapsed));
                }
                return result;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                elapsed += 5;
                slog(&app, &log, "info", format!("{} still waiting... ({}s)", label, elapsed));
            }
        }
    }
}

/// Returns Some(status) on success, None if the server doesn't support presence
/// (or returns stub/stale data, e.g. matrix.org with presence disabled).
async fn fetch_user_presence(client: &Client, user_id: &matrix_sdk::ruma::UserId) -> Option<String> {
    use matrix_sdk::ruma::api::client::presence::get_presence;

    let request = get_presence::v3::Request::new(user_id.to_owned());
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.send(request),
    ).await {
        Ok(Ok(response)) => {
            // Servers with presence disabled (e.g. matrix.org) return 200 but with
            // stale default data: offline, no last_active_ago, no currently_active.
            // Treat this as "not supported" so we don't show bogus offline statuses.
            if response.currently_active.is_none()
                && response.last_active_ago.is_none()
                && response.presence == matrix_sdk::ruma::presence::PresenceState::Offline
            {
                return None;
            }
            Some(match response.presence {
                matrix_sdk::ruma::presence::PresenceState::Online => "online",
                matrix_sdk::ruma::presence::PresenceState::Unavailable => "away",
                _ => "offline",
            }.to_string())
        }
        _ => None, // timeout or error
    }
}

#[tauri::command]
pub async fn get_server_log(
    state: State<'_, MatrixState>,
) -> Result<Vec<LogEntry>, String> {
    Ok(state.log.get_all())
}

#[tauri::command]
pub async fn matrix_login(
    credentials: LoginCredentials,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<String, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("Logging in as {} to {}", credentials.username, credentials.homeserver));

    let server_name = ServerName::parse(&credentials.homeserver.replace("https://", ""))
        .map_err(|e| {
            slog(&app, &log, "error", format!("Invalid homeserver: {}", e));
            format!("Invalid homeserver: {}", e)
        })?;

    let data_path = crate::matrix_client::data_dir()?;
    // Fresh login — clear stale sqlite stores to avoid device ID mismatches
    if data_path.exists() {
        slog(&app, &log, "info", "Clearing old sqlite store for fresh login...".into());
        let _ = std::fs::remove_dir_all(&data_path);
    }
    std::fs::create_dir_all(&data_path)
        .map_err(|e| format!("Failed to create data dir: {}", e))?;

    slog(&app, &log, "info", "Building client with sqlite store...".into());
    let client = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        with_heartbeat(&app, &log, "Client build",
            Client::builder()
                .server_name(&server_name)
                .sqlite_store(&data_path, None)
                .build(),
        ),
    )
        .await
        .map_err(|_| {
            slog(&app, &log, "error", "Client build timed out after 60s".into());
            "Client build timed out — try restarting the app".to_string()
        })?
        .map_err(|e| {
            slog(&app, &log, "error", format!("Failed to build client: {}", e));
            format!("Failed to build client: {}", e)
        })?;

    slog(&app, &log, "info", format!("Resolved homeserver: {}", client.homeserver()));
    slog(&app, &log, "info", "Sending login request...".into());

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client
            .matrix_auth()
            .login_username(&credentials.username, &credentials.password)
            .initial_device_display_name("icq26a")
            .send(),
    )
        .await
        .map_err(|_| {
            slog(&app, &log, "error", "Login request timed out after 30s".into());
            "Login timed out".to_string()
        })?
        .map_err(|e| {
            slog(&app, &log, "error", format!("Login failed: {}", e));
            format!("Login failed: {}", e)
        })?;

    let user_id = response.user_id.to_string();
    slog(&app, &log, "info", format!("Login OK — user_id={}, device_id={}", user_id, response.device_id));

    // Save session for restore on next launch
    let session_data = PersistedSession {
        homeserver_url: client.homeserver().to_string(),
        user_id: response.user_id.to_string(),
        device_id: response.device_id.to_string(),
        access_token: response.access_token.clone(),
        refresh_token: response.refresh_token.clone(),
    };
    let session_path = crate::matrix_client::session_file_path()?;
    let json = serde_json::to_string_pretty(&session_data)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    std::fs::write(&session_path, json)
        .map_err(|e| format!("Failed to write session: {}", e))?;
    slog(&app, &log, "info", "Session saved to disk".into());

    let mut client_lock = state.client.lock().await;
    *client_lock = Some(client);

    Ok(user_id)
}

#[tauri::command]
pub async fn matrix_logout(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", "Logging out...".into());

    state.abort_sync_tasks();

    let mut client_lock = state.client.lock().await;
    if let Some(client) = client_lock.as_ref() {
        let _ = client.matrix_auth().logout().await;
    }
    *client_lock = None;

    if let Ok(path) = crate::matrix_client::session_file_path() {
        let _ = std::fs::remove_file(path);
    }

    slog(&app, &log, "info", "Logged out, session file deleted".into());
    Ok(())
}

#[tauri::command]
pub async fn matrix_disconnect(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", "Disconnecting (keeping session)...".into());

    state.abort_sync_tasks();

    let mut client_lock = state.client.lock().await;
    *client_lock = None;

    slog(&app, &log, "info", "Disconnected, session file preserved".into());
    Ok(())
}

#[tauri::command]
pub async fn try_restore_session(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<String, String> {
    let log = state.log.clone();
    let session_path = crate::matrix_client::session_file_path()?;

    if !session_path.exists() {
        slog(&app, &log, "info", "No saved session found".into());
        return Err("No saved session".to_string());
    }

    slog(&app, &log, "info", "Restoring saved session...".into());

    let json = std::fs::read_to_string(&session_path)
        .map_err(|e| format!("Failed to read session: {}", e))?;
    let saved: PersistedSession = serde_json::from_str(&json)
        .map_err(|e| {
            slog(&app, &log, "error", format!("Corrupt session file: {}", e));
            format!("Failed to parse session: {}", e)
        })?;

    slog(&app, &log, "info", format!("Session file: user={}, homeserver={}", saved.user_id, saved.homeserver_url));

    let data_path = crate::matrix_client::data_dir()?;

    let client = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        with_heartbeat(&app, &log, "Client build",
            Client::builder()
                .homeserver_url(&saved.homeserver_url)
                .sqlite_store(&data_path, None)
                .build(),
        ),
    )
        .await
        .map_err(|_| {
            slog(&app, &log, "error", "Client build timed out after 60s".into());
            "Session restore timed out — try restarting the app".to_string()
        })?
        .map_err(|e| {
            slog(&app, &log, "error", format!("Failed to build client: {}", e));
            format!("Failed to build client: {}", e)
        })?;

    let session = matrix_sdk::authentication::matrix::MatrixSession {
        meta: matrix_sdk::SessionMeta {
            user_id: matrix_sdk::ruma::UserId::parse(&saved.user_id)
                .map_err(|e| format!("Invalid user_id: {}", e))?,
            device_id: saved.device_id.as_str().into(),
        },
        tokens: matrix_sdk::SessionTokens {
            access_token: saved.access_token,
            refresh_token: saved.refresh_token,
        },
    };

    client
        .restore_session(session)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Session restore failed: {}", e));
            format!("Failed to restore session: {}", e)
        })?;

    slog(&app, &log, "info", format!("Session restored — user={}", saved.user_id));

    let user_id = saved.user_id;

    let mut client_lock = state.client.lock().await;
    *client_lock = Some(client);

    Ok(user_id)
}

#[tauri::command]
pub async fn get_buddy_list(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Vec<Buddy>, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", "get_buddy_list: running sync_once...".into());

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    with_heartbeat(&app, &log, "sync_once", client.sync_once(Default::default()))
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("sync_once failed: {}", e));
            format!("Sync failed: {}", e)
        })?;

    slog(&app, &log, "info", "sync_once complete, scanning rooms...".into());

    let joined = client.joined_rooms();
    slog(&app, &log, "info", format!("Found {} joined rooms", joined.len()));

    let mut buddies = Vec::new();
    let mut presence_supported = true;
    for room in joined {
        if room.is_direct().await.unwrap_or(false) {
            let members = room
                .members(matrix_sdk::RoomMemberships::ACTIVE)
                .await
                .unwrap_or_default();
            for member in members {
                let user_id = member.user_id().to_string();
                if user_id
                    != client
                        .user_id()
                        .map(|u| u.to_string())
                        .unwrap_or_default()
                {
                    let presence = if presence_supported {
                        match fetch_user_presence(client, member.user_id()).await {
                            Some(p) => p,
                            None => {
                                slog(&app, &log, "warn", "Presence not supported by server, skipping remaining".into());
                                presence_supported = false;
                                "unknown".to_string()
                            }
                        }
                    } else {
                        "unknown".to_string()
                    };
                    buddies.push(Buddy {
                        user_id: user_id.clone(),
                        display_name: member.display_name().unwrap_or(&user_id).to_string(),
                        avatar_url: member.avatar_url().map(|u| u.to_string()),
                        presence,
                    });
                }
            }
        }
    }
    slog(&app, &log, "info", format!("get_buddy_list: returning {} buddies", buddies.len()));
    Ok(buddies)
}

#[tauri::command]
pub async fn get_room_members(
    room_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Vec<Buddy>, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("get_room_members: {}", room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let members = with_heartbeat(
        &app, &log, "get_room_members",
        room.members(matrix_sdk::RoomMemberships::ACTIVE),
    )
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Failed to get members: {}", e));
            format!("Failed to get members: {}", e)
        })?;

    slog(&app, &log, "info", format!("Room has {} active members", members.len()));

    let buddies: Vec<Buddy> = members
        .iter()
        .map(|member| {
            let user_id = member.user_id().to_string();
            Buddy {
                display_name: member.display_name().unwrap_or(&user_id).to_string(),
                avatar_url: member.avatar_url().map(|u| u.to_string()),
                presence: "offline".to_string(),
                user_id,
            }
        })
        .collect();

    slog(&app, &log, "info", format!("get_room_members: returning {} members", buddies.len()));
    Ok(buddies)
}

#[tauri::command]
pub async fn get_rooms(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Vec<Room>, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", "get_rooms: fetching joined rooms...".into());

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let mut rooms = Vec::new();
    for room in client.joined_rooms() {
        rooms.push(Room {
            room_id: room.room_id().to_string(),
            name: room
                .display_name()
                .await
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "Unknown".to_string()),
            is_direct: room.is_direct().await.unwrap_or(false),
            last_message: None,
            unread_count: 0,
        });
    }
    slog(&app, &log, "info", format!("get_rooms: returning {} rooms", rooms.len()));
    Ok(rooms)
}

#[tauri::command]
pub async fn get_room_messages(
    room_id: String,
    limit: u64,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Vec<Message>, String> {
    let _ = limit;
    let log = state.log.clone();
    slog(&app, &log, "info", format!("get_room_messages: {}", room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    slog(&app, &log, "info", "Fetching messages from server...".into());
    let options = matrix_sdk::room::MessagesOptions::backward();
    let messages_response = with_heartbeat(
        &app, &log, "messages",
        room.messages(options),
    )
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Failed to get messages: {}", e));
            format!("Failed to get messages: {}", e)
        })?;

    let mut messages = Vec::new();
    for event in messages_response.chunk {
        if let Ok(timeline_event) = event.raw().deserialize() {
            if let matrix_sdk::ruma::events::AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg),
            ) = timeline_event
            {
                let (body, msg_type) = match msg.as_original() {
                    Some(original) => match &original.content.msgtype {
                        matrix_sdk::ruma::events::room::message::MessageType::Text(text) => {
                            (text.body.clone(), "text".to_string())
                        }
                        _ => (String::new(), "unknown".to_string()),
                    },
                    None => (String::new(), "unknown".to_string()),
                };

                messages.push(Message {
                    room_id: room_id.to_string(),
                    event_id: msg.event_id().to_string(),
                    sender: msg.sender().to_string(),
                    sender_name: msg.sender().localpart().to_string(),
                    body,
                    timestamp: msg.origin_server_ts().as_secs().into(),
                    msg_type,
                });
            }
        }
    }
    messages.reverse();
    slog(&app, &log, "info", format!("get_room_messages: returning {} messages", messages.len()));
    Ok(messages)
}

#[tauri::command]
pub async fn send_message(
    room_id: String,
    body: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("send_message: room={}, len={}", room_id, body.len()));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let content =
        matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(&body);
    room.send(content)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Send failed: {}", e));
            format!("Send failed: {}", e)
        })?;

    slog(&app, &log, "info", "Message sent OK".into());
    Ok(())
}

#[tauri::command]
pub async fn set_presence(status: String, state: State<'_, MatrixState>) -> Result<(), String> {
    // Stub — will map ICQ statuses to Matrix presence
    let _ = (status, state);
    Ok(())
}

#[tauri::command]
pub async fn start_sync(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", "start_sync: beginning background sync...".into());

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?.clone();
    drop(client_lock);

    let app_handle = app.clone();

    // Abort any existing sync tasks before starting new ones
    state.abort_sync_tasks();

    // Poll for room list changes (new rooms joined from other clients)
    let poll_client = client.clone();
    let poll_app = app.clone();
    let poll_log = log.clone();
    let poll_task = tokio::spawn(async move {
        let mut known_ids: std::collections::HashSet<String> = poll_client
            .joined_rooms()
            .iter()
            .map(|r| r.room_id().to_string())
            .collect();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let current_ids: std::collections::HashSet<String> = poll_client
                .joined_rooms()
                .iter()
                .map(|r| r.room_id().to_string())
                .collect();
            if current_ids != known_ids {
                slog(&poll_app, &poll_log, "info", format!("Room list changed: {} -> {} rooms", known_ids.len(), current_ids.len()));
                known_ids = current_ids;
                let _ = poll_app.emit("rooms_changed", "");
            }
        }
    });

    // Verification request handler
    let verify_app = app.clone();
    let verify_client = client.clone();
    let verify_log = log.clone();

    let sync_log = log.clone();
    let sync_app = app.clone();

    let sync_task = tokio::spawn(async move {
        // Handle incoming verification requests
        let va = verify_app.clone();
        let vc = verify_client.clone();
        let vl = verify_log.clone();
        verify_client.add_event_handler(
            move |event: matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent| {
                let app = va.clone();
                let client = vc.clone();
                let log = vl.clone();
                async move {
                    let user_id = event.sender;
                    let flow_id = event.content.transaction_id.to_string();
                    slog(&app, &log, "info", format!("Verification request from {} (flow={})", user_id, flow_id));
                    if let Some(request) = client
                        .encryption()
                        .get_verification_request(&user_id, &flow_id)
                        .await
                    {
                        let payload = VerificationEvent {
                            flow_id,
                            user_id: user_id.to_string(),
                            is_self_verification: request.is_self_verification(),
                        };
                        let _ = app.emit("verification_request", &payload);
                    }
                }
            },
        );

        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent,
                  room: matrix_sdk::Room| {
                let app = app_handle.clone();
                async move {
                    if let Some(original) = event.as_original() {
                        if let matrix_sdk::ruma::events::room::message::MessageType::Text(text) =
                            &original.content.msgtype
                        {
                            let msg = Message {
                                room_id: room.room_id().to_string(),
                                event_id: event.event_id().to_string(),
                                sender: event.sender().to_string(),
                                sender_name: event.sender().localpart().to_string(),
                                body: text.body.clone(),
                                timestamp: event.origin_server_ts().as_secs().into(),
                                msg_type: "text".to_string(),
                            };
                            let _ = app.emit("new_message", &msg);
                        }
                    }
                }
            },
        );

        slog(&sync_app, &sync_log, "info", "Sync loop starting...".into());
        let settings = matrix_sdk::config::SyncSettings::default();
        match client.sync(settings).await {
            Ok(_) => slog_buf(&sync_log, "info", "Sync loop ended".into()),
            Err(e) => slog_buf(&sync_log, "error", format!("Sync loop error: {}", e)),
        }
    });

    // Store task handles so we can abort them on disconnect/logout
    {
        let mut tasks = state.sync_tasks.lock().unwrap();
        *tasks = vec![poll_task, sync_task];
    }

    Ok(())
}

#[tauri::command]
pub async fn upload_file(
    room_id: String,
    file_path: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("upload_file: {} to room {}", file_path, room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let data = std::fs::read(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let filename = std::path::Path::new(&file_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mime = mime_guess::from_path(&file_path).first_or_octet_stream();

    slog(&app, &log, "info", format!("Uploading {} ({} bytes, {})", filename, data.len(), mime));
    let response = client
        .media()
        .upload(&mime, data, None)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Upload failed: {}", e));
            format!("Upload failed: {}", e)
        })?;

    let content = matrix_sdk::ruma::events::room::message::RoomMessageEventContent::new(
        matrix_sdk::ruma::events::room::message::MessageType::File(
            matrix_sdk::ruma::events::room::message::FileMessageEventContent::plain(
                filename,
                response.content_uri,
            ),
        ),
    );
    room.send(content).await.map_err(|e| format!("Send failed: {}", e))?;

    slog(&app, &log, "info", "File sent OK".into());
    Ok(())
}

#[tauri::command]
pub async fn accept_verification(
    user_id: String,
    flow_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("Accepting verification from {} (flow={})", user_id, flow_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?.clone();
    drop(client_lock);

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let request = client
        .encryption()
        .get_verification_request(&user_id, &flow_id)
        .await
        .ok_or("Verification request not found")?;

    request
        .accept()
        .await
        .map_err(|e| format!("Failed to accept: {}", e))?;

    slog(&app, &log, "info", "Starting SAS verification...".into());
    let sas = request
        .start_sas()
        .await
        .map_err(|e| format!("Failed to start SAS: {}", e))?
        .ok_or("Could not start SAS verification")?;

    // Poll for emoji availability (SAS handshake takes a moment)
    let uid = user_id.to_owned();
    let fid = flow_id.clone();
    let poll_log = log.clone();
    tokio::spawn(async move {
        for i in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if let Some(emojis) = sas.emoji() {
                slog(&app, &poll_log, "info", format!("SAS emojis ready after {}ms", (i + 1) * 500));
                let payload = VerificationEmojisEvent {
                    flow_id: fid,
                    user_id: uid.to_string(),
                    emojis: emojis
                        .iter()
                        .map(|e| VerificationEmoji {
                            symbol: e.symbol.to_string(),
                            description: e.description.to_string(),
                        })
                        .collect(),
                };
                let _ = app.emit("verification_emojis", &payload);
                return;
            }
        }
        slog(&app, &poll_log, "warn", "SAS emoji timeout after 15s".into());
        let _ = app.emit(
            "verification_cancelled",
            serde_json::json!({ "flow_id": fid, "reason": "Timed out waiting for emojis" }),
        );
    });

    Ok(())
}

#[tauri::command]
pub async fn confirm_verification(
    user_id: String,
    flow_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("Confirming verification (flow={})", flow_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let verification = client
        .encryption()
        .get_verification(&user_id, &flow_id)
        .await
        .ok_or("Verification not found")?;

    let sas = verification.sas().ok_or("Not a SAS verification")?;
    sas.confirm()
        .await
        .map_err(|e| format!("Failed to confirm: {}", e))?;

    slog(&app, &log, "info", "Verification confirmed!".into());
    let _ = app.emit(
        "verification_done",
        serde_json::json!({ "flow_id": flow_id, "user_id": user_id.to_string() }),
    );

    Ok(())
}

#[tauri::command]
pub async fn cancel_verification(
    user_id: String,
    flow_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("Cancelling verification (flow={})", flow_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    if let Some(request) = client
        .encryption()
        .get_verification_request(&user_id, &flow_id)
        .await
    {
        request
            .cancel()
            .await
            .map_err(|e| format!("Failed to cancel: {}", e))?;
    } else if let Some(verification) = client
        .encryption()
        .get_verification(&user_id, &flow_id)
        .await
    {
        if let Some(sas) = verification.sas() {
            sas.mismatch()
                .await
                .map_err(|e| format!("Failed to cancel: {}", e))?;
        }
    }

    let _ = app.emit(
        "verification_cancelled",
        serde_json::json!({ "flow_id": flow_id }),
    );

    Ok(())
}
