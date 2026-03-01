<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { buddyList, rooms, spaces, unreadCounts, isLoggedIn, currentUserId, currentStatus, syncing, spaceCollapseState, roomTags } from '../lib/stores'
  import { getBuddyList, getRooms, getSpaces, matrixLogout, matrixDisconnect, tryRestoreSession, leaveRoom, removeBuddy, getPendingInvites, acceptInvite, rejectInvite, setDockBadge, getRoomTags, setRoomTag, removeRoomTag } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import type { Buddy, Room, Message, InviteInfo } from '../lib/types'
  import StatusPicker from './StatusPicker.svelte'
  import TitleBar from './TitleBar.svelte'
  import { openPreferencesWindow, openDirectMessageWindow, openChatRoomWindow, openServerLogWindow, openUserInfoWindow, openRoomInfoWindow, openFindUserWindow, openJoinRoomWindow, openBrowseSpacesWindow, openBrowseSpaceWindow } from '../lib/windows'

  let pendingInvites = $state<InviteInfo[]>([])

  const isOffline = $derived($currentStatus === 'offline')
  const presenceAvailable = $derived($buddyList.some(b => b.presence !== 'unknown'))
  const onlineBuddies = $derived($buddyList.filter(b => b.presence !== 'offline' && b.presence !== 'unknown'))
  const offlineBuddies = $derived($buddyList.filter(b => b.presence === 'offline'))
  const spacedRoomIds = $derived(new Set($spaces.flatMap(s => s.child_room_ids)))
  const tagGroups = $derived([...new Set(Object.values($roomTags).flat())].sort())
  const taggedRoomIds = $derived(new Set(Object.keys($roomTags).filter(id => $roomTags[id].length > 0)))
  const ungroupedRooms = $derived($rooms.filter(r => !r.is_direct && !spacedRoomIds.has(r.room_id) && !taggedRoomIds.has(r.room_id)))

  function getSpaceRooms(childIds: string[]): Room[] {
    return childIds
      .map(id => $rooms.find(r => r.room_id === id))
      .filter((r): r is Room => r !== undefined)
  }

  function getTagRooms(tag: string): Room[] {
    return Object.entries($roomTags)
      .filter(([_, tags]) => tags.includes(tag))
      .map(([roomId]) => $rooms.find(r => r.room_id === roomId))
      .filter((r): r is Room => r !== undefined)
  }

  function getRoomTag(roomId: string): string | null {
    const tags = $roomTags[roomId]
    return tags && tags.length > 0 ? tags[0] : null
  }

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
      const fetchedSpaces = await getSpaces()
      spaces.set(fetchedSpaces)
      const fetchedTags = await getRoomTags().catch(() => ({}))
      roomTags.set(fetchedTags)
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
      if (roomId) {
        unreadCounts.update(counts => ({
          ...counts,
          [roomId]: (counts[roomId] || 0) + 1,
        }))
        // Desktop notification
        if (Notification.permission === 'granted') {
          const senderName = event.payload.sender_name || 'Someone'
          const body = event.payload.body?.slice(0, 100) || 'New message'
          new Notification(senderName, { body, tag: roomId })
        }
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

  let groupPrompt = $state<{ room: { room_id: string; name: string }; value: string } | null>(null)

  function handleContextSetGroup() {
    if (!contextMenu?.room) return
    const room = contextMenu.room
    contextMenu = null
    groupPrompt = { room, value: '' }
  }

  async function submitGroupPrompt() {
    if (!groupPrompt || !groupPrompt.value.trim()) return
    const { room, value } = groupPrompt
    groupPrompt = null
    try {
      await setRoomTag(room.room_id, value.trim())
      const tags = await getRoomTags()
      roomTags.set(tags)
    } catch (e) {
      console.error('Set group failed:', e)
    }
  }

  async function handleContextRemoveGroup() {
    if (!contextMenu?.room) return
    const room = contextMenu.room
    const tag = getRoomTag(room.room_id)
    contextMenu = null
    if (!tag) return
    try {
      await removeRoomTag(room.room_id, tag)
      const tags = await getRoomTags()
      roomTags.set(tags)
    } catch (e) {
      console.error('Remove group failed:', e)
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
    spaces.set([])
    roomTags.set({})
    unreadCounts.set({})
  }
</script>

<div class="window buddy-list-window">
  <TitleBar title="ICQ26a" showMinimize />
  <div class="window-body">
    <div class="buddy-actions">
      <button onclick={openFindUserWindow}>Find Users</button>
      <button onclick={openJoinRoomWindow}>Join Room</button>
      <button onclick={openBrowseSpacesWindow}>Spaces</button>
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
      {#if $spaces.length > 0 || tagGroups.length > 0 || ungroupedRooms.length > 0}
        <div class="group-header">Rooms</div>
        <ul class="tree-view rooms-tree">
          {#each $spaces as space}
            <li>
              <details open={!$spaceCollapseState[space.room_id]} ontoggle={(e: Event) => {
                const details = e.target as HTMLDetailsElement;
                const isNowOpen = details.open;
                if (isNowOpen === !!$spaceCollapseState[space.room_id]) {
                  spaceCollapseState.toggle(space.room_id);
                }
              }}>
                <summary>
                  <span class="space-name">{space.name}</span>
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <button class="space-browse-btn" title="Browse space rooms" onclick={(e: MouseEvent) => { e.stopPropagation(); openBrowseSpaceWindow(space.room_id, space.name) }}>+</button>
                </summary>
                <ul>
                  {#each getSpaceRooms(space.child_room_ids) as room}
                    <li>
                      <button class="tree-room-btn" onclick={() => openRoomChat(room)} oncontextmenu={(e: MouseEvent) => handleRoomContext(e, room)}>
                        {room.name}
                        {#if $unreadCounts[room.room_id] > 0}
                          <span class="unread-badge">{$unreadCounts[room.room_id]}</span>
                        {/if}
                      </button>
                    </li>
                  {/each}
                </ul>
              </details>
            </li>
          {/each}
          {#each tagGroups as tag}
            <li class="tag-group">
              <details open={!$spaceCollapseState[`tag:${tag}`]} ontoggle={(e: Event) => {
                const details = e.target as HTMLDetailsElement;
                const isNowOpen = details.open;
                if (isNowOpen === !!$spaceCollapseState[`tag:${tag}`]) {
                  spaceCollapseState.toggle(`tag:${tag}`);
                }
              }}>
                <summary>{tag}</summary>
                <ul>
                  {#each getTagRooms(tag) as room}
                    <li>
                      <button class="tree-room-btn" onclick={() => openRoomChat(room)} oncontextmenu={(e: MouseEvent) => handleRoomContext(e, room)}>
                        {room.name}
                        {#if $unreadCounts[room.room_id] > 0}
                          <span class="unread-badge">{$unreadCounts[room.room_id]}</span>
                        {/if}
                      </button>
                    </li>
                  {/each}
                </ul>
              </details>
            </li>
          {/each}
          {#each ungroupedRooms as room}
            <li>
              <button class="tree-room-btn" onclick={() => openRoomChat(room)} oncontextmenu={(e: MouseEvent) => handleRoomContext(e, room)}>
                {room.name}
                {#if $unreadCounts[room.room_id] > 0}
                  <span class="unread-badge">{$unreadCounts[room.room_id]}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      {#if $buddyList.length === 0 && ungroupedRooms.length === 0 && $spaces.length === 0 && tagGroups.length === 0}
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
        {#if getRoomTag(contextMenu.room.room_id)}
          <button class="context-item" onclick={handleContextRemoveGroup}>Remove from Group</button>
        {:else if !spacedRoomIds.has(contextMenu.room.room_id)}
          <button class="context-item" onclick={handleContextSetGroup}>Set Group...</button>
        {/if}
        <button class="context-item danger" onclick={handleContextLeaveRoom}>Leave Room</button>
      {/if}
    </div>
  {/if}

  <!-- Group name prompt -->
  {#if groupPrompt}
    <div class="prompt-overlay" onclick={() => groupPrompt = null} role="presentation"></div>
    <div class="prompt-dialog window">
      <div class="title-bar">
        <div class="title-bar-text">Set Group</div>
      </div>
      <div class="window-body">
        <p style="margin: 0 0 4px; font-size: 11px;">Group name for "{groupPrompt.room.name}":</p>
        <input
          type="text"
          bind:value={groupPrompt.value}
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') submitGroupPrompt(); if (e.key === 'Escape') groupPrompt = null; }}
          style="width: 100%; box-sizing: border-box;"
          autofocus
        />
        <div style="display: flex; justify-content: flex-end; gap: 4px; margin-top: 8px;">
          <button onclick={submitGroupPrompt}>OK</button>
          <button onclick={() => groupPrompt = null}>Cancel</button>
        </div>
      </div>
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
  .rooms-tree {
    margin: 0 4px;
    padding: 4px;
    border: none;
    box-shadow: none;
    background: transparent;
  }
  /* Override 98.css's white cover-up block to match our gray background */
  .rooms-tree :global(ul > li:last-child::after) {
    background: #c0c0c0;
  }
  .rooms-tree :global(summary) {
    font-size: 11px;
    font-weight: bold;
    cursor: default;
    user-select: none;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .space-browse-btn {
    font-size: 10px;
    padding: 0 3px;
    min-width: 0;
    min-height: 0;
    line-height: 1;
    margin-left: auto;
  }
  .tag-group :global(> details > summary) {
    font-style: italic;
  }
  .tree-room-btn {
    display: flex;
    align-items: center;
    width: 100%;
    border: none;
    box-shadow: none;
    background: transparent;
    padding: 1px 4px;
    text-align: left;
    cursor: pointer;
    font-size: 11px;
    color: #000;
  }
  .tree-room-btn:hover {
    background: #000080;
    color: white;
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
  .prompt-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 199;
    background: rgba(0, 0, 0, 0.2);
  }
  .prompt-dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 200;
    width: 220px;
  }
  .prompt-dialog .window-body {
    padding: 8px;
  }
</style>
