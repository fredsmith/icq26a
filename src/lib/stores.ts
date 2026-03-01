import { writable } from 'svelte/store'
import type { Buddy, Room, Space, PresenceStatus, AppPreferences, RoomTagMap } from './types'

export const isLoggedIn = writable(false)
export const currentUserId = writable<string | null>(null)
export const buddyList = writable<Buddy[]>([])
export const rooms = writable<Room[]>([])
export const spaces = writable<Space[]>([])
export const currentStatus = writable<PresenceStatus>('online')
export const activeRoomId = writable<string | null>(null)
export const unreadCounts = writable<Record<string, number>>({})
export const syncing = writable(false)
export const roomTags = writable<RoomTagMap>({})

const PREFS_KEY = 'icq26a_preferences'

const defaultPrefs: AppPreferences = {
  homeserver: 'https://matrix.org',
  notification_sounds: true,
}

function loadPrefs(): AppPreferences {
  try {
    const raw = localStorage.getItem(PREFS_KEY)
    if (raw) return { ...defaultPrefs, ...JSON.parse(raw) }
  } catch {}
  return { ...defaultPrefs }
}

function createPreferencesStore() {
  const { subscribe, set, update } = writable<AppPreferences>(loadPrefs())

  return {
    subscribe,
    set(value: AppPreferences) {
      localStorage.setItem(PREFS_KEY, JSON.stringify(value))
      set(value)
    },
    update(fn: (prev: AppPreferences) => AppPreferences) {
      update((prev) => {
        const next = fn(prev)
        localStorage.setItem(PREFS_KEY, JSON.stringify(next))
        return next
      })
    },
  }
}

export const preferences = createPreferencesStore()

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => {
    if (e.key === PREFS_KEY && e.newValue) {
      try {
        const parsed = JSON.parse(e.newValue)
        preferences.set({ ...defaultPrefs, ...parsed })
      } catch {}
    }
  })
}

const SPACE_COLLAPSE_KEY = 'icq26a_space_collapse'

function loadCollapseState(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(SPACE_COLLAPSE_KEY)
    if (raw) return JSON.parse(raw)
  } catch {}
  return {}
}

function createSpaceCollapseStore() {
  const { subscribe, update } = writable<Record<string, boolean>>(loadCollapseState())

  return {
    subscribe,
    toggle(spaceId: string) {
      update((prev) => {
        const next = { ...prev, [spaceId]: !prev[spaceId] }
        localStorage.setItem(SPACE_COLLAPSE_KEY, JSON.stringify(next))
        return next
      })
    },
  }
}

export const spaceCollapseState = createSpaceCollapseStore()
