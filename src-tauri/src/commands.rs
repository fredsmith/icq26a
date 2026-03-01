use crate::matrix_client::{
    Buddy, InviteInfo, LogEntry, LoginCredentials, MatrixState, Message, MessageDeletedEvent,
    MessageEditEvent, MessagesPage, PersistedSession, ReactionEvent, Room, RoomProfile, ServerLog,
    SharedRoom, TypingEvent, UserProfile, VerificationEmoji, VerificationEmojisEvent,
    VerificationEvent,
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

/// Convert an mxc:// URL to an HTTP thumbnail URL via the homeserver's media API.
fn mxc_to_http(homeserver: &str, mxc_url: &str) -> Option<String> {
    let path = mxc_url.strip_prefix("mxc://")?;
    let (server_name, media_id) = path.split_once('/')?;
    Some(format!(
        "{}/_matrix/media/v3/thumbnail/{}/{}?width=96&height=96&method=crop",
        homeserver.trim_end_matches('/'),
        server_name,
        media_id,
    ))
}

/// Extract the mxc:// URL string from a MediaSource (plain only; encrypted media not supported).
fn media_source_to_mxc(source: &matrix_sdk::ruma::events::room::MediaSource) -> Option<String> {
    match source {
        matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
        _ => None,
    }
}

/// Parse Matrix reply fallback from body. Returns (sender_id, quoted_text).
fn extract_reply_fallback(body: &str) -> Option<(String, String)> {
    if !body.starts_with("> <") {
        return None;
    }
    let mut lines = body.lines();
    let first_line = lines.next()?;
    let after_bracket = first_line.strip_prefix("> <")?;
    let end_bracket = after_bracket.find('>')?;
    let sender = after_bracket[..end_bracket].to_string();
    let first_quote = after_bracket[end_bracket + 1..].trim_start().to_string();

    let mut quoted_lines = vec![first_quote];
    for line in lines {
        if let Some(stripped) = line.strip_prefix("> ") {
            quoted_lines.push(stripped.to_string());
        } else {
            break;
        }
    }

    let quoted_body = quoted_lines.join("\n").trim().to_string();
    Some((sender, quoted_body))
}

/// Remove the Matrix reply fallback prefix from a body, returning the actual reply text.
fn strip_reply_fallback(body: &str) -> String {
    if !body.starts_with("> ") {
        return body.to_string();
    }
    let mut lines = body.lines().peekable();
    while let Some(line) = lines.peek() {
        if line.starts_with("> ") || *line == ">" {
            lines.next();
        } else {
            break;
        }
    }
    // Skip blank line after fallback
    if let Some(line) = lines.peek() {
        if line.is_empty() {
            lines.next();
        }
    }
    lines.collect::<Vec<_>>().join("\n")
}

/// Fetch an mxc:// avatar as a base64 data URL using authenticated media endpoints.
/// Tries the authenticated endpoint first (_matrix/client/v1/media), then falls back
/// to the unauthenticated one (_matrix/media/v3).
async fn fetch_avatar_data_url(client: &Client, mxc_url: &str) -> Option<String> {
    let path = mxc_url.strip_prefix("mxc://")?;
    let (server_name, media_id) = path.split_once('/')?;
    let hs = client.homeserver().to_string();
    let hs = hs.trim_end_matches('/');

    let access_token = client.access_token()?;

    let urls = [
        format!("{}/_matrix/client/v1/media/thumbnail/{}/{}?width=96&height=96&method=crop", hs, server_name, media_id),
        format!("{}/_matrix/media/v3/thumbnail/{}/{}?width=96&height=96&method=crop", hs, server_name, media_id),
    ];

    let http = reqwest::Client::new();
    for url in &urls {
        let resp = http.get(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .ok()?;
        if resp.status().is_success() {
            let bytes = resp.bytes().await.ok()?;
            if bytes.is_empty() {
                continue;
            }
            let content_type = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                "image/png"
            } else if bytes.starts_with(&[0xFF, 0xD8]) {
                "image/jpeg"
            } else if bytes.starts_with(b"GIF") {
                "image/gif"
            } else {
                "image/png"
            };
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Some(format!("data:{};base64,{}", content_type, b64));
        }
    }
    None
}

/// Fetch the explicit room name from the server via the Matrix state API.
/// Returns None if the room has no m.room.name event or on any error.
async fn fetch_room_name_from_server(client: &Client, room_id: &str) -> Option<String> {
    let hs = client.homeserver().to_string();
    let hs = hs.trim_end_matches('/');
    let access_token = client.access_token()?;

    let url = format!(
        "{}/_matrix/client/v3/rooms/{}/state/m.room.name",
        hs, room_id
    );

    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let name = json.get("name")?.as_str()?;
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// Get the best available name for a room, with server API fallback.
/// For non-DM rooms, falls back to querying the server if the local SDK store
/// doesn't have the m.room.name state event cached.
async fn resolve_room_name(client: &Client, room: &matrix_sdk::Room, is_direct: bool) -> String {
    // 1. Check explicit m.room.name from local state
    if let Some(name) = room.name().filter(|n| !n.is_empty()) {
        return name;
    }

    // 2. Check canonical alias
    if let Some(alias) = room.canonical_alias() {
        return alias.to_string();
    }

    // 3. For non-DM rooms, try fetching name from server
    if !is_direct {
        if let Some(name) = fetch_room_name_from_server(client, room.room_id().as_str()).await {
            return name;
        }
    }

    // 4. Fall back to SDK computed display name
    room.display_name()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "Unknown".to_string())
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
pub async fn matrix_register(
    credentials: LoginCredentials,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<String, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("Registering as {} on {}", credentials.username, credentials.homeserver));

    let server_name = ServerName::parse(&credentials.homeserver.replace("https://", ""))
        .map_err(|e| {
            slog(&app, &log, "error", format!("Invalid homeserver: {}", e));
            format!("Invalid homeserver: {}", e)
        })?;

    let data_path = crate::matrix_client::data_dir()?;
    if data_path.exists() {
        slog(&app, &log, "info", "Clearing old sqlite store for fresh registration...".into());
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

    let homeserver_url = client.homeserver().to_string();
    slog(&app, &log, "info", format!("Resolved homeserver: {}", homeserver_url));
    slog(&app, &log, "info", "Sending registration request...".into());

    // Use raw HTTP for the UIAA handshake — the SDK doesn't expose
    // session/flows from UIAA errors, so we need direct control.
    let register_url = format!("{}/_matrix/client/v3/register", homeserver_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "username": credentials.username,
        "password": credentials.password,
        "kind": "user",
        "initial_device_display_name": "icq26a"
    });

    let http = reqwest::Client::new();

    // Step 1: Initial request — expect 401 with UIAA flows + session
    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        http.post(&register_url).json(&body).send(),
    )
        .await
        .map_err(|_| {
            slog(&app, &log, "error", "Registration request timed out after 30s".into());
            "Registration timed out".to_string()
        })?
        .map_err(|e| {
            slog(&app, &log, "error", format!("Registration failed: {}", e));
            format!("Registration failed: {}", e)
        })?;

    let status = resp.status();
    let resp_body: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // If registration succeeded without UIAA (rare — some servers allow it)
    if status.is_success() {
        slog(&app, &log, "info", "Registration succeeded without UIAA".into());
        return finish_registration(client, &resp_body, &app, &log, &state).await;
    }

    // Not 401 → real error, not UIAA
    if status.as_u16() != 401 {
        let error_msg = resp_body["error"].as_str().unwrap_or("Registration failed");
        slog(&app, &log, "error", format!("Registration failed: {}", error_msg));
        return Err(format!("Registration failed: {}", error_msg));
    }

    // 401 UIAA — check if any flow is just m.login.dummy
    slog(&app, &log, "info", format!("UIAA response, flows: {}", resp_body["flows"]));

    let session = resp_body["session"].as_str();
    let has_dummy_flow = resp_body["flows"].as_array().map_or(false, |flows| {
        flows.iter().any(|f| {
            f["stages"].as_array().map_or(false, |stages| {
                stages.len() == 1 && stages[0].as_str() == Some("m.login.dummy")
            })
        })
    });

    if !has_dummy_flow {
        slog(&app, &log, "warn", "Server requires auth flows we can't handle".into());
        return Err(format!(
            "This server requires additional verification steps (e.g. email or captcha). Please register at {} in your browser.",
            credentials.homeserver
        ));
    }

    // Step 2: Retry with m.login.dummy auth + session
    slog(&app, &log, "info", "Retrying registration with m.login.dummy auth...".into());

    let mut retry_body = body.clone();
    let mut auth = serde_json::json!({"type": "m.login.dummy"});
    if let Some(s) = session {
        auth["session"] = serde_json::Value::String(s.to_string());
    }
    retry_body["auth"] = auth;

    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        http.post(&register_url).json(&retry_body).send(),
    )
        .await
        .map_err(|_| {
            slog(&app, &log, "error", "Registration retry timed out".into());
            "Registration timed out".to_string()
        })?
        .map_err(|e| {
            slog(&app, &log, "error", format!("Registration retry failed: {}", e));
            format!("Registration failed: {}", e)
        })?;

    if !resp.status().is_success() {
        let error_body: serde_json::Value = resp.json().await
            .map_err(|e| format!("Failed to parse error: {}", e))?;
        let error_msg = error_body["error"].as_str().unwrap_or("Registration failed");
        slog(&app, &log, "error", format!("Registration failed after UIAA: {}", error_msg));
        return Err(format!("Registration failed: {}", error_msg));
    }

    let resp_body: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    slog(&app, &log, "info", "Registration succeeded after UIAA dummy auth".into());
    finish_registration(client, &resp_body, &app, &log, &state).await
}

