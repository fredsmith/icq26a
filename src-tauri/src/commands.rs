use crate::matrix_client::{Buddy, LoginCredentials, MatrixState, Message, Room};
use matrix_sdk::{Client, ServerName};
use tauri::{Emitter, State};

#[tauri::command]
pub async fn matrix_login(
    credentials: LoginCredentials,
    state: State<'_, MatrixState>,
) -> Result<String, String> {
    let server_name = ServerName::parse(&credentials.homeserver.replace("https://", ""))
        .map_err(|e| format!("Invalid homeserver: {}", e))?;

    let client = Client::builder()
        .server_name(&server_name)
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let response = client
        .matrix_auth()
        .login_username(&credentials.username, &credentials.password)
        .send()
        .await
        .map_err(|e| format!("Login failed: {}", e))?;

    let user_id = response.user_id.to_string();

    let mut client_lock = state.client.lock().await;
    *client_lock = Some(client);

    Ok(user_id)
}

#[tauri::command]
pub async fn matrix_logout(state: State<'_, MatrixState>) -> Result<(), String> {
    let mut client_lock = state.client.lock().await;
    *client_lock = None;
    Ok(())
}

#[tauri::command]
pub async fn get_buddy_list(state: State<'_, MatrixState>) -> Result<Vec<Buddy>, String> {
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    client
        .sync_once(Default::default())
        .await
        .map_err(|e| format!("Sync failed: {}", e))?;

    let mut buddies = Vec::new();
    for room in client.joined_rooms() {
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
                    buddies.push(Buddy {
                        user_id: user_id.clone(),
                        display_name: member.display_name().unwrap_or(&user_id).to_string(),
                        avatar_url: member.avatar_url().map(|u| u.to_string()),
                        presence: "offline".to_string(),
                    });
                }
            }
        }
    }
    Ok(buddies)
}

#[tauri::command]
pub async fn get_rooms(state: State<'_, MatrixState>) -> Result<Vec<Room>, String> {
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
    Ok(rooms)
}

#[tauri::command]
pub async fn get_room_messages(
    room_id: String,
    limit: u64,
    state: State<'_, MatrixState>,
) -> Result<Vec<Message>, String> {
    let _ = limit;
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let options = matrix_sdk::room::MessagesOptions::backward();
    let messages_response = room
        .messages(options)
        .await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

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
    Ok(messages)
}

#[tauri::command]
pub async fn send_message(
    room_id: String,
    body: String,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let content =
        matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(&body);
    room.send(content)
        .await
        .map_err(|e| format!("Send failed: {}", e))?;

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
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?.clone();
    drop(client_lock);

    let app_handle = app.clone();

    tokio::spawn(async move {
        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent,
                  _room: matrix_sdk::Room| {
                let app = app_handle.clone();
                async move {
                    if let Some(original) = event.as_original() {
                        if let matrix_sdk::ruma::events::room::message::MessageType::Text(text) =
                            &original.content.msgtype
                        {
                            let msg = Message {
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

        let settings = matrix_sdk::config::SyncSettings::default();
        let _ = client.sync(settings).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn upload_file(
    room_id: String,
    file_path: String,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
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

    let response = client
        .media()
        .upload(&mime, data, None)
        .await
        .map_err(|e| format!("Upload failed: {}", e))?;

    let content = matrix_sdk::ruma::events::room::message::RoomMessageEventContent::new(
        matrix_sdk::ruma::events::room::message::MessageType::File(
            matrix_sdk::ruma::events::room::message::FileMessageEventContent::plain(
                filename,
                response.content_uri,
            ),
        ),
    );
    room.send(content).await.map_err(|e| format!("Send failed: {}", e))?;

    Ok(())
}
