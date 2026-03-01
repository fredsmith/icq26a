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
  | 'unknown'

export interface Room {
  room_id: string
  name: string
  is_direct: boolean
  last_message: string | null
  unread_count: number
}

export interface Space {
  room_id: string
  name: string
  child_room_ids: string[]
}

export interface Message {
  room_id: string
  event_id: string
  sender: string
  sender_name: string
  body: string
  timestamp: number
  msg_type: 'text' | 'image' | 'file' | 'audio' | 'video' | 'unknown'
  media_url?: string | null
  filename?: string | null
  in_reply_to?: string | null
  reply_sender_name?: string | null
  reply_body?: string | null
}

export interface TypingEvent {
  room_id: string
  user_ids: string[]
  display_names: string[]
}

export interface MessageEditEvent {
  room_id: string
  original_event_id: string
  new_body: string
  sender: string
  sender_name: string
}

export interface MessageDeletedEvent {
  room_id: string
  event_id: string
}

export interface ReactionEvent {
  room_id: string
  event_id: string
  reaction_key: string
  sender: string
  sender_name: string
  relates_to: string
}

export interface InviteInfo {
  room_id: string
  room_name: string | null
  inviter: string | null
  inviter_name: string | null
}

export interface MessagesPage {
  messages: Message[]
  end_token: string | null
}

export interface SharedRoom {
  room_id: string
  name: string
}

export interface UserProfile {
  user_id: string
  display_name: string
  avatar_url: string | null
  presence: string
  last_seen_ago: number | null
  shared_rooms: SharedRoom[]
}

export interface RoomProfile {
  room_id: string
  name: string
  topic: string | null
  is_direct: boolean
  member_count: number
}

export type RoomTagMap = Record<string, string[]>

export interface LoginCredentials {
  homeserver: string
  username: string
  password: string
}

export interface AppPreferences {
  homeserver: string
  notification_sounds: boolean
}

export interface LogEntry {
  timestamp: number
  level: string
  message: string
}

export interface VerificationEmoji {
  symbol: string
  description: string
}

export interface VerificationRequestEvent {
  flow_id: string
  user_id: string
  is_self_verification: boolean
}

export interface VerificationEmojisEvent {
  flow_id: string
  user_id: string
  emojis: VerificationEmoji[]
}

export interface PublicSpace {
  room_id: string
  name: string
  topic: string | null
  num_joined_members: number
  alias: string | null
}

export interface SpaceChild {
  room_id: string
  name: string
  topic: string | null
  num_joined_members: number
  room_type: string | null
  is_joined: boolean
}
