<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomMessages, getRoomMembers, sendMessage, sendTyping, markAsRead, getRooms, createDmRoom, fetchMedia, editMessage, deleteMessage, sendReaction } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { emit } from '@tauri-apps/api/event'
  import { ask } from '@tauri-apps/plugin-dialog'
  import type { Message, Buddy, TypingEvent, MessageEditEvent, MessageDeletedEvent, ReactionEvent } from '../lib/types'
  import { openUserInfoWindow, openDirectMessageWindow, openRoomInfoWindow } from '../lib/windows'
  import { linkify } from '../lib/linkify'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    roomId: string
    roomName: string
  }
  let { roomId, roomName }: Props = $props()

  let messages = $state<Message[]>([])
  let members = $state<Buddy[]>([])
  let newMessage = $state('')
  let memberFilter = $state('')
  let loading = $state(true)
  let loadingOlder = $state(false)
  let endToken = $state<string | null>(null)
  let messagesDiv = $state<HTMLDivElement | undefined>(undefined)
  let myUserId = $state<string | null>(null)
  let showNewMsgHint = $state(false)

  // Edit state
  let editingMsg = $state<Message | null>(null)

  // Reactions state: event_id -> { key -> Set<sender_name> }
  let reactions = $state<Record<string, Record<string, Set<string>>>>({})

  // Typing indicator state
  let typingUsers = $state<string[]>([])
  let typingTimeout: ReturnType<typeof setTimeout> | null = null
  const typingText = $derived(
    typingUsers.length === 0 ? '' :
    typingUsers.length === 1 ? `${typingUsers[0]} is typing...` :
    `${typingUsers.join(', ')} are typing...`
  )

  // Reply state
  let replyTo = $state<Message | null>(null)

  // Context menus
  let contextMenu = $state<{ x: number; y: number; member: Buddy } | null>(null)
  let msgContextMenu = $state<{ x: number; y: number; msg: Message } | null>(null)

  let unlisteners: (() => void)[] = []
  let windowFocused = $state(true)

  // Sort members by most recent message, then filter by search term
  const sortedFilteredMembers = $derived.by(() => {
    const lastActive = new Map<string, number>()
    for (const msg of messages) {
      const existing = lastActive.get(msg.sender) ?? 0
      if (msg.timestamp > existing) {
        lastActive.set(msg.sender, msg.timestamp)
      }
    }

    const sorted = [...members].sort((a, b) => {
      const aTime = lastActive.get(a.user_id) ?? 0
      const bTime = lastActive.get(b.user_id) ?? 0
      if (aTime !== bTime) return bTime - aTime
      return a.display_name.localeCompare(b.display_name)
    })

    if (!memberFilter.trim()) return sorted
    const q = memberFilter.trim().toLowerCase()
    return sorted.filter(m =>
      m.display_name.toLowerCase().includes(q) || m.user_id.toLowerCase().includes(q)
    )
  })

  onMount(async () => {
    myUserId = await invoke<string>('try_restore_session').catch(() => null)
    if (roomId) {
      loading = true
      try {
        const [page, mems] = await Promise.all([
          getRoomMessages(roomId, 50),
          getRoomMembers(roomId),
        ])
        messages = page.messages
        endToken = page.end_token
        members = mems
      } catch (e) {
        console.error('Failed to load room data:', e)
      } finally {
        loading = false
        scrollToBottom()
      }
    }

    // Listen for new messages
    unlisteners.push(await listen<Message>('new_message', (event) => {
      if (event.payload.room_id === roomId && event.payload.sender !== '') {
        messages = [...messages, event.payload]
        if (isNearBottom()) {
          scrollToBottom()
        } else {
          showNewMsgHint = true
        }
        // Send read receipt if window is focused
        if (windowFocused && event.payload.event_id) {
          markAsRead(roomId, event.payload.event_id).catch(() => {})
          emit('clear_unread', { room_id: roomId })
        }
      }
    }))

    // Listen for message edits
    unlisteners.push(await listen<MessageEditEvent>('message_edited', (event) => {
      if (event.payload.room_id === roomId) {
        messages = messages.map(msg =>
          msg.event_id === event.payload.original_event_id
            ? { ...msg, body: event.payload.new_body }
            : msg
        )
      }
    }))

    // Listen for typing events
    unlisteners.push(await listen<TypingEvent>('typing', (event) => {
      if (event.payload.room_id === roomId) {
        typingUsers = event.payload.display_names
      }
    }))

    // Listen for message deletions
    unlisteners.push(await listen<MessageDeletedEvent>('message_deleted', (event) => {
      if (event.payload.room_id === roomId) {
        messages = messages.filter(msg => msg.event_id !== event.payload.event_id)
      }
    }))

    // Listen for reactions
    unlisteners.push(await listen<ReactionEvent>('reaction', (event) => {
      if (event.payload.room_id === roomId) {
        const eventId = event.payload.relates_to
        const key = event.payload.reaction_key
        const sender = event.payload.sender_name
        reactions = { ...reactions }
        if (!reactions[eventId]) reactions[eventId] = {}
        if (!reactions[eventId][key]) reactions[eventId][key] = new Set()
        reactions[eventId][key].add(sender)
      }
    }))

    // Track window focus for read receipts
    const appWindow = getCurrentWindow()
    const unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
      windowFocused = focused
      if (focused && messages.length > 0) {
        const lastMsg = messages[messages.length - 1]
        if (lastMsg.event_id) {
          markAsRead(roomId, lastMsg.event_id).catch(() => {})
          emit('clear_unread', { room_id: roomId })
        }
      }
    })
    unlisteners.push(unlistenFocus)

    // Mark as read on initial load if focused
    if (windowFocused && messages.length > 0) {
      const lastMsg = messages[messages.length - 1]
      if (lastMsg.event_id) {
        markAsRead(roomId, lastMsg.event_id).catch(() => {})
        emit('clear_unread', { room_id: roomId })
      }
    }
  })

  onDestroy(() => {
    for (const fn of unlisteners) fn()
    if (typingTimeout) clearTimeout(typingTimeout)
  })

  async function loadOlderMessages() {
    if (!roomId || !endToken || loadingOlder) return
    loadingOlder = true
    try {
      const el = messagesDiv!
      const prevHeight = el.scrollHeight
      const page = await getRoomMessages(roomId, 50, endToken)
      if (page.messages.length > 0) {
        messages = [...page.messages, ...messages]
        endToken = page.end_token
        await tick()
        el.scrollTop = el.scrollHeight - prevHeight
      } else {
        endToken = null
      }
    } catch (e) {
      console.error('Failed to load older messages:', e)
    } finally {
      loadingOlder = false
    }
  }

  function isNearBottom(): boolean {
    if (!messagesDiv) return true
    return messagesDiv.scrollHeight - messagesDiv.scrollTop - messagesDiv.clientHeight < 60
  }

  function handleScroll() {
    if (messagesDiv && messagesDiv.scrollTop < 50 && endToken && !loadingOlder) {
      loadOlderMessages()
    }
    if (isNearBottom()) {
      showNewMsgHint = false
    }
  }

  function jumpToBottom() {
    showNewMsgHint = false
    scrollToBottom()
  }

  function scrollToBottom() {
    setTimeout(() => {
      if (messagesDiv) messagesDiv.scrollTop = messagesDiv.scrollHeight
    }, 0)
  }

  function handleTypingInput() {
    sendTyping(roomId, true).catch(() => {})
    if (typingTimeout) clearTimeout(typingTimeout)
    typingTimeout = setTimeout(() => {
      sendTyping(roomId, false).catch(() => {})
      typingTimeout = null
    }, 3000)
  }

  async function handleSend() {
    if (!newMessage.trim() || !roomId) return
    const body = newMessage
    const replyEventId = replyTo?.event_id
    const editing = editingMsg
    newMessage = ''
    replyTo = null
    editingMsg = null
    if (typingTimeout) {
      clearTimeout(typingTimeout)
      typingTimeout = null
    }
    sendTyping(roomId, false).catch(() => {})
    try {
      if (editing) {
        await editMessage(roomId, editing.event_id, body)
        messages = messages.map(m => m.event_id === editing.event_id ? { ...m, body } : m)
      } else {
        await sendMessage(roomId, body, replyEventId ?? undefined)
      }
    } catch (e) {
      console.error('Failed to send:', e)
      newMessage = body
    }
  }

  async function handleAttach() {
    if (!roomId) return
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const file = await open({ multiple: false })
      if (file) {
        await invoke('upload_file', { roomId, filePath: file })
      }
    } catch (e) {
      console.error('Failed to attach file:', e)
    }
  }

  // Message context menu
  function handleMsgContext(e: MouseEvent, msg: Message) {
    e.preventDefault()
    msgContextMenu = { x: e.clientX, y: e.clientY, msg }
  }

  function closeMsgContextMenu() {
    msgContextMenu = null
  }

  function handleMsgReply() {
    if (!msgContextMenu) return
    replyTo = msgContextMenu.msg
    msgContextMenu = null
  }

  function handleMsgEdit() {
    if (!msgContextMenu) return
    editingMsg = msgContextMenu.msg
    newMessage = msgContextMenu.msg.body
    msgContextMenu = null
  }

  async function handleMsgDelete() {
    if (!msgContextMenu) return
    const msg = msgContextMenu.msg
    msgContextMenu = null
    try {
      await deleteMessage(roomId, msg.event_id)
      messages = messages.filter(m => m.event_id !== msg.event_id)
    } catch (e) {
      console.error('Failed to delete:', e)
    }
  }

  function cancelEdit() {
    editingMsg = null
    newMessage = ''
  }

  function cancelReply() {
    replyTo = null
  }

  async function handleReaction(eventId: string, key: string) {
    try {
      await sendReaction(roomId, eventId, key)
    } catch (e) {
      console.error('Failed to react:', e)
    }
  }

  // Member context menu
  function handleMemberContext(e: MouseEvent, member: Buddy) {
    e.preventDefault()
    contextMenu = { x: e.clientX, y: e.clientY, member }
  }

  function closeContextMenu() {
    contextMenu = null
  }

  async function handleContextMessage() {
    if (!contextMenu) return
    const member = contextMenu.member
    contextMenu = null
    try {
      const allRooms = await getRooms()
      const dmRoom = allRooms.find(r => r.is_direct && r.name === member.display_name)
      if (dmRoom) {
        openDirectMessageWindow(dmRoom.room_id, dmRoom.name)
      } else {
        const confirmed = await ask(
          `Start a new conversation with ${member.display_name}? They will be notified.`,
          { title: 'New Message', kind: 'info' },
        )
        if (confirmed) {
          const newRoom = await createDmRoom(member.user_id)
          openDirectMessageWindow(newRoom.room_id, newRoom.name)
        }
      }
    } catch (e) {
      console.error('Failed to open DM:', e)
    }
  }

  function handleContextUserInfo() {
    if (!contextMenu) return
    openUserInfoWindow(contextMenu.member.user_id, contextMenu.member.display_name)
    contextMenu = null
  }

  function loadMedia(node: HTMLImageElement, mxcUrl: string) {
    fetchMedia(mxcUrl).then(dataUrl => { node.src = dataUrl }).catch(() => {
      node.alt = 'Failed to load image'
    })
    return {
      update(newUrl: string) {
        fetchMedia(newUrl).then(dataUrl => { node.src = dataUrl }).catch(() => {})
      }
    }
  }

  function downloadFile(node: HTMLAnchorElement, params: { mxcUrl: string; filename: string }) {
    let current = params
    node.href = '#'
    node.onclick = async (e) => {
      e.preventDefault()
      e.stopPropagation()
      try {
        const dataUrl = await fetchMedia(current.mxcUrl)
        const a = document.createElement('a')
        a.href = dataUrl
        a.download = current.filename
        document.body.appendChild(a)
        a.click()
        document.body.removeChild(a)
      } catch {
        console.error('Failed to download file')
      }
    }
    return {
      update(newParams: { mxcUrl: string; filename: string }) {
        current = newParams
      }
    }
  }

  function closeWindow() {
    getCurrentWindow().close()
  }

  function formatTime(ts: number): string {
    const d = new Date(ts * 1000)
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  }
</script>

