<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { buddyList, rooms, unreadCounts, isLoggedIn, currentUserId, currentStatus, syncing } from '../lib/stores'
  import { getBuddyList, getRooms, matrixLogout, matrixDisconnect, tryRestoreSession, leaveRoom, removeBuddy, getPendingInvites, acceptInvite, rejectInvite, setDockBadge } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import type { Buddy, Message, InviteInfo } from '../lib/types'
  import StatusPicker from './StatusPicker.svelte'
  import TitleBar from './TitleBar.svelte'
  import { openPreferencesWindow, openDirectMessageWindow, openChatRoomWindow, openServerLogWindow, openUserInfoWindow, openRoomInfoWindow, openFindUserWindow, openJoinRoomWindow } from '../lib/windows'

  let pendingInvites = $state<InviteInfo[]>([])

  // Track rooms with focused, scrolled-to-bottom windows — no badges for these
  let visibleRooms = new Set<string>()

  const isOffline = $derived($currentStatus === 'offline')
  const presenceAvailable = $derived($buddyList.some(b => b.presence !== 'unknown'))
  const onlineBuddies = $derived($buddyList.filter(b => b.presence !== 'offline' && b.presence !== 'unknown'))
  const offlineBuddies = $derived($buddyList.filter(b => b.presence === 'offline'))
  const groupRooms = $derived($rooms.filter(r => !r.is_direct))

  // Update dock badge when unread counts change
  const totalUnread = $derived(Object.values($unreadCounts).reduce((a, b) => a + b, 0))
  $effect(() => {
    setDockBadge(totalUnread).catch(() => {})
  })

  let refreshing = false
  async function refreshLists() {
    if (refreshing) return
    refreshing = true
    syncing.set(true)
    try {
      const fetchedBuddies = await getBuddyList()
      buddyList.set(fetchedBuddies)
      const fetchedRooms = await getRooms()
      rooms.set(fetchedRooms)
      const invites = await getPendingInvites().catch(() => [])
      pendingInvites = invites
    } catch (e) {
      console.error('Failed to load buddy list:', e)
    } finally {
      refreshing = false
      syncing.set(false)
    }
  }

  onMount(async () => {
    await refreshLists()

    await listen<Message>('new_message', (event) => {
      const roomId = event.payload.room_id
      if (roomId && !visibleRooms.has(roomId)) {
        unreadCounts.update(counts => ({
          ...counts,
          [roomId]: (counts[roomId] || 0) + 1,
        }))
        // Desktop notification only for non-visible rooms
        if (Notification.permission === 'granted') {
          const senderName = event.payload.sender_name || 'Someone'
          const body = event.payload.body?.slice(0, 100) || 'New message'
          new Notification(senderName, { body, tag: roomId })
        }
      }
    })

    // Track which rooms have focused, scrolled-to-bottom windows
    await listen<{ room_id: string; visible: boolean }>('room_visible', (event) => {
      if (event.payload.visible) {
        visibleRooms.add(event.payload.room_id)
      } else {
        visibleRooms.delete(event.payload.room_id)
      }
    })

    // Request notification permission
    if (Notification.permission === 'default') {
      Notification.requestPermission()
    }

    await listen('rooms_changed', () => {
      refreshLists()
    })

    await listen<string>('sync_status', (event) => {
      if (event.payload === 'synced') {
        refreshLists()
      }
    })

    await listen<InviteInfo>('room_invite', (event) => {
      // Add to pending invites if not already there
      if (!pendingInvites.find(i => i.room_id === event.payload.room_id)) {
        pendingInvites = [...pendingInvites, event.payload]
      }
    })

    await listen<{ room_id: string }>('clear_unread', (event) => {
      const roomId = event.payload.room_id
      if (roomId) {
        unreadCounts.update(counts => {
          const { [roomId]: _, ...rest } = counts
          return rest
        })
      }
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
    syncing.set(true)
    try {
      await tryRestoreSession()
      await invoke('start_sync')
    } catch (e) {
      syncing.set(false)
      console.error('Reconnect failed:', e)
    }
  }

  let contextMenu = $state<{ x: number; y: number; buddy?: Buddy; room?: { room_id: string; name: string } } | null>(null)

  function handleBuddyContext(e: MouseEvent, buddy: Buddy) {
    e.preventDefault()
    contextMenu = { x: e.clientX, y: e.clientY, buddy }
  }

  function handleRoomContext(e: MouseEvent, room: { room_id: string; name: string }) {
    e.preventDefault()
    contextMenu = { x: e.clientX, y: e.clientY, room }
  }

  function closeContextMenu() {
    contextMenu = null
  }

  function handleContextMessage() {
    if (contextMenu?.buddy) {
      openBuddyChat(contextMenu.buddy)
    }
    contextMenu = null
  }

  function handleContextUserInfo() {
    if (contextMenu?.buddy) {
      openUserInfoWindow(contextMenu.buddy.user_id, contextMenu.buddy.display_name)
    }
    contextMenu = null
  }

  function handleContextRoomInfo() {
    if (contextMenu?.room) {
      openRoomInfoWindow(contextMenu.room.room_id, contextMenu.room.name)
    }
    contextMenu = null
  }

  async function handleContextRemoveBuddy() {
    if (!contextMenu?.buddy) return
    const buddy = contextMenu.buddy
    contextMenu = null
    try {
      await removeBuddy(buddy.user_id)
      await refreshLists()
    } catch (e) {
      console.error('Remove buddy failed:', e)
    }
  }

  async function handleContextLeaveRoom() {
    if (!contextMenu?.room) return
    const room = contextMenu.room
    contextMenu = null
    try {
      await leaveRoom(room.room_id)
      await refreshLists()
    } catch (e) {
      console.error('Leave room failed:', e)
    }
  }

  async function handleAcceptInvite(invite: InviteInfo) {
    try {
      const room = await acceptInvite(invite.room_id)
      pendingInvites = pendingInvites.filter(i => i.room_id !== invite.room_id)
      if (room.is_direct) {
        openDirectMessageWindow(room.room_id, room.name)
      } else {
        openChatRoomWindow(room.room_id, room.name)
      }
      await refreshLists()
    } catch (e) {
      console.error('Failed to accept invite:', e)
    }
  }

  async function handleRejectInvite(invite: InviteInfo) {
    try {
      await rejectInvite(invite.room_id)
      pendingInvites = pendingInvites.filter(i => i.room_id !== invite.room_id)
    } catch (e) {
      console.error('Failed to reject invite:', e)
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
    <div class="buddy-actions">
      <button onclick={openFindUserWindow}>Find Users</button>
      <button onclick={openJoinRoomWindow}>Join Room</button>
    </div>
    <div class="buddy-scroll" class:disconnected={isOffline}>
      {#if presenceAvailable}
        {#if onlineBuddies.length > 0}
          <div class="group-header">Online</div>
          {#each onlineBuddies as buddy}
            <button class="buddy-row" onclick={() => openBuddyChat(buddy)} oncontextmenu={(e: MouseEvent) => handleBuddyContext(e, buddy)}>
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
            <button class="buddy-row offline" onclick={() => openBuddyChat(buddy)} oncontextmenu={(e: MouseEvent) => handleBuddyContext(e, buddy)}>
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
          <button class="buddy-row" onclick={() => openBuddyChat(buddy)} oncontextmenu={(e: MouseEvent) => handleBuddyContext(e, buddy)}>
            <span class="status-dot online"></span>
            {buddy.display_name}
            {#if getUnreadForBuddy(buddy) > 0}
              <span class="unread-badge">{getUnreadForBuddy(buddy)}</span>
            {/if}
          </button>
        {/each}
      {/if}
      {#if pendingInvites.length > 0}
        <div class="group-header">Invitations</div>
        {#each pendingInvites as invite}
          <div class="invite-row">
            <span class="invite-name">{invite.room_name || invite.room_id}</span>
            {#if invite.inviter_name}
              <span class="invite-from">from {invite.inviter_name}</span>
            {/if}
            <div class="invite-actions">
              <button class="invite-btn accept" onclick={() => handleAcceptInvite(invite)}>Join</button>
              <button class="invite-btn reject" onclick={() => handleRejectInvite(invite)}>X</button>
            </div>
          </div>
        {/each}
      {/if}
      {#if groupRooms.length > 0}
        <div class="group-header">Rooms</div>
        {#each groupRooms as room}
          <button class="buddy-row" onclick={() => openRoomChat(room)} oncontextmenu={(e: MouseEvent) => handleRoomContext(e, room)}>
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

  <!-- Context menu -->
  {#if contextMenu}
    <div class="context-overlay" onclick={closeContextMenu} onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') closeContextMenu() }} role="presentation">
    </div>
    <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
      {#if contextMenu.buddy}
        <button class="context-item" onclick={handleContextMessage}>Message</button>
        <button class="context-item" onclick={handleContextUserInfo}>User Info</button>
        <div class="context-separator"></div>
        <button class="context-item danger" onclick={handleContextRemoveBuddy}>Remove</button>
      {:else if contextMenu.room}
        <button class="context-item" onclick={handleContextRoomInfo}>Room Info</button>
        <div class="context-separator"></div>
        <button class="context-item danger" onclick={handleContextLeaveRoom}>Leave Room</button>
      {/if}
    </div>
  {/if}

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
  .buddy-actions {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    padding: 3px 4px;
    border-bottom: 1px solid #808080;
  }
  .buddy-actions button {
    border: none;
    background: transparent;
    box-shadow: none;
    padding: 2px 6px;
    font-size: 11px;
    cursor: pointer;
    color: #000;
  }
  .buddy-actions button:hover {
    text-decoration: underline;
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
    box-shadow: none;
    background: transparent;
    padding: 2px 12px;
    text-align: left;
    cursor: pointer;
    font-size: 11px;
    color: #000;
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
  .context-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 99;
  }
  .context-menu {
    position: fixed;
    z-index: 100;
    background: #c0c0c0;
    border: 2px outset #c0c0c0;
    padding: 2px;
    min-width: 100px;
  }
  .context-item {
    display: block;
    width: 100%;
    border: none;
    background: transparent;
    padding: 2px 16px;
    text-align: left;
    font-size: 11px;
    cursor: pointer;
  }
  .context-item:hover {
    background: #000080;
    color: white;
  }
  .context-item.danger {
    color: #cc0000;
  }
  .context-item.danger:hover {
    background: #cc0000;
    color: white;
  }
  .context-separator {
    height: 1px;
    background: #808080;
    margin: 2px 4px;
  }
  .buddy-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
    border-top: 1px solid #808080;
  }
  .buddy-toolbar button {
    border: none;
    background: transparent;
    box-shadow: none;
    padding: 2px 6px;
    font-size: 11px;
    cursor: pointer;
    color: #000;
  }
  .buddy-toolbar button:hover {
    text-decoration: underline;
  }
  .invite-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 12px;
    font-size: 11px;
    flex-wrap: wrap;
  }
  .invite-name {
    font-weight: bold;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .invite-from {
    color: #666;
    font-size: 10px;
  }
  .invite-actions {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }
  .invite-btn {
    font-size: 10px;
    padding: 0 6px;
    line-height: 16px;
    cursor: pointer;
  }
  .invite-btn.accept {
    background: #00cc00;
    color: white;
    border: 1px solid #009900;
  }
  .invite-btn.reject {
    background: #cc0000;
    color: white;
    border: 1px solid #990000;
  }
</style>
