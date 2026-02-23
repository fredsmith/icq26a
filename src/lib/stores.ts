import { writable } from 'svelte/store'
import type { Buddy, Room, PresenceStatus, AppPreferences } from './types'

export const isLoggedIn = writable(false)
export const currentUserId = writable<string | null>(null)
export const buddyList = writable<Buddy[]>([])
export const rooms = writable<Room[]>([])
export const currentStatus = writable<PresenceStatus>('online')
export const activeRoomId = writable<string | null>(null)
export const unreadCounts = writable<Record<string, number>>({})

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
