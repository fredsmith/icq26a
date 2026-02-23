# ICQ26a features

## Matrix client

Connects over Matrix to display a contact list, send/receive direct messages, access multi-user chats, and chat rooms.

### Authentication & configuration

- Preferences pane allows user to configure homeserver URL (default: matrix.org)
- Standard Matrix login flow (username/password)
- "Awaiting Authorization" in buddy list maps to Matrix invite state

### Presence & status

- Online, Away, N/A, Occupied, DND, Free For Chat, Invisible, Offline
- Map to Matrix presence states where possible; store extended status in account data

### Notifications

- Classic ICQ-style notification sounds (user-provided assets)
- Visual notification for new messages

### File transfer

- Support Matrix file uploads/downloads (m.file, m.image, m.audio, m.video events)
- No custom transfer protocol — only what Matrix natively supports

## UI

Capture the spirit of ICQ98a / Windows 95 styling — grey backgrounds, beveled/blocky UI elements, cascading menus, system-font aesthetics. Not pixel-perfect, but unmistakably retro. Scale appropriately for modern screen resolutions.

### Core windows

1. **Buddy list** — narrow vertical window, online/offline groupings, status icons, tab filters (All / User), bottom toolbar with status picker
2. **Direct message** — simple message session with To/From fields, message area, Send/Cancel/History buttons
3. **Multi-user chat** — split-pane layout with participant streams, timestamps, menu bar
4. **Preferences** — homeserver URL, account settings, notification preferences

## App target

Cross-platform: macOS and Linux. Electron is acceptable, but Go or Rust are preferred.