<div class="window chat-window">
  <TitleBar title="ICQ Chat - {roomName}" onclose={closeWindow} />

  <div class="window-body chat-body">
    <div class="chat-main">
      <!-- Messages pane -->
      <div class="messages-wrap">
        <div class="chat-messages" bind:this={messagesDiv} onscroll={handleScroll}>
          {#if loadingOlder}
            <p class="loading-text">Loading older messages...</p>
          {/if}
          {#if loading}
            <p class="loading-text">Loading...</p>
          {:else}
            {#each messages as msg}
              <div class="chat-message" role="article" oncontextmenu={(e: MouseEvent) => handleMsgContext(e, msg)}>
                {#if msg.in_reply_to && (msg.reply_sender_name || msg.reply_body)}
                  <div class="reply-quote">
                    {#if msg.reply_sender_name}<span class="reply-quote-sender">{msg.reply_sender_name}</span>{/if}
                    {#if msg.reply_body}<span class="reply-quote-body">{msg.reply_body.length > 80 ? msg.reply_body.slice(0, 80) + '...' : msg.reply_body}</span>{/if}
                  </div>
                {/if}
                <div class="chat-message-header">
                  <span class="chat-sender">{msg.sender_name}</span>
                  <span class="chat-time">{formatTime(msg.timestamp)}</span>
                </div>
                {#if msg.msg_type === 'image' && msg.media_url}
                  <div class="chat-message-body"><img class="message-image" use:loadMedia={msg.media_url} alt={msg.filename || msg.body} /></div>
                {:else if (msg.msg_type === 'file' || msg.msg_type === 'audio' || msg.msg_type === 'video') && msg.media_url}
                  <div class="chat-message-body"><a href="#download" class="message-file" role="button" use:downloadFile={{ mxcUrl: msg.media_url, filename: msg.filename || msg.body }}>{msg.filename || msg.body}</a></div>
                {:else}
                  <div class="chat-message-body">{@html linkify(msg.body)}</div>
                {/if}
                {#if reactions[msg.event_id]}
                  <div class="reactions-row">
                    {#each Object.entries(reactions[msg.event_id]) as [key, senders]}
                      <button class="reaction-badge" onclick={() => handleReaction(msg.event_id, key)} title={[...senders].join(', ')}>
                        {key} {senders.size}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
        {#if showNewMsgHint}
          <button class="new-msg-hint" onclick={jumpToBottom}>New messages below</button>
        {/if}
      </div>

      <!-- Typing indicator -->
      {#if typingText}
        <div class="typing-indicator">{typingText}</div>
      {/if}

      <!-- Reply preview -->
      {#if replyTo}
        <div class="reply-preview">
          <span class="reply-preview-text">Reply to <b>{replyTo.sender_name}</b>: {replyTo.body.length > 60 ? replyTo.body.slice(0, 60) + '...' : replyTo.body}</span>
          <button class="reply-preview-cancel" onclick={cancelReply}>X</button>
        </div>
      {/if}

      <!-- Edit preview -->
      {#if editingMsg}
        <div class="reply-preview">
          <span class="reply-preview-text">Editing: {editingMsg.body.length > 60 ? editingMsg.body.slice(0, 60) + '...' : editingMsg.body}</span>
          <button class="reply-preview-cancel" onclick={cancelEdit}>X</button>
        </div>
      {/if}

      <!-- Input area -->
      <div class="chat-input">
        <textarea
          bind:value={newMessage}
          oninput={handleTypingInput}
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend() }}}
          rows="3"
          placeholder="Type a message..."
        ></textarea>
        <button onclick={handleAttach}>Attach</button>
        <button onclick={handleSend}>Send</button>
      </div>
    </div>

    <!-- Members sidebar -->
    <div class="members-panel">
      <div class="panel-info-row">
        <button class="info-btn" onclick={() => openRoomInfoWindow(roomId, roomName)}>Info</button>
      </div>
      <div class="members-header">Participants ({members.length})</div>
      {#if members.length > 5}
        <div class="member-filter">
          <input type="text" bind:value={memberFilter} placeholder="Filter..." />
        </div>
      {/if}
      <div class="members-list">
        {#each sortedFilteredMembers as member}
          <button class="member-row clickable" oncontextmenu={(e: MouseEvent) => handleMemberContext(e, member)}>
            <span class="member-dot"></span>
            {member.display_name}
          </button>
        {/each}
      </div>
    </div>
  </div>

  <!-- Member context menu -->
  {#if contextMenu}
    <div class="context-overlay" onclick={closeContextMenu} onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') closeContextMenu() }} role="presentation">
    </div>
    <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
      <button class="context-item" onclick={handleContextMessage}>Message</button>
      <button class="context-item" onclick={handleContextUserInfo}>User Info</button>
    </div>
  {/if}

  <!-- Message context menu -->
  {#if msgContextMenu}
    <div class="context-overlay" onclick={closeMsgContextMenu} onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') closeMsgContextMenu() }} role="presentation">
    </div>
    <div class="context-menu" style="left: {msgContextMenu.x}px; top: {msgContextMenu.y}px;">
      <button class="context-item" onclick={handleMsgReply}>Reply</button>
      <button class="context-item" onclick={() => { const eid = msgContextMenu!.msg.event_id; closeMsgContextMenu(); handleReaction(eid, '\u{1F44D}') }}>React +1</button>
      {#if myUserId && msgContextMenu.msg.sender === myUserId}
        <div class="context-separator"></div>
        <button class="context-item" onclick={handleMsgEdit}>Edit</button>
        <button class="context-item danger" onclick={handleMsgDelete}>Delete</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .chat-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }
  .chat-body {
    flex: 1;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }
  .chat-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .messages-wrap {
    flex: 1;
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .chat-messages {
    flex: 1;
    overflow-y: auto;
    background: #000;
    color: #fff;
    padding: 4px;
    font-size: 11px;
    font-family: 'Courier New', monospace;
  }
  .new-msg-hint {
    position: absolute;
    bottom: 4px;
    left: 50%;
    transform: translateX(-50%);
    background: #003366;
    color: #66ccff;
    border: 1px solid #66ccff;
    padding: 2px 12px;
    font-size: 10px;
    cursor: pointer;
    z-index: 10;
    white-space: nowrap;
    font-family: 'Courier New', monospace;
  }
  .new-msg-hint:hover {
    background: #004488;
  }
  .chat-message {
    margin-bottom: 4px;
  }
  .chat-message-header {
    display: flex;
    justify-content: space-between;
  }
  .chat-sender {
    font-weight: bold;
    color: #00cccc;
  }
  .chat-time {
    color: #888;
    font-size: 10px;
  }
  .chat-message-body {
    padding-left: 8px;
  }
  .reply-quote {
    border-left: 3px solid #666;
    padding: 1px 6px;
    margin-bottom: 2px;
    font-size: 10px;
    color: #999;
    background: #1a1a1a;
  }
  .reply-quote-sender {
    font-weight: bold;
    color: #888;
    margin-right: 4px;
  }
  .typing-indicator {
    font-size: 10px;
    color: #888;
    padding: 1px 8px;
    font-style: italic;
    font-family: 'Courier New', monospace;
    background: #111;
  }
  .reply-preview {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: #1a1a2e;
    border: 1px solid #333;
    font-size: 10px;
    color: #ccc;
    font-family: 'Courier New', monospace;
  }
  .reply-preview-text {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reply-preview-cancel {
    font-size: 10px;
    padding: 0 4px;
    cursor: pointer;
    border: 1px solid #555;
    background: #333;
    color: #ccc;
    line-height: 14px;
  }
  .message-image {
    max-width: 200px;
    max-height: 200px;
    display: block;
    margin: 2px 0;
    cursor: pointer;
  }
  .message-file {
    color: #66ccff;
    text-decoration: underline;
  }
  .chat-message-body :global(a) {
    color: #66ccff;
    text-decoration: underline;
  }
  .chat-input {
    display: flex;
    gap: 4px;
    padding: 4px;
  }
  .chat-input textarea {
    flex: 1;
    resize: none;
  }
  .members-panel {
    width: 140px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid #808080;
  }
  .panel-info-row {
    padding: 4px 8px;
    background: #c0c0c0;
    border-bottom: 1px solid #808080;
  }
  .info-btn {
    font-size: 10px;
    padding: 1px 8px;
    width: 100%;
    border: none;
    box-shadow: none;
    background: transparent;
    cursor: pointer;
    color: #000;
    text-align: left;
  }
  .info-btn:hover {
    text-decoration: underline;
  }
  .members-header {
    font-weight: bold;
    font-size: 11px;
    padding: 4px 8px;
    background: #c0c0c0;
    border-bottom: 1px solid #808080;
  }
  .member-filter {
    padding: 3px 4px;
    background: #c0c0c0;
    border-bottom: 1px solid #808080;
  }
  .member-filter input {
    width: 100%;
    box-sizing: border-box;
    font-size: 10px;
    padding: 1px 3px;
  }
  .members-list {
    flex: 1;
    overflow-y: auto;
    background: white;
    font-size: 11px;
  }
  .member-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 1px 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
    border: none;
    box-shadow: none;
    background: transparent;
    text-align: left;
    font-size: 11px;
    color: #000;
  }
  .member-row.clickable {
    cursor: pointer;
  }
  .member-row.clickable:hover {
    background: #000080;
    color: white;
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
  .reactions-row {
    display: flex;
    gap: 3px;
    padding: 1px 0 1px 8px;
    flex-wrap: wrap;
  }
  .reaction-badge {
    font-size: 10px;
    padding: 0 4px;
    border: 1px solid #444;
    background: #222;
    color: #ccc;
    border-radius: 8px;
    cursor: pointer;
    line-height: 16px;
    font-family: 'Courier New', monospace;
  }
  .reaction-badge:hover {
    background: #333;
    border-color: #66ccff;
  }
  .member-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #999;
    flex-shrink: 0;
  }
  .loading-text {
    text-align: center;
    color: #888;
    padding: 20px;
  }
</style>
