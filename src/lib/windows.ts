import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

interface ChildWindowOptions {
  label: string
  url: string
  title: string
  width: number
  height: number
  parent?: string
  resizable?: boolean
}

export async function openChildWindow(opts: ChildWindowOptions) {
  const existing = await WebviewWindow.getByLabel(opts.label)
  if (existing) {
    await existing.setFocus()
    return
  }

  new WebviewWindow(opts.label, {
    url: opts.url,
    title: opts.title,
    width: opts.width,
    height: opts.height,
    decorations: false,
    center: true,
    parent: opts.parent,
    resizable: opts.resizable,
  })
}

export function sanitizeLabel(id: string): string {
  return id.replace(/[^a-zA-Z0-9_-]/g, '_')
}

export function openPreferencesWindow() {
  openChildWindow({
    label: 'preferences',
    url: '/?window=preferences',
    title: 'Preferences',
    width: 380,
    height: 220,
    parent: 'main',
  })
}

export function openDirectMessageWindow(roomId: string, roomName: string) {
  openChildWindow({
    label: `dm-${sanitizeLabel(roomId)}`,
    url: `/?window=dm&roomId=${encodeURIComponent(roomId)}&roomName=${encodeURIComponent(roomName)}`,
    title: `${roomName} - Message Session`,
    width: 440,
    height: 480,
  })
}

export function openServerLogWindow() {
  openChildWindow({
    label: 'serverlog',
    url: '/?window=serverlog',
    title: 'Server Log',
    width: 560,
    height: 400,
  })
}

export function openUserInfoWindow(userId: string, displayName: string) {
  openChildWindow({
    label: `userinfo-${sanitizeLabel(userId)}`,
    url: `/?window=userinfo&userId=${encodeURIComponent(userId)}&displayName=${encodeURIComponent(displayName)}`,
    title: `User Info - ${displayName}`,
    width: 300,
    height: 340,
  })
}

export function openRoomInfoWindow(roomId: string, roomName: string) {
  openChildWindow({
    label: `roominfo-${sanitizeLabel(roomId)}`,
    url: `/?window=roominfo&roomId=${encodeURIComponent(roomId)}&roomName=${encodeURIComponent(roomName)}`,
    title: `Room Info - ${roomName}`,
    width: 300,
    height: 340,
  })
}

export function openChatRoomWindow(roomId: string, roomName: string) {
  openChildWindow({
    label: `chatroom-${sanitizeLabel(roomId)}`,
    url: `/?window=chatroom&roomId=${encodeURIComponent(roomId)}&roomName=${encodeURIComponent(roomName)}`,
    title: `ICQ Chat - ${roomName}`,
    width: 640,
    height: 500,
  })
}

export function openFindUserWindow() {
  openChildWindow({
    label: 'finduser',
    url: '/?window=finduser',
    title: 'Find Users',
    width: 340,
    height: 380,
    parent: 'main',
  })
}

export function openJoinRoomWindow() {
  openChildWindow({
    label: 'joinroom',
    url: '/?window=joinroom',
    title: 'Join Room',
    width: 340,
    height: 200,
    parent: 'main',
  })
}