/// Post-registration: restore session on SDK client, save to disk, store in state.
async fn finish_registration(
    client: Client,
    resp: &serde_json::Value,
    app: &tauri::AppHandle,
    log: &std::sync::Arc<ServerLog>,
    state: &State<'_, MatrixState>,
) -> Result<String, String> {
    let user_id = resp["user_id"].as_str()
        .ok_or("Registration response missing user_id")?;
    let access_token = resp["access_token"].as_str()
        .ok_or("Registration response missing access_token")?;
    let device_id = resp["device_id"].as_str()
        .ok_or("Registration response missing device_id")?;
    let refresh_token = resp["refresh_token"].as_str().map(|s| s.to_string());

    slog(app, log, "info", format!("Registered user_id={}, device_id={}", user_id, device_id));

    // Restore session on SDK client so it's authenticated for sync, etc.
    let session = matrix_sdk::authentication::matrix::MatrixSession {
        meta: matrix_sdk::SessionMeta {
            user_id: matrix_sdk::ruma::UserId::parse(user_id)
                .map_err(|e| format!("Invalid user_id: {}", e))?,
            device_id: device_id.into(),
        },
        tokens: matrix_sdk::SessionTokens {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.clone(),
        },
    };
    client.restore_session(session).await.map_err(|e| {
        slog(app, log, "error", format!("Failed to restore session: {}", e));
        format!("Registration succeeded but session setup failed: {}", e)
    })?;

    // Save session for restore on next launch
    let session_data = PersistedSession {
        homeserver_url: client.homeserver().to_string(),
        user_id: user_id.to_string(),
        device_id: device_id.to_string(),
        access_token: access_token.to_string(),
        refresh_token,
    };
    let session_path = crate::matrix_client::session_file_path()?;
    let json = serde_json::to_string_pretty(&session_data)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    std::fs::write(&session_path, json)
        .map_err(|e| format!("Failed to write session: {}", e))?;
    slog(app, log, "info", "Session saved to disk".into());

    let mut client_lock = state.client.lock().await;
    *client_lock = Some(client);

    Ok(user_id.to_string())
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
pub async fn get_user_profile(
    user_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<UserProfile, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("get_user_profile: {}", user_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let parsed_user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|e| format!("Invalid user ID: {}", e))?;

    // Fetch profile (display name + avatar)
    let mut display_name = user_id.clone();
    let mut avatar_url: Option<String> = None;
    let mut mxc_avatar: Option<String> = None;
    {
        use matrix_sdk::ruma::api::client::profile::get_profile;
        use matrix_sdk::ruma::api::client::profile::{AvatarUrl, DisplayName};
        let request = get_profile::v3::Request::new(parsed_user_id.clone());
        match tokio::time::timeout(std::time::Duration::from_secs(5), client.send(request)).await {
            Ok(Ok(response)) => {
                if let Ok(Some(name)) = response.get_static::<DisplayName>() {
                    display_name = name;
                }
                if let Ok(Some(url)) = response.get_static::<AvatarUrl>() {
                    mxc_avatar = Some(url.to_string());
                }
            }
            Ok(Err(e)) => {
                slog(&app, &log, "warn", format!("Profile fetch failed: {}", e));
            }
            Err(_) => {
                slog(&app, &log, "warn", "Profile fetch timed out".into());
            }
        }
    }

    // Download avatar via authenticated media endpoint → base64 data URL
    if let Some(mxc) = &mxc_avatar {
        avatar_url = fetch_avatar_data_url(client, mxc).await;
    }

    // Fetch presence + last_active_ago
    let mut presence = "unknown".to_string();
    let mut last_seen_ago: Option<u64> = None;
    {
        use matrix_sdk::ruma::api::client::presence::get_presence;
        let request = get_presence::v3::Request::new(parsed_user_id.clone());
        match tokio::time::timeout(std::time::Duration::from_secs(3), client.send(request)).await {
            Ok(Ok(response)) => {
                let is_stale = response.currently_active.is_none()
                    && response.last_active_ago.is_none()
                    && response.presence == matrix_sdk::ruma::presence::PresenceState::Offline;
                if !is_stale {
                    presence = match response.presence {
                        matrix_sdk::ruma::presence::PresenceState::Online => "online",
                        matrix_sdk::ruma::presence::PresenceState::Unavailable => "away",
                        _ => "offline",
                    }
                    .to_string();
                    last_seen_ago = response.last_active_ago.map(|d| d.as_secs());
                }
            }
            _ => {}
        }
    }

    // Scan joined rooms for shared membership
    let mut shared_rooms = Vec::new();
    for room in client.joined_rooms() {
        let members = room
            .members(matrix_sdk::RoomMemberships::ACTIVE)
            .await
            .unwrap_or_default();
        let has_user = members.iter().any(|m| m.user_id() == parsed_user_id);
        if has_user {
            let is_direct = room.is_direct().await.unwrap_or(false);
            let name = resolve_room_name(client, &room, is_direct).await;
            shared_rooms.push(SharedRoom {
                room_id: room.room_id().to_string(),
                name,
            });
        }
    }

    slog(
        &app,
        &log,
        "info",
        format!(
            "get_user_profile: {} — presence={}, shared_rooms={}",
            user_id,
            presence,
            shared_rooms.len()
        ),
    );

    Ok(UserProfile {
        user_id,
        display_name,
        avatar_url,
        presence,
        last_seen_ago,
        shared_rooms,
    })
}

#[tauri::command]
pub async fn get_room_info(
    room_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<RoomProfile, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("get_room_info: {}", room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id_parsed = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id_parsed).ok_or("Room not found")?;

    let is_direct = room.is_direct().await.unwrap_or(false);
    let name = resolve_room_name(client, &room, is_direct).await;

    let topic = room.topic();

    let members = room
        .members(matrix_sdk::RoomMemberships::ACTIVE)
        .await
        .unwrap_or_default();
    let member_count = members.len() as u64;

    Ok(RoomProfile {
        room_id,
        name,
        topic,
        is_direct,
        member_count,
    })
}

#[tauri::command]
pub async fn create_dm_room(
    user_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Room, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("create_dm_room: {}", user_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let parsed_user_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|e| format!("Invalid user ID: {}", e))?;

    use matrix_sdk::ruma::api::client::room::create_room::v3::Request as CreateRoomRequest;
    let mut request = CreateRoomRequest::new();
    request.invite = vec![parsed_user_id];
    request.is_direct = true;
    request.preset = Some(matrix_sdk::ruma::api::client::room::create_room::v3::RoomPreset::TrustedPrivateChat);

    let response = client
        .create_room(request)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Failed to create DM room: {}", e));
            format!("Failed to create room: {}", e)
        })?;

    let room_id = response.room_id().to_string();
    slog(&app, &log, "info", format!("DM room created: {}", room_id));

    // Get the room to fetch its display name
    let name = if let Some(room) = client.get_room(response.room_id()) {
        room.display_name()
            .await
            .map(|n| n.to_string())
            .unwrap_or_else(|_| user_id.clone())
    } else {
        user_id.clone()
    };

    Ok(Room {
        room_id,
        name,
        is_direct: true,
        last_message: None,
        unread_count: 0,
    })
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
    let mut seen_user_ids = std::collections::HashSet::new();
    let mut presence_supported = true;
    let my_user_id = client.user_id().map(|u| u.to_string()).unwrap_or_default();

    for room in joined {
        if room.is_direct().await.unwrap_or(false) {
            let members = room
                .members(matrix_sdk::RoomMemberships::ACTIVE)
                .await
                .unwrap_or_default();
            for member in members {
                let user_id = member.user_id().to_string();
                if user_id != my_user_id && seen_user_ids.insert(user_id.clone()) {
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
                    let hs = client.homeserver().to_string();
                    buddies.push(Buddy {
                        user_id: user_id.clone(),
                        display_name: member.display_name().unwrap_or(&user_id).to_string(),
                        avatar_url: member.avatar_url().and_then(|u| mxc_to_http(&hs, &u.to_string())),
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

    let hs = client.homeserver().to_string();
    let buddies: Vec<Buddy> = members
        .iter()
        .map(|member| {
            let user_id = member.user_id().to_string();
            Buddy {
                display_name: member.display_name().unwrap_or(&user_id).to_string(),
                avatar_url: member.avatar_url().and_then(|u| mxc_to_http(&hs, &u.to_string())),
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
        let is_direct = room.is_direct().await.unwrap_or(false);
        rooms.push(Room {
            room_id: room.room_id().to_string(),
            name: resolve_room_name(client, &room, is_direct).await,
            is_direct,
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
    from: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<MessagesPage, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("get_room_messages: {} (from={:?})", room_id, from));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    slog(&app, &log, "info", "Fetching messages from server...".into());
    let mut options = matrix_sdk::room::MessagesOptions::backward();
    if let Some(ref token) = from {
        options.from = Some(token.clone());
    }
    if let Some(l) = matrix_sdk::ruma::UInt::new(limit) {
        options.limit = l;
    }
    let messages_response = with_heartbeat(
        &app, &log, "messages",
        room.messages(options),
    )
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Failed to get messages: {}", e));
            format!("Failed to get messages: {}", e)
        })?;

    let end_token = messages_response.end;

    let mut messages = Vec::new();
    let mut edits: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for event in messages_response.chunk {
        if let Ok(timeline_event) = event.raw().deserialize() {
            if let matrix_sdk::ruma::events::AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg),
            ) = timeline_event
            {
                let Some(original) = msg.as_original() else {
                    continue;
                };

                // Handle edits: collect replacement content, skip the edit event itself
                if let Some(matrix_sdk::ruma::events::room::message::Relation::Replacement(replacement)) = &original.content.relates_to {
                    let new_body = match &replacement.new_content.msgtype {
                        matrix_sdk::ruma::events::room::message::MessageType::Text(text) => text.body.clone(),
                        matrix_sdk::ruma::events::room::message::MessageType::Notice(notice) => notice.body.clone(),
                        matrix_sdk::ruma::events::room::message::MessageType::Emote(emote) => format!("* {}", emote.body),
                        _ => String::new(),
                    };
                    if !new_body.is_empty() {
                        edits.insert(replacement.event_id.to_string(), new_body);
                    }
                    continue;
                }

                // Extract reply relation (Reply or Thread)
                let mut in_reply_to: Option<String> = None;
                let mut reply_sender_name: Option<String> = None;
                let mut reply_body_text: Option<String> = None;

                if let Some(relation) = &original.content.relates_to {
                    match relation {
                        matrix_sdk::ruma::events::room::message::Relation::Reply { in_reply_to: irt } => {
                            in_reply_to = Some(irt.event_id.to_string());
                        }
                        matrix_sdk::ruma::events::room::message::Relation::Thread(thread) => {
                            if let Some(irt) = &thread.in_reply_to {
                                in_reply_to = Some(irt.event_id.to_string());
                            }
                        }
                        _ => {}
                    }
                }

                // Fallback: parse raw JSON for m.relates_to.m.in_reply_to.event_id
                // Some servers (e.g. continuwuity) may include fields that ruma
                // doesn't recognize, causing typed deserialization to miss the reply.
                if in_reply_to.is_none() {
                    if let Ok(raw_json) = serde_json::from_str::<serde_json::Value>(event.raw().json().get()) {
                        if let Some(eid) = raw_json.pointer("/content/m.relates_to/m.in_reply_to/event_id")
                            .and_then(|v| v.as_str())
                        {
                            in_reply_to = Some(eid.to_string());
                        }
                    }
                }

                let (mut body, msg_type, media_url, filename) = match &original.content.msgtype {
                    matrix_sdk::ruma::events::room::message::MessageType::Text(text) => {
                        (text.body.clone(), "text".to_string(), None, None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Notice(notice) => {
                        (notice.body.clone(), "text".to_string(), None, None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Emote(emote) => {
                        (format!("* {}", emote.body), "text".to_string(), None, None)
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Image(img) => {
                        (img.body.clone(), "image".to_string(), media_source_to_mxc(&img.source), Some(img.body.clone()))
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::File(file) => {
                        let fname = file.filename.clone().unwrap_or_else(|| file.body.clone());
                        (file.body.clone(), "file".to_string(), media_source_to_mxc(&file.source), Some(fname))
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Audio(audio) => {
                        (audio.body.clone(), "audio".to_string(), media_source_to_mxc(&audio.source), Some(audio.body.clone()))
                    }
                    matrix_sdk::ruma::events::room::message::MessageType::Video(video) => {
                        (video.body.clone(), "video".to_string(), media_source_to_mxc(&video.source), Some(video.body.clone()))
                    }
                    _ => (String::new(), "unknown".to_string(), None, None),
                };

                // For text replies, parse and strip fallback
                if msg_type == "text" && body.starts_with("> <") {
                    if in_reply_to.is_none() {
                        // Body has reply fallback but typed relation wasn't parsed —
                        // use body text as the source of reply info
                        in_reply_to = Some("fallback".to_string());
                    }
                    if let Some((sender, quoted)) = extract_reply_fallback(&body) {
                        reply_sender_name = Some(sender);
                        reply_body_text = Some(quoted);
                    }
                    body = strip_reply_fallback(&body);
                }

                messages.push(Message {
                    room_id: room_id.to_string(),
                    event_id: msg.event_id().to_string(),
                    sender: msg.sender().to_string(),
                    sender_name: msg.sender().localpart().to_string(),
                    body,
                    timestamp: msg.origin_server_ts().as_secs().into(),
                    msg_type,
                    media_url,
                    filename,
                    in_reply_to,
                    reply_sender_name,
                    reply_body: reply_body_text,
                });
            }
        }
    }

    // Apply edits to original messages
    for msg in &mut messages {
        if let Some(new_body) = edits.get(&msg.event_id) {
            msg.body = new_body.clone();
        }
    }

    messages.reverse();
    slog(&app, &log, "info", format!("get_room_messages: returning {} messages ({} edits applied)", messages.len(), edits.len()));
    Ok(MessagesPage { messages, end_token })
}

#[tauri::command]
pub async fn send_message(
    room_id: String,
    body: String,
    in_reply_to_event_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("send_message: room={}, len={}, reply={:?}", room_id, body.len(), in_reply_to_event_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let mut content =
        matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(&body);

    if let Some(reply_id) = in_reply_to_event_id {
        let event_id = matrix_sdk::ruma::OwnedEventId::try_from(reply_id.as_str())
            .map_err(|e| format!("Invalid event ID: {}", e))?;
        content.relates_to = Some(
            matrix_sdk::ruma::events::room::message::Relation::Reply {
                in_reply_to: matrix_sdk::ruma::events::relation::InReplyTo::new(event_id),
            }
        );
    }

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
pub async fn edit_message(
    room_id: String,
    event_id: String,
    new_body: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("edit_message: room={}, event={}", room_id, event_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id).ok_or("Room not found")?;

    // Build the edit event as raw JSON — the SDK's Replacement type is non_exhaustive/private
    let raw_content = serde_json::json!({
        "msgtype": "m.text",
        "body": format!("* {}", new_body),
        "m.new_content": {
            "msgtype": "m.text",
            "body": new_body,
        },
        "m.relates_to": {
            "rel_type": "m.replace",
            "event_id": event_id,
        }
    });
    let content: matrix_sdk::ruma::events::room::message::RoomMessageEventContent =
        serde_json::from_value(raw_content)
            .map_err(|e| format!("Failed to build edit content: {}", e))?;

    room.send(content)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Edit failed: {}", e));
            format!("Edit failed: {}", e)
        })?;

    slog(&app, &log, "info", "Message edited OK".into());
    Ok(())
}

#[tauri::command]
pub async fn delete_message(
    room_id: String,
    event_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("delete_message: room={}, event={}", room_id, event_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let event_id = matrix_sdk::ruma::OwnedEventId::try_from(event_id.as_str())
        .map_err(|e| format!("Invalid event ID: {}", e))?;

    room.redact(&event_id, None, None)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Delete failed: {}", e));
            format!("Delete failed: {}", e)
        })?;

    slog(&app, &log, "info", "Message deleted OK".into());
    Ok(())
}

#[tauri::command]
pub async fn send_reaction(
    room_id: String,
    event_id: String,
    reaction_key: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("send_reaction: room={}, event={}, key={}", room_id, event_id, reaction_key));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id).ok_or("Room not found")?;

    let event_id = matrix_sdk::ruma::OwnedEventId::try_from(event_id.as_str())
        .map_err(|e| format!("Invalid event ID: {}", e))?;

    let content = matrix_sdk::ruma::events::reaction::ReactionEventContent::new(
        matrix_sdk::ruma::events::relation::Annotation::new(event_id, reaction_key),
    );

    room.send(content)
        .await
        .map_err(|e| {
            slog(&app, &log, "error", format!("Reaction failed: {}", e));
            format!("Reaction failed: {}", e)
        })?;

    slog(&app, &log, "info", "Reaction sent OK".into());
    Ok(())
}

#[tauri::command]
pub async fn set_presence(
    status: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    // Map ICQ status names to Matrix presence states
    let presence = match status.as_str() {
        "online" | "free_for_chat" => matrix_sdk::ruma::presence::PresenceState::Online,
        "away" | "na" => matrix_sdk::ruma::presence::PresenceState::Unavailable,
        "occupied" | "dnd" => matrix_sdk::ruma::presence::PresenceState::Unavailable,
        "invisible" | "offline" => matrix_sdk::ruma::presence::PresenceState::Offline,
        _ => matrix_sdk::ruma::presence::PresenceState::Online,
    };

    use matrix_sdk::ruma::api::client::presence::set_presence;
    let user_id = client.user_id().ok_or("No user ID")?.to_owned();
    let mut request = set_presence::v3::Request::new(user_id, presence.clone());
    // Set a status message for non-standard ICQ statuses
    match status.as_str() {
        "dnd" => request.status_msg = Some("Do Not Disturb".to_string()),
        "occupied" => request.status_msg = Some("Occupied".to_string()),
        "na" => request.status_msg = Some("Not Available".to_string()),
        "free_for_chat" => request.status_msg = Some("Free for Chat".to_string()),
        _ => {}
    }

    match client.send(request).await {
        Ok(_) => {
            slog(&app, &log, "info", format!("Presence set to {} (matrix: {:?})", status, presence));
        }
        Err(e) => {
            // Some servers don't support presence — log but don't fail
            slog(&app, &log, "warn", format!("Failed to set presence: {}", e));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn send_typing(
    room_id: String,
    typing: bool,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id).ok_or("Room not found")?;

    room.typing_notice(typing)
        .await
        .map_err(|e| format!("Typing notice failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn mark_as_read(
    room_id: String,
    event_id: String,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id_parsed = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id_parsed).ok_or("Room not found")?;

    let event_id_parsed = matrix_sdk::ruma::OwnedEventId::try_from(event_id.as_str())
        .map_err(|e| format!("Invalid event ID: {}", e))?;

    room.send_single_receipt(
        matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType::Read,
        matrix_sdk::ruma::events::receipt::ReceiptThread::Unthreaded,
        event_id_parsed,
    )
    .await
    .map_err(|e| format!("Read receipt failed: {}", e))?;

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

        // Typing event handler
        let typing_app = app_handle.clone();
        let typing_client = client.clone();
        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::SyncEphemeralRoomEvent<matrix_sdk::ruma::events::typing::TypingEventContent>,
                  room: matrix_sdk::Room| {
                let app = typing_app.clone();
                let cl = typing_client.clone();
                async move {
                    let my_id_str = cl.user_id().map(|u| u.to_string());
                    let mut display_names = Vec::new();
                    let mut user_ids = Vec::new();

                    for uid in &event.content.user_ids {
                        if my_id_str.as_deref() == Some(uid.as_str()) {
                            continue;
                        }
                        user_ids.push(uid.to_string());
                        let name = match room.get_member_no_sync(uid).await {
                            Ok(Some(member)) => member.display_name().unwrap_or(uid.localpart()).to_string(),
                            _ => uid.localpart().to_string(),
                        };
                        display_names.push(name);
                    }

                    let payload = TypingEvent {
                        room_id: room.room_id().to_string(),
                        user_ids,
                        display_names,
                    };
                    let _ = app.emit("typing", &payload);
                }
            },
        );

        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent,
                  room: matrix_sdk::Room| {
                let app = app_handle.clone();
                async move {
                    if let Some(original) = event.as_original() {
                        // Handle edits: emit message_edited event
                        if let Some(matrix_sdk::ruma::events::room::message::Relation::Replacement(replacement)) = &original.content.relates_to {
                            let new_body = match &replacement.new_content.msgtype {
                                matrix_sdk::ruma::events::room::message::MessageType::Text(text) => text.body.clone(),
                                matrix_sdk::ruma::events::room::message::MessageType::Notice(notice) => notice.body.clone(),
                                matrix_sdk::ruma::events::room::message::MessageType::Emote(emote) => format!("* {}", emote.body),
                                _ => return,
                            };
                            let edit = MessageEditEvent {
                                room_id: room.room_id().to_string(),
                                original_event_id: replacement.event_id.to_string(),
                                new_body,
                                sender: event.sender().to_string(),
                                sender_name: event.sender().localpart().to_string(),
                            };
                            let _ = app.emit("message_edited", &edit);
                            return;
                        }

                        // Extract reply relation (Reply or Thread)
                        let mut in_reply_to: Option<String> = None;
                        let mut reply_sender_name: Option<String> = None;
                        let mut reply_body_text: Option<String> = None;

                        if let Some(relation) = &original.content.relates_to {
                            match relation {
                                matrix_sdk::ruma::events::room::message::Relation::Reply { in_reply_to: irt } => {
                                    in_reply_to = Some(irt.event_id.to_string());
                                }
                                matrix_sdk::ruma::events::room::message::Relation::Thread(thread) => {
                                    if let Some(irt) = &thread.in_reply_to {
                                        in_reply_to = Some(irt.event_id.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }

                        let (mut body, msg_type, media_url, filename) = match &original.content.msgtype {
                            matrix_sdk::ruma::events::room::message::MessageType::Text(text) => {
                                (text.body.clone(), "text".to_string(), None, None)
                            }
                            matrix_sdk::ruma::events::room::message::MessageType::Notice(notice) => {
                                (notice.body.clone(), "text".to_string(), None, None)
                            }
                            matrix_sdk::ruma::events::room::message::MessageType::Emote(emote) => {
                                (format!("* {}", emote.body), "text".to_string(), None, None)
                            }
                            matrix_sdk::ruma::events::room::message::MessageType::Image(img) => {
                                (img.body.clone(), "image".to_string(), media_source_to_mxc(&img.source), Some(img.body.clone()))
                            }
                            matrix_sdk::ruma::events::room::message::MessageType::File(file) => {
                                let fname = file.filename.clone().unwrap_or_else(|| file.body.clone());
                                (file.body.clone(), "file".to_string(), media_source_to_mxc(&file.source), Some(fname))
                            }
                            matrix_sdk::ruma::events::room::message::MessageType::Audio(audio) => {
                                (audio.body.clone(), "audio".to_string(), media_source_to_mxc(&audio.source), Some(audio.body.clone()))
                            }
                            matrix_sdk::ruma::events::room::message::MessageType::Video(video) => {
                                (video.body.clone(), "video".to_string(), media_source_to_mxc(&video.source), Some(video.body.clone()))
                            }
                            _ => return,
                        };

                        // For text replies, parse and strip fallback
                        if msg_type == "text" && body.starts_with("> <") {
                            if in_reply_to.is_none() {
                                // Body has reply fallback but typed relation wasn't parsed —
                                // use body text as the source of reply info
                                in_reply_to = Some("fallback".to_string());
                            }
                            if let Some((sender, quoted)) = extract_reply_fallback(&body) {
                                reply_sender_name = Some(sender);
                                reply_body_text = Some(quoted);
                            }
                            body = strip_reply_fallback(&body);
                        }

                        let msg = Message {
                            room_id: room.room_id().to_string(),
                            event_id: event.event_id().to_string(),
                            sender: event.sender().to_string(),
                            sender_name: event.sender().localpart().to_string(),
                            body,
                            timestamp: event.origin_server_ts().as_secs().into(),
                            msg_type,
                            media_url,
                            filename,
                            in_reply_to,
                            reply_sender_name,
                            reply_body: reply_body_text,
                        };
                        let _ = app.emit("new_message", &msg);
                    }
                }
            },
        );

        // Redaction event handler (message deletion)
        let redact_app = sync_app.clone();
        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::room::redaction::SyncRoomRedactionEvent,
                  room: matrix_sdk::Room| {
                let app = redact_app.clone();
                async move {
                    if let Some(original) = event.as_original() {
                        let payload = MessageDeletedEvent {
                            room_id: room.room_id().to_string(),
                            event_id: original.redacts.as_ref().map(|e| e.to_string()).unwrap_or_default(),
                        };
                        if !payload.event_id.is_empty() {
                            let _ = app.emit("message_deleted", &payload);
                        }
                    }
                }
            },
        );

        // Reaction event handler
        let react_app = sync_app.clone();
        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::reaction::SyncReactionEvent,
                  room: matrix_sdk::Room| {
                let app = react_app.clone();
                async move {
                    if let Some(original) = event.as_original() {
                        let payload = ReactionEvent {
                            room_id: room.room_id().to_string(),
                            event_id: event.event_id().to_string(),
                            reaction_key: original.content.relates_to.key.clone(),
                            sender: event.sender().to_string(),
                            sender_name: event.sender().localpart().to_string(),
                            relates_to: original.content.relates_to.event_id.to_string(),
                        };
                        let _ = app.emit("reaction", &payload);
                    }
                }
            },
        );

        // Room invite handler — notify frontend when someone invites us
        let invite_app = sync_app.clone();
        client.add_event_handler(
            move |event: matrix_sdk::ruma::events::room::member::StrippedRoomMemberEvent,
                  room: matrix_sdk::Room| {
                let app = invite_app.clone();
                async move {
                    // Only handle events targeting us (our membership changed to invite)
                    if event.content.membership == matrix_sdk::ruma::events::room::member::MembershipState::Invite {
                        let payload = InviteInfo {
                            room_id: room.room_id().to_string(),
                            room_name: room.display_name().await.map(|n| n.to_string()).ok(),
                            inviter: Some(event.sender.to_string()),
                            inviter_name: Some(event.sender.localpart().to_string()),
                        };
                        let _ = app.emit("room_invite", &payload);
                    }
                }
            },
        );

        slog(&sync_app, &sync_log, "info", "Sync loop starting...".into());
        let _ = sync_app.emit("sync_status", "syncing");

        let synced_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = synced_flag.clone();
        let cb_app = sync_app.clone();
        let cb_log = sync_log.clone();

        let settings = matrix_sdk::config::SyncSettings::default();
        match client.sync_with_result_callback(settings, move |result| {
            let flag = flag.clone();
            let app = cb_app.clone();
            let log = cb_log.clone();
            async move {
                match result {
                    Ok(_) => {
                        if !flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            slog_buf(&log, "info", "Initial sync complete".into());
                            let _ = app.emit("sync_status", "synced");
                        }
                    }
                    Err(ref e) => {
                        slog_buf(&log, "error", format!("Sync error (retrying): {}", e));
                    }
                }
                Ok(matrix_sdk::LoopCtrl::Continue)
            }
        }).await {
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
pub async fn fetch_media(
    mxc_url: String,
    state: State<'_, MatrixState>,
) -> Result<String, String> {
    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let path = mxc_url.strip_prefix("mxc://")
        .ok_or("Invalid mxc:// URL")?;
    let (server_name, media_id) = path.split_once('/')
        .ok_or("Invalid mxc URL format")?;

    let hs = client.homeserver().to_string();
    let hs = hs.trim_end_matches('/');
    let access_token = client.access_token()
        .ok_or("No access token available")?;

    // Try authenticated endpoint first, then unauthenticated fallback
    let urls = [
        format!("{}/_matrix/client/v1/media/download/{}/{}", hs, server_name, media_id),
        format!("{}/_matrix/media/v3/download/{}/{}", hs, server_name, media_id),
    ];

    let http = reqwest::Client::new();
    for url in &urls {
        if let Ok(resp) = http.get(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if bytes.is_empty() { continue; }
                    let content_type = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                        "image/png"
                    } else if bytes.starts_with(&[0xFF, 0xD8]) {
                        "image/jpeg"
                    } else if bytes.starts_with(b"GIF") {
                        "image/gif"
                    } else if bytes.starts_with(b"RIFF") {
                        "image/webp"
                    } else {
                        "application/octet-stream"
                    };
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    return Ok(format!("data:{};base64,{}", content_type, b64));
                }
            }
        }
    }
    Err("Failed to fetch media from any endpoint".into())
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

    slog(&app, &log, "info", "Accepted, waiting to start SAS...".into());

    // Spawn a task to start SAS (with retries) and wait for emojis.
    // After accept(), the ready event needs to propagate via sync before
    // start_sas() will succeed. The other side may also start SAS first.
    let uid = user_id.to_owned();
    let fid = flow_id.clone();
    let poll_log = log.clone();
    let poll_client = client.clone();
    tokio::spawn(async move {
        let mut sas_opt = None;

        // Phase 1: Get SAS started (up to 20s — either we start it or the other side does)
        for i in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Try to start SAS ourselves
            match request.start_sas().await {
                Ok(Some(s)) => {
                    slog(&app, &poll_log, "info", format!("SAS started by us after {}ms", (i + 1) * 500));
                    sas_opt = Some(s);
                    break;
                }
                Ok(None) => {
                    // Check if the other side started SAS
                    if let Some(verification) = poll_client
                        .encryption()
                        .get_verification(&uid, &fid)
                        .await
                    {
                        if let Some(s) = verification.sas() {
                            slog(&app, &poll_log, "info", format!("SAS started by other side after {}ms", (i + 1) * 500));
                            sas_opt = Some(s);
                            break;
                        }
                    }
                }
                Err(e) => {
                    slog(&app, &poll_log, "warn", format!("start_sas attempt failed: {}", e));
                }
            }

            if i % 6 == 5 {
                slog(&app, &poll_log, "info", format!("Still waiting to start SAS ({}s)...", (i + 1) / 2));
            }
        }

        let sas = match sas_opt {
            Some(s) => s,
            None => {
                slog(&app, &poll_log, "warn", "Timed out waiting for SAS to start".into());
                let _ = app.emit(
                    "verification_cancelled",
                    serde_json::json!({ "flow_id": fid, "reason": "Timed out starting SAS" }),
                );
                return;
            }
        };

        // Phase 2: Wait for emojis (up to 30s — key exchange needs to complete via sync)
        for i in 0..60 {
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
        slog(&app, &poll_log, "warn", "SAS emoji timeout after 30s".into());
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

#[tauri::command]
pub async fn search_users(
    query: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Vec<Buddy>, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("search_users: {}", query));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    use matrix_sdk::ruma::api::client::user_directory::search_users;

    let request = search_users::v3::Request::new(query);
    let response = client.send(request).await.map_err(|e| {
        slog(&app, &log, "error", format!("User search failed: {}", e));
        format!("User search failed: {}", e)
    })?;

    let hs = client.homeserver().to_string();
    let results: Vec<Buddy> = response.results.iter().map(|user| {
        Buddy {
            user_id: user.user_id.to_string(),
            display_name: user.display_name.clone().unwrap_or_else(|| user.user_id.to_string()),
            avatar_url: user.avatar_url.as_ref().and_then(|u| mxc_to_http(&hs, &u.to_string())),
            presence: "unknown".to_string(),
        }
    }).collect();

    slog(&app, &log, "info", format!("search_users: found {} results", results.len()));
    Ok(results)
}

#[tauri::command]
pub async fn join_room(
    room_id_or_alias: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Room, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("join_room: {}", room_id_or_alias));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let id = matrix_sdk::ruma::OwnedRoomOrAliasId::try_from(room_id_or_alias.as_str())
        .map_err(|e| format!("Invalid room ID or alias: {}", e))?;

    let room = client.join_room_by_id_or_alias(&id, &[]).await.map_err(|e| {
        slog(&app, &log, "error", format!("Failed to join room: {}", e));
        format!("Failed to join room: {}", e)
    })?;

    let room_id_str = room.room_id().to_string();
    let name = room
        .display_name()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| room_id_str.clone());

    slog(&app, &log, "info", format!("Joined room: {} ({})", name, room_id_str));

    Ok(Room {
        room_id: room_id_str,
        name,
        is_direct: false,
        last_message: None,
        unread_count: 0,
    })
}

#[tauri::command]
pub async fn create_room(
    room_alias: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Room, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("create_room: {}", room_alias));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    use matrix_sdk::ruma::api::client::room::create_room::v3::Request as CreateRoomRequest;
    use matrix_sdk::ruma::api::client::room::create_room::v3::RoomPreset;

    // Extract local alias from #alias:server → alias
    let local_alias = room_alias
        .trim_start_matches('#')
        .split(':')
        .next()
        .unwrap_or(&room_alias)
        .to_string();

    let mut request = CreateRoomRequest::new();
    request.room_alias_name = Some(local_alias.clone());
    request.name = Some(local_alias.clone());
    request.preset = Some(RoomPreset::PublicChat);

    let response = client.create_room(request).await.map_err(|e| {
        slog(&app, &log, "error", format!("Failed to create room: {}", e));
        format!("Failed to create room: {}", e)
    })?;

    let room_id_str = response.room_id().to_string();
    let name = if let Some(room) = client.get_room(response.room_id()) {
        room.display_name()
            .await
            .map(|n| n.to_string())
            .unwrap_or_else(|_| local_alias.clone())
    } else {
        local_alias.clone()
    };

    slog(&app, &log, "info", format!("Created room: {} ({})", name, room_id_str));

    Ok(Room {
        room_id: room_id_str,
        name,
        is_direct: false,
        last_message: None,
        unread_count: 0,
    })
}

#[tauri::command]
pub async fn leave_room(
    room_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("leave_room: {}", room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;
    let room = client.get_room(&room_id).ok_or("Room not found")?;

    room.leave().await.map_err(|e| {
        slog(&app, &log, "error", format!("Failed to leave room: {}", e));
        format!("Failed to leave room: {}", e)
    })?;

    slog(&app, &log, "info", "Left room OK".into());
    Ok(())
}

#[tauri::command]
pub async fn remove_buddy(
    user_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("remove_buddy: {}", user_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let target_id = matrix_sdk::ruma::UserId::parse(&user_id)
        .map_err(|e| format!("Invalid user ID: {}", e))?;

    let mut left_count = 0;
    for room in client.joined_rooms() {
        if !room.is_direct().await.unwrap_or(false) {
            continue;
        }
        let members = room
            .members(matrix_sdk::RoomMemberships::ACTIVE)
            .await
            .unwrap_or_default();
        let has_target = members.iter().any(|m| m.user_id() == target_id);
        if has_target {
            if let Err(e) = room.leave().await {
                slog(&app, &log, "error", format!("Failed to leave DM room {}: {}", room.room_id(), e));
            } else {
                left_count += 1;
            }
        }
    }

    slog(&app, &log, "info", format!("remove_buddy: left {} DM rooms with {}", left_count, user_id));
    Ok(())
}

#[tauri::command]
pub async fn get_pending_invites(
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Vec<InviteInfo>, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", "get_pending_invites".into());

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let mut invites = Vec::new();
    for room in client.invited_rooms() {
        let room_id = room.room_id().to_string();
        let room_name = room
            .display_name()
            .await
            .map(|n| n.to_string())
            .ok();

        // Try to find who invited us from the room state
        let mut inviter: Option<String> = None;
        let mut inviter_name: Option<String> = None;
        if let Ok(Some(member)) = room.get_member_no_sync(client.user_id().unwrap()).await {
            let event = member.event();
            inviter = Some(event.sender().to_string());
            inviter_name = Some(event.sender().localpart().to_string());
        }

        invites.push(InviteInfo {
            room_id,
            room_name,
            inviter,
            inviter_name,
        });
    }

    slog(&app, &log, "info", format!("get_pending_invites: {} invites", invites.len()));
    Ok(invites)
}

#[tauri::command]
pub async fn accept_invite(
    room_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<Room, String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("accept_invite: {}", room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id_parsed = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id_parsed).ok_or("Room not found")?;

    room.join().await.map_err(|e| {
        slog(&app, &log, "error", format!("Failed to accept invite: {}", e));
        format!("Failed to accept invite: {}", e)
    })?;

    let name = room
        .display_name()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| room_id.clone());
    let is_direct = room.is_direct().await.unwrap_or(false);

    slog(&app, &log, "info", format!("Accepted invite to: {}", name));

    Ok(Room {
        room_id,
        name,
        is_direct,
        last_message: None,
        unread_count: 0,
    })
}

#[tauri::command]
pub async fn reject_invite(
    room_id: String,
    app: tauri::AppHandle,
    state: State<'_, MatrixState>,
) -> Result<(), String> {
    let log = state.log.clone();
    slog(&app, &log, "info", format!("reject_invite: {}", room_id));

    let client_lock = state.client.lock().await;
    let client = client_lock.as_ref().ok_or("Not logged in")?;

    let room_id_parsed = matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
        .map_err(|e| format!("Invalid room ID: {}", e))?;

    let room = client.get_room(&room_id_parsed).ok_or("Room not found")?;

    room.leave().await.map_err(|e| {
        slog(&app, &log, "error", format!("Failed to reject invite: {}", e));
        format!("Failed to reject invite: {}", e)
    })?;

    slog(&app, &log, "info", "Invite rejected".into());
    Ok(())
}

#[tauri::command]
pub async fn set_dock_badge(count: u32) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSApplication;
        use objc2_foundation::NSString;
        unsafe {
            let mtm = MainThreadMarker::new_unchecked();
            let app = NSApplication::sharedApplication(mtm);
            if count == 0 {
                app.dockTile().setBadgeLabel(None);
            } else {
                let label = NSString::from_str(&count.to_string());
                app.dockTile().setBadgeLabel(Some(&label));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── mxc_to_http ──────────────────────────────────────────

    #[test]
    fn mxc_to_http_valid() {
        let url = mxc_to_http("https://matrix.org", "mxc://matrix.org/abc123");
        assert_eq!(
            url.unwrap(),
            "https://matrix.org/_matrix/media/v3/thumbnail/matrix.org/abc123?width=96&height=96&method=crop"
        );
    }

    #[test]
    fn mxc_to_http_trailing_slash_on_homeserver() {
        let url = mxc_to_http("https://matrix.org/", "mxc://matrix.org/abc123");
        assert_eq!(
            url.unwrap(),
            "https://matrix.org/_matrix/media/v3/thumbnail/matrix.org/abc123?width=96&height=96&method=crop"
        );
    }

    #[test]
    fn mxc_to_http_missing_prefix() {
        assert!(mxc_to_http("https://matrix.org", "https://not-mxc/foo").is_none());
    }

    #[test]
    fn mxc_to_http_missing_slash() {
        assert!(mxc_to_http("https://matrix.org", "mxc://noslash").is_none());
    }

    // ── extract_reply_fallback ───────────────────────────────

    #[test]
    fn extract_reply_simple() {
        let body = "> <@alice:matrix.org> hello world\n\nmy reply";
        let (sender, quoted) = extract_reply_fallback(body).unwrap();
        assert_eq!(sender, "@alice:matrix.org");
        assert_eq!(quoted, "hello world");
    }

    #[test]
    fn extract_reply_multiline() {
        let body = "> <@bob:example.com> line one\n> line two\n\nactual reply";
        let (sender, quoted) = extract_reply_fallback(body).unwrap();
        assert_eq!(sender, "@bob:example.com");
        assert_eq!(quoted, "line one\nline two");
    }

    #[test]
    fn extract_reply_no_fallback() {
        assert!(extract_reply_fallback("just a normal message").is_none());
    }

    #[test]
    fn extract_reply_empty() {
        assert!(extract_reply_fallback("").is_none());
    }

    // ── strip_reply_fallback ─────────────────────────────────

    #[test]
    fn strip_reply_with_fallback() {
        let body = "> <@alice:matrix.org> quoted\n\nactual reply";
        assert_eq!(strip_reply_fallback(body), "actual reply");
    }

    #[test]
    fn strip_reply_no_fallback() {
        assert_eq!(strip_reply_fallback("just text"), "just text");
    }

    #[test]
    fn strip_reply_bare_quote_lines() {
        let body = "> line1\n>\n> line3\n\nreply text";
        assert_eq!(strip_reply_fallback(body), "reply text");
    }

    #[test]
    fn strip_reply_only_fallback() {
        let body = "> <@user:host> quoted";
        assert_eq!(strip_reply_fallback(body), "");
    }
}
