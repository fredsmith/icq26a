# <img src="ref/loading-flower.gif" width="32" height="32" alt="ICQ26a logo" /> ICQ26a

A Matrix chat client with the look and feel of ICQ 98a, built with Tauri, Svelte 5, and Rust.

![Buddy List & Join Room](ref/add_users.png)
![Conversations](ref/conversations.png)
![Login](ref/login.png)

## Features

- **Buddy list** with online/offline grouping and presence indicators
- **Direct messages** and **multi-user chat rooms**
- **User search** via Matrix user directory and direct user ID entry
- **Join or create rooms** by alias or room ID
- **Registration** — sign up for a new account in-app (with UIAA dummy-auth support)
- **File uploads and downloads** via Matrix media API
- **Session persistence** — login once, sessions restore on relaunch
- **SAS emoji verification** for cross-signing trust
- **Notification sounds** and unread message badges
- **Status picker** — Online, Away, Do Not Disturb, and more
- **User info** and **room info** panels
- **Right-click context menus** — message, view info, remove buddy, leave room
- **Server log** debug window for visibility into backend operations
- **Windows 95 aesthetic** via [98.css](https://jdan.github.io/98.css/)


## Installation

Check the [latest release](https://github.com/fredsmith/icq26a/releases) for your platform of choice.

### Homebrew (macOS)

```bash
brew tap fredsmith/icq26a https://github.com/fredsmith/icq26a
brew install --cask icq26a
```

### macOS Gatekeeper (unsigned app)

ICQ26a is not signed with an Apple Developer certificate (Apple charges
$100/year). On first launch, macOS will block it. To allow it:

1. Open **System Settings > Privacy & Security**
2. Scroll down and click **"Open Anyway"** next to the ICQ26a message

Or run in the terminal:
```bash
xattr -d com.apple.quarantine /Applications/ICQ26a.app
```