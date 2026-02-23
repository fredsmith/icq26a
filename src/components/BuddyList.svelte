<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { buddyList, rooms, unreadCounts, isLoggedIn, currentUserId, currentStatus } from '../lib/stores'
  import { getBuddyList, getRooms, matrixLogout, matrixDisconnect, tryRestoreSession } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import type { Buddy, Message } from '../lib/types'
  import StatusPicker from './StatusPicker.svelte'
  import TitleBar from './TitleBar.svelte'
  import { openPreferencesWindow, openDirectMessageWindow, openChatRoomWindow, openServerLogWindow } from '../lib/windows'

  const isOffline = $derived($currentStatus === 'offline')
  const presenceAvailable = $derived($buddyList.some(b => b.presence !== 'unknown'))
  const onlineBuddies = $derived($buddyList.filter(b => b.presence !== 'offline' && b.presence !== 'unknown'))
  const offlineBuddies = $derived($buddyList.filter(b => b.presence === 'offline'))
  const groupRooms = $derived($rooms.filter(r => !r.is_direct))

  let refreshing = false
  async function refreshLists() {
    if (refreshing) return
    refreshing = true
    try {
      const fetchedBuddies = await getBuddyList()
      buddyList.set(fetchedBuddies)
      const fetchedRooms = await getRooms()
      rooms.set(fetchedRooms)
    } catch (e) {
      console.error('Failed to load buddy list:', e)
    } finally {
      refreshing = false
    }
  }

  onMount(async () => {
    await refreshLists()

    await listen<Message>('new_message', (event) => {
      const roomId = event.payload.room_id
      if (roomId) {
        unreadCounts.update(counts => ({
          ...counts,
          [roomId]: (counts[roomId] || 0) + 1,
        }))
      }
    })

    await listen('rooms_changed', () => {
      refreshLists()
    })
  })

  function openBuddyChat(buddy: Buddy) {
    const room = $rooms.find(r => r.is_direct && r.name === buddy.display_name)
    if (!room) return
    unreadCounts.update(counts => {
      const { [room.room_id]: _, ...rest } = counts
      return rest
    })
    openDirectMessageWindow(room.room_id, buddy.display_name)
  }

  function openRoomChat(room: { room_id: string; name: string }) {
    unreadCounts.update(counts => {
      const { [room.room_id]: _, ...rest } = counts
      return rest
    })
    openChatRoomWindow(room.room_id, room.name)
  }

  function getUnreadForBuddy(buddy: Buddy): number {
    const room = $rooms.find(r => r.is_direct && r.name === buddy.display_name)
    if (!room) return 0
    return $unreadCounts[room.room_id] || 0
  }

  async function handleDisconnect() {
    try {
      await matrixDisconnect()
    } catch (e) {
      console.error('Disconnect failed:', e)
    }
    currentStatus.set('offline')
  }

  async function handleReconnect() {
    currentStatus.set('online')
    try {
      await tryRestoreSession()
      await invoke('start_sync')
      await refreshLists()
    } catch (e) {
      console.error('Reconnect failed:', e)
    }
  }

  async function handleLogout() {
    try {
      await matrixLogout()
    } catch (e) {
      console.error('Logout failed:', e)
    }
    isLoggedIn.set(false)
    currentUserId.set(null)
    currentStatus.set('online')
    buddyList.set([])
    rooms.set([])
    unreadCounts.set({})
  }
</script>

<div class="window buddy-list-window">
  <TitleBar title="ICQ26a" showMinimize />
  <div class="window-body">
    <div class="buddy-scroll" class:disconnected={isOffline}>
      {#if presenceAvailable}
        {#if onlineBuddies.length > 0}
          <div class="group-header">Online</div>
          {#each onlineBuddies as buddy}
            <button class="buddy-row" onclick={() => openBuddyChat(buddy)}>
              <span class="status-dot online"></span>
              {buddy.display_name}
              {#if getUnreadForBuddy(buddy) > 0}
                <span class="unread-badge">{getUnreadForBuddy(buddy)}</span>
              {/if}
            </button>
          {/each}
        {/if}
        {#if offlineBuddies.length > 0}
          <div class="group-header">Offline</div>
          {#each offlineBuddies as buddy}
            <button class="buddy-row offline" onclick={() => openBuddyChat(buddy)}>
              <span class="status-dot"></span>
              {buddy.display_name}
              {#if getUnreadForBuddy(buddy) > 0}
                <span class="unread-badge">{getUnreadForBuddy(buddy)}</span>
              {/if}
            </button>
          {/each}
        {/if}
      {:else}
        {#each $buddyList as buddy}
          <button class="buddy-row" onclick={() => openBuddyChat(buddy)}>
            <span class="status-dot online"></span>
            {buddy.display_name}
            {#if getUnreadForBuddy(buddy) > 0}
              <span class="unread-badge">{getUnreadForBuddy(buddy)}</span>
            {/if}
          </button>
        {/each}
      {/if}
      {#if groupRooms.length > 0}
        <div class="group-header">Rooms</div>
        {#each groupRooms as room}
          <button class="buddy-row" onclick={() => openRoomChat(room)}>
            {room.name}
            {#if $unreadCounts[room.room_id] > 0}
              <span class="unread-badge">{$unreadCounts[room.room_id]}</span>
            {/if}
          </button>
        {/each}
      {/if}
      {#if $buddyList.length === 0 && groupRooms.length === 0}
        <p class="empty-text">No contacts or rooms</p>
      {/if}
    </div>
  </div>

  <!-- Bottom toolbar -->
  <div class="buddy-toolbar">
    <StatusPicker {presenceAvailable} onLogout={handleLogout} onDisconnect={handleDisconnect} onReconnect={handleReconnect} />
    <button onclick={openPreferencesWindow}>Settings</button>
    <button onclick={openServerLogWindow}>Log</button>
  </div>
</div>

<style>
  .buddy-list-window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
  }
  .buddy-list-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 0;
  }
  .buddy-scroll {
    flex: 1;
    overflow-y: auto;
    background: #c0c0c0;
    border: 2px inset #c0c0c0;
    margin: 4px;
  }
  .buddy-scroll.disconnected {
    opacity: 0.5;
    pointer-events: none;
  }
  .group-header {
    font-weight: bold;
    font-size: 11px;
    padding: 4px 8px 2px;
    color: #444;
  }
  .buddy-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    border: none;
    background: transparent;
    padding: 2px 12px;
    text-align: left;
    cursor: pointer;
    font-size: 11px;
  }
  .buddy-row:hover {
    background: #000080;
    color: white;
  }
  .buddy-row.offline {
    color: #888;
  }
  .status-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #999;
  }
  .status-dot.online {
    background: #00cc00;
  }
  .empty-text {
    text-align: center;
    color: #888;
    padding: 20px;
    font-size: 11px;
  }
  .unread-badge {
    margin-left: auto;
    background: #ff0000;
    color: white;
    font-size: 9px;
    font-weight: bold;
    min-width: 14px;
    height: 14px;
    line-height: 14px;
    text-align: center;
    border-radius: 7px;
    padding: 0 3px;
  }
  .buddy-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
    border-top: 1px solid #808080;
  }
</style>
