<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { buddyList, rooms, activeRoomId, unreadCounts } from '../lib/stores'
  import { getBuddyList, getRooms } from '../lib/matrix'
  import type { Buddy, Message } from '../lib/types'
  import StatusPicker from './StatusPicker.svelte'

  let activeTab = $state<'all' | 'users'>('all')

  const onlineBuddies = $derived($buddyList.filter(b => b.presence !== 'offline'))
  const offlineBuddies = $derived($buddyList.filter(b => b.presence === 'offline'))
  const filteredRooms = $derived($rooms.filter(r => !r.is_direct))

  onMount(async () => {
    try {
      const fetchedBuddies = await getBuddyList()
      buddyList.set(fetchedBuddies)
      const fetchedRooms = await getRooms()
      rooms.set(fetchedRooms)
    } catch (e) {
      console.error('Failed to load buddy list:', e)
    }

    await listen<{ room_id: string; message: Message }>('new_message', (event) => {
      const roomId = event.payload.room_id
      if (roomId !== $activeRoomId) {
        unreadCounts.update(counts => ({
          ...counts,
          [roomId]: (counts[roomId] || 0) + 1,
        }))
      }
    })
  })

  function openChat(roomId: string) {
    activeRoomId.set(roomId)
    unreadCounts.update(counts => {
      const { [roomId]: _, ...rest } = counts
      return rest
    })
  }

  function findRoomForBuddy(buddy: Buddy): string | null {
    const room = $rooms.find(r => r.is_direct && r.name === buddy.display_name)
    return room?.room_id ?? null
  }

  function getUnreadForBuddy(buddy: Buddy): number {
    const room = $rooms.find(r => r.is_direct && r.name === buddy.display_name)
    if (!room) return 0
    return $unreadCounts[room.room_id] || 0
  }
</script>

<div class="window buddy-list-window">
  <div class="title-bar">
    <div class="title-bar-text">ICQ26a</div>
    <div class="title-bar-controls">
      <button aria-label="Minimize"></button>
      <button aria-label="Close"></button>
    </div>
  </div>
  <div class="window-body">
    <!-- Tab bar -->
    <menu role="tablist">
      <button
        aria-selected={activeTab === 'all'}
        onclick={() => (activeTab = 'all')}
      >All</button>
      <button
        aria-selected={activeTab === 'users'}
        onclick={() => (activeTab = 'users')}
      >Users</button>
    </menu>

    <!-- Buddy/room list -->
    <div class="buddy-scroll" role="tabpanel">
      {#if activeTab === 'all'}
        {#if onlineBuddies.length > 0}
          <div class="group-header">Online</div>
          {#each onlineBuddies as buddy}
            <button class="buddy-row" onclick={() => {
              const rid = findRoomForBuddy(buddy)
              if (rid) openChat(rid)
            }}>
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
            <button class="buddy-row offline" onclick={() => {
              const rid = findRoomForBuddy(buddy)
              if (rid) openChat(rid)
            }}>
              <span class="status-dot"></span>
              {buddy.display_name}
              {#if getUnreadForBuddy(buddy) > 0}
                <span class="unread-badge">{getUnreadForBuddy(buddy)}</span>
              {/if}
            </button>
          {/each}
        {/if}
        {#if $buddyList.length === 0}
          <p class="empty-text">No contacts yet</p>
        {/if}
      {:else}
        <!-- Rooms tab -->
        {#each filteredRooms as room}
          <button class="buddy-row" onclick={() => openChat(room.room_id)}>
            {room.name}
            {#if $unreadCounts[room.room_id] > 0}
              <span class="unread-badge">{$unreadCounts[room.room_id]}</span>
            {/if}
          </button>
        {/each}
        {#if filteredRooms.length === 0}
          <p class="empty-text">No rooms</p>
        {/if}
      {/if}
    </div>
  </div>

  <!-- Bottom toolbar -->
  <div class="buddy-toolbar">
    <StatusPicker />
  </div>
</div>

<style>
  .buddy-list-window {
    width: 220px;
    height: 500px;
    display: flex;
    flex-direction: column;
  }
  .buddy-list-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 0;
  }
  menu[role="tablist"] {
    margin: 0;
    padding: 2px 4px 0;
  }
  .buddy-scroll {
    flex: 1;
    overflow-y: auto;
    background: white;
    border: 2px inset #c0c0c0;
    margin: 0 4px 4px;
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
    padding: 4px;
    border-top: 1px solid #808080;
  }
</style>
