# CLAUDE.md

## Project

ICQ26a — a Matrix chat client styled after ICQ 98a. Tauri v2 desktop app with a Svelte 5 frontend and Rust backend.

## Architecture

- **Frontend**: Svelte 5 (runes: `$state`, `$derived`, `$props`, `$effect`) + TypeScript + 98.css
- **Backend**: Rust via Tauri v2 commands in `src-tauri/src/commands.rs`
- **Matrix**: `matrix-sdk` 0.16 with sqlite store for crypto/room persistence
- **Multi-window**: Each window type (DM, chat room, find user, join room, user info, room info, preferences, server log) is a separate OS window routed via query params in `src/main.ts`

## Key Files

| File | Purpose |
|------|---------|
| `src-tauri/src/commands.rs` | All Tauri commands (login, register, sync, messaging, rooms, verification, logging) |
| `src-tauri/src/matrix_client.rs` | Shared types (`Buddy`, `Room`, `Message`, `MatrixState`, `ServerLog`) and helpers |
| `src-tauri/src/lib.rs` | Command registration |
| `src/main.ts` | Window type router — loads component based on `?window=` param |
| `src/App.svelte` | Root: session restore → Login or BuddyList |
| `src/components/Login.svelte` | Login/Register screen with mode toggle |
| `src/components/BuddyList.svelte` | Main contact list with Find Users/Join Room toolbar and context menus |
| `src/components/FindUser.svelte` | User search dialog (directory search + direct user ID entry) |
| `src/components/JoinRoom.svelte` | Join/create room dialog |
| `src/components/DirectMessage.svelte` | 1:1 DM window |
| `src/components/ChatRoom.svelte` | Multi-user chat room window |
| `src/components/UserInfo.svelte` | User profile panel |
| `src/components/RoomInfo.svelte` | Room info panel |
| `src/lib/matrix.ts` | TypeScript wrappers around Tauri `invoke()` calls |
| `src/lib/stores.ts` | Svelte stores (buddyList, rooms, unreadCounts, etc.) |
| `src/lib/windows.ts` | Window creation helpers (`openDirectMessageWindow`, etc.) |
| `src/lib/types.ts` | TypeScript interfaces matching Rust structs |

## Commands

```bash
npm install          # install frontend dependencies
npm run tauri dev    # development with hot reload
npm run tauri build  # production build
cargo check --manifest-path src-tauri/Cargo.toml  # type-check Rust
```

## Conventions

- Svelte 5 runes only — no `$:` reactive statements, no `export let` props
- 98.css provides Windows 95 styling — use `.window`, `.window-body`, `.title-bar` classes
- Custom `TitleBar.svelte` component replaces native window decorations (`decorations: false` in tauri.conf.json)
- All Tauri commands return `Result<T, String>` — errors are string messages
- `MatrixState` is managed Tauri state with `Arc<Mutex<Option<Client>>>` for the Matrix client
- `ServerLog` uses `std::sync::Mutex` (not tokio) since pushes are synchronous
- Session tokens persist in `data_dir()/session.json`; crypto state in sqlite store at `data_dir()/store/`
- New windows are opened via `WebviewWindow::builder` with query params, not Tauri's multi-window config
- Registration uses raw HTTP requests (not the SDK) to handle the UIAA handshake (dummy auth flow)
- Buddy list deduplicates by user_id across multiple DM rooms

## CI

GitHub Actions builds on push to main for macOS (arm64 + x64) and Linux (x64). Artifacts are uploaded per-platform.
