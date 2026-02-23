export interface Buddy {
  user_id: string
  display_name: string
  avatar_url: string | null
  presence: PresenceStatus
}

export type PresenceStatus =
  | 'online'
  | 'away'
  | 'na'
  | 'occupied'
  | 'dnd'
  | 'free_for_chat'
  | 'invisible'
  | 'offline'

export interface Room {
  room_id: string
  name: string
  is_direct: boolean
  last_message: string | null
  unread_count: number
}

export interface Message {
  room_id: string
  event_id: string
  sender: string
  sender_name: string
  body: string
  timestamp: number
  msg_type: 'text' | 'image' | 'file' | 'audio' | 'video'
}

export interface LoginCredentials {
  homeserver: string
  username: string
  password: string
}

export interface AppPreferences {
  homeserver: string
  notification_sounds: boolean
}
