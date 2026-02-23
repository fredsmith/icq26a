import '98.css'
import './app.css'
import { mount } from 'svelte'

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
  } else if (windowType === 'chatroom') {
    const mod = await import('./components/ChatRoom.svelte')
    component = mod.default
    props = {
      roomId: params.get('roomId') ?? '',
      roomName: params.get('roomName') ?? 'Chat',
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
