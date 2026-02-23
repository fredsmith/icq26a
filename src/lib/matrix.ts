import { invoke } from '@tauri-apps/api/core'
import type { Buddy, Room, Message, LoginCredentials, LogEntry } from './types'

export async function matrixLogin(credentials: LoginCredentials): Promise<string> {
  return invoke('matrix_login', { credentials })
}

export async function matrixLogout(): Promise<void> {
  return invoke('matrix_logout')
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

export async function getRoomMessages(roomId: string, limit: number = 50): Promise<Message[]> {
  return invoke('get_room_messages', { roomId, limit })
}

export async function sendMessage(roomId: string, body: string): Promise<void> {
  return invoke('send_message', { roomId, body })
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
