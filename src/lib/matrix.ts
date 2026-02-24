import { invoke } from '@tauri-apps/api/core'
import type { Buddy, Room, Message, MessagesPage, LoginCredentials, LogEntry, UserProfile, RoomProfile } from './types'

export async function matrixLogin(credentials: LoginCredentials): Promise<string> {
  return invoke('matrix_login', { credentials })
}

export async function matrixRegister(credentials: LoginCredentials): Promise<string> {
  return invoke('matrix_register', { credentials })
}

export async function matrixLogout(): Promise<void> {
  return invoke('matrix_logout')
}

export async function matrixDisconnect(): Promise<void> {
  return invoke('matrix_disconnect')
}

export async function getBuddyList(): Promise<Buddy[]> {
  return invoke('get_buddy_list')
}

export async function getRoomMembers(roomId: string): Promise<Buddy[]> {
  return invoke('get_room_members', { roomId })
}

export async function getRooms(): Promise<Room[]> {
  return invoke('get_rooms')
}

export async function getRoomMessages(roomId: string, limit: number = 50, from?: string): Promise<MessagesPage> {
  return invoke('get_room_messages', { roomId, limit, from: from ?? null })
}

export async function sendMessage(roomId: string, body: string, inReplyToEventId?: string): Promise<void> {
  return invoke('send_message', { roomId, body, inReplyToEventId: inReplyToEventId ?? null })
}

export async function sendTyping(roomId: string, typing: boolean): Promise<void> {
  return invoke('send_typing', { roomId, typing })
}

export async function markAsRead(roomId: string, eventId: string): Promise<void> {
  return invoke('mark_as_read', { roomId, eventId })
}

export async function setPresence(status: string): Promise<void> {
  return invoke('set_presence', { status })
}

export async function tryRestoreSession(): Promise<string> {
  return invoke('try_restore_session')
}

export async function acceptVerification(userId: string, flowId: string): Promise<void> {
  return invoke('accept_verification', { userId, flowId })
}

export async function confirmVerification(userId: string, flowId: string): Promise<void> {
  return invoke('confirm_verification', { userId, flowId })
}

export async function cancelVerification(userId: string, flowId: string): Promise<void> {
  return invoke('cancel_verification', { userId, flowId })
}

export async function getServerLog(): Promise<LogEntry[]> {
  return invoke('get_server_log')
}

export async function getUserProfile(userId: string): Promise<UserProfile> {
  return invoke('get_user_profile', { userId })
}

export async function getRoomInfo(roomId: string): Promise<RoomProfile> {
  return invoke('get_room_info', { roomId })
}

export async function createDmRoom(userId: string): Promise<Room> {
  return invoke('create_dm_room', { userId })
}

export async function searchUsers(query: string): Promise<Buddy[]> {
  return invoke('search_users', { query })
}

export async function joinRoom(roomIdOrAlias: string): Promise<Room> {
  return invoke('join_room', { roomIdOrAlias })
}

export async function createRoom(roomAlias: string): Promise<Room> {
  return invoke('create_room', { roomAlias })
}

export async function leaveRoom(roomId: string): Promise<void> {
  return invoke('leave_room', { roomId })
}

export async function removeBuddy(userId: string): Promise<void> {
  return invoke('remove_buddy', { userId })
}

export async function fetchMedia(mxcUrl: string): Promise<string> {
  return invoke('fetch_media', { mxcUrl })
}
