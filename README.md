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
brew install --cask --no-quarantine icq26a
```

The `--no-quarantine` flag is needed because the app is unsigned.

### MacOS special instructions (manual install)

After you download the client, you will need to allowlist it in MacOS's privacy & Security
screen to run it.  You can also run `xattr -d com.apple.quarantine /Applications/ICQ26a.app`
in the terminal to allow-list it.  This is required because I am not a paid Apple Developer
and Apple charges $100/year to bypass this requirement.