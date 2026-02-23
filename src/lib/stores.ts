import { writable } from 'svelte/store'
import type { Buddy, Room, PresenceStatus, AppPreferences } from './types'

export const isLoggedIn = writable(false)
export const currentUserId = writable<string | null>(null)
export const buddyList = writable<Buddy[]>([])
export const rooms = writable<Room[]>([])
export const currentStatus = writable<PresenceStatus>('online')
export const activeRoomId = writable<string | null>(null)
export const preferences = writable<AppPreferences>({
  homeserver: 'https://matrix.org',
  notification_sounds: true,
})
export const unreadCounts = writable<Record<string, number>>({})
