import '98.css'
import './app.css'
import { mount } from 'svelte'
import { open } from '@tauri-apps/plugin-shell'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'

const ZOOM_STEP = 0.1
const MIN_ZOOM = 0.5
const MAX_ZOOM = 3.0
let zoomFactor = 1.0
let baseSize: { width: number; height: number } | null = null

document.addEventListener('keydown', async (e) => {
  if (!e.metaKey) return
  if (e.key !== '=' && e.key !== '+' && e.key !== '-') return
  e.preventDefault()

  const win = getCurrentWindow()
  if (!baseSize) {
    const s = await win.innerSize()
    const dpr = window.devicePixelRatio || 1
    baseSize = { width: Math.round(s.width / dpr), height: Math.round(s.height / dpr) }
  }

  if (e.key === '=' || e.key === '+') {
    zoomFactor = Math.min(MAX_ZOOM, +(zoomFactor + ZOOM_STEP).toFixed(1))
  } else {
    zoomFactor = Math.max(MIN_ZOOM, +(zoomFactor - ZOOM_STEP).toFixed(1))
  }

  document.documentElement.style.zoom = String(zoomFactor)
  await win.setSize(new LogicalSize(
    Math.round(baseSize.width * zoomFactor),
    Math.round(baseSize.height * zoomFactor),
  ))
})

// Intercept clicks on <a target="_blank"> to open in system browser
document.addEventListener('click', (e) => {
  const anchor = (e.target as HTMLElement).closest('a[target="_blank"]') as HTMLAnchorElement | null
  if (anchor?.href) {
    e.preventDefault()
    open(anchor.href)
  }
})

const params = new URLSearchParams(window.location.search)
const windowType = params.get('window')

;(async () => {
  let component: any
  let props: Record<string, any> = {}

  if (windowType === 'preferences') {
    const mod = await import('./components/PreferencesWindow.svelte')
    component = mod.default
  } else if (windowType === 'dm') {
    const mod = await import('./components/DirectMessage.svelte')
    component = mod.default
    props = {
      roomId: params.get('roomId') ?? '',
      roomName: params.get('roomName') ?? 'Unknown',
    }
  } else if (windowType === 'serverlog') {
    const mod = await import('./components/ServerLog.svelte')
    component = mod.default
  } else if (windowType === 'userinfo') {
    const mod = await import('./components/UserInfo.svelte')
    component = mod.default
    props = {
      userId: params.get('userId') ?? '',
      displayName: params.get('displayName') ?? 'Unknown',
    }
  } else if (windowType === 'roominfo') {
    const mod = await import('./components/RoomInfo.svelte')
    component = mod.default
    props = {
      roomId: params.get('roomId') ?? '',
      roomName: params.get('roomName') ?? 'Room',
    }
  } else if (windowType === 'chatroom') {
    const mod = await import('./components/ChatRoom.svelte')
    component = mod.default
    props = {
      roomId: params.get('roomId') ?? '',
      roomName: params.get('roomName') ?? 'Chat',
    }
  } else if (windowType === 'finduser') {
    const mod = await import('./components/FindUser.svelte')
    component = mod.default
  } else if (windowType === 'joinroom') {
    const mod = await import('./components/JoinRoom.svelte')
    component = mod.default
  } else if (windowType === 'browsespaces') {
    const mod = await import('./components/BrowseSpaces.svelte')
    component = mod.default
    const spaceId = params.get('spaceId')
    const spaceName = params.get('spaceName')
    if (spaceId && spaceName) {
      props = { spaceId, spaceName }
    }
  } else {
    const mod = await import('./App.svelte')
    component = mod.default
  }

  mount(component, {
    target: document.getElementById('app')!,
    props,
  })
})()
