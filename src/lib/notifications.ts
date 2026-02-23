import { get } from 'svelte/store'
import { preferences } from './stores'

let messageSound: HTMLAudioElement | null = null

export function initNotifications() {
  try {
    messageSound = new Audio('/sounds/message.wav')
  } catch {
    console.warn('Notification sound not found')
  }
}

export function playMessageSound() {
  const prefs = get(preferences)
  if (prefs.notification_sounds && messageSound) {
    messageSound.currentTime = 0
    messageSound.play().catch(() => {})
  }
}
