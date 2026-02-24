<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomMessages, getRoomMembers, sendMessage, sendTyping, markAsRead, fetchMedia, editMessage, deleteMessage, sendReaction } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { emit } from '@tauri-apps/api/event'
  import type { Message, TypingEvent, MessageEditEvent, MessageDeletedEvent, ReactionEvent } from '../lib/types'
  import { openUserInfoWindow } from '../lib/windows'
  import { linkify } from '../lib/linkify'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    roomId: string
    roomName: string
  }
  let { roomId, roomName }: Props = $props()

  let messages = $state<Message[]>([])
  let newMessage = $state('')
  let loading = $state(true)
  let loadingOlder = $state(false)
  let endToken = $state<string | null>(null)
  let messagesDiv = $state<HTMLDivElement | undefined>(undefined)
  let dmUserId = $state<string | null>(null)
  let myUserId = $state<string | null>(null)
  let showNewMsgHint = $state(false)

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

  // Edit state
  let editingMsg = $state<Message | null>(null)

  // Reactions state: event_id -> { key -> Set<sender_name> }
  let reactions = $state<Record<string, Record<string, Set<string>>>>({})

  // Message context menu
  let msgContextMenu = $state<{ x: number; y: number; msg: Message } | null>(null)

  let unlisteners: (() => void)[] = []
  let windowFocused = $state(true)

  onMount(async () => {
    if (roomId) {
      await loadMessages()
      // Find the other user in this DM
      try {
        const members = await getRoomMembers(roomId)
        const myId = await invoke<string>('try_restore_session').catch(() => null)
        myUserId = myId
        const other = members.find(m => m.user_id !== myId)
        if (other) dmUserId = other.user_id
      } catch { /* ignore */ }
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

  async function loadMessages() {
    if (!roomId) return
    loading = true
    try {
      const page = await getRoomMessages(roomId, 50)
      messages = page.messages
      endToken = page.end_token
    } catch (e) {
      console.error('Failed to load messages:', e)
    } finally {
      loading = false
      scrollToBottom()
    }
  }

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
    // Stop typing indicator
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

  function handleMsgContext(e: MouseEvent, msg: Message) {
    e.preventDefault()
    msgContextMenu = { x: e.clientX, y: e.clientY, msg }
  }

  function closeMsgContextMenu() {
    msgContextMenu = null
  }

  function handleReply() {
    if (!msgContextMenu) return
    replyTo = msgContextMenu.msg
    msgContextMenu = null
  }

  function handleEdit() {
    if (!msgContextMenu) return
    editingMsg = msgContextMenu.msg
    newMessage = msgContextMenu.msg.body
    msgContextMenu = null
  }

  async function handleDelete() {
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
</script>

<div class="window dm-window">
  <TitleBar title="{roomName} - Message Session" onclose={closeWindow} />
  <div class="window-body">
    <!-- Header fields -->
    <div class="dm-header">
      <div class="field-row">
        <label>To:</label>
        <span>{roomName}</span>
        {#if dmUserId}
          <button class="info-btn" onclick={() => openUserInfoWindow(dmUserId!, roomName)}>Info</button>
        {/if}
      </div>
    </div>

    <!-- Messages area -->
    <div class="messages-wrap">
      <div class="messages-area" bind:this={messagesDiv} onscroll={handleScroll}>
        {#if loadingOlder}
          <p class="loading-text">Loading older messages...</p>
        {/if}
        {#if loading}
          <p class="loading-text">Loading messages...</p>
        {:else if messages.length === 0}
          <p class="empty-text">No messages yet</p>
        {:else}
          {#each messages as msg}
            <div class="message" oncontextmenu={(e: MouseEvent) => handleMsgContext(e, msg)}>
              {#if msg.in_reply_to && (msg.reply_sender_name || msg.reply_body)}
                <div class="reply-quote">
                  {#if msg.reply_sender_name}<span class="reply-quote-sender">{msg.reply_sender_name}</span>{/if}
                  {#if msg.reply_body}<span class="reply-quote-body">{msg.reply_body.length > 80 ? msg.reply_body.slice(0, 80) + '...' : msg.reply_body}</span>{/if}
                </div>
              {/if}
              <span class="message-sender">{msg.sender_name}:</span>
              {#if msg.msg_type === 'image' && msg.media_url}
                <span class="message-body"><img class="message-image" use:loadMedia={msg.media_url} alt={msg.filename || msg.body} /></span>
              {:else if (msg.msg_type === 'file' || msg.msg_type === 'audio' || msg.msg_type === 'video') && msg.media_url}
                <span class="message-body"><a class="message-file" use:downloadFile={{ mxcUrl: msg.media_url, filename: msg.filename || msg.body }}>{msg.filename || msg.body}</a></span>
              {:else}
                <span class="message-body">{@html linkify(msg.body)}</span>
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
    <div class="dm-input">
      <label for="msg-input">Enter Message:</label>
      <textarea
        id="msg-input"
        bind:value={newMessage}
        oninput={handleTypingInput}
        onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend() }}}
        rows="3"
      ></textarea>
    </div>

    <!-- Buttons -->
    <div class="dm-buttons">
      <button onclick={closeWindow}>Cancel</button>
      <button onclick={handleAttach}>Attach</button>
      <button onclick={handleSend}>Send</button>
    </div>
  </div>
</div>

<!-- Message context menu -->
{#if msgContextMenu}
  <div class="context-overlay" onclick={closeMsgContextMenu} onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') closeMsgContextMenu() }} role="presentation">
  </div>
  <div class="context-menu" style="left: {msgContextMenu.x}px; top: {msgContextMenu.y}px;">
    <button class="context-item" onclick={handleReply}>Reply</button>
    <button class="context-item" onclick={() => { const eid = msgContextMenu!.msg.event_id; closeMsgContextMenu(); handleReaction(eid, '\u{1F44D}') }}>React +1</button>
    {#if myUserId && msgContextMenu.msg.sender === myUserId}
      <div class="context-separator"></div>
      <button class="context-item" onclick={handleEdit}>Edit</button>
      <button class="context-item danger" onclick={handleDelete}>Delete</button>
    {/if}
  </div>
{/if}

<style>
  .dm-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }
  .dm-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .dm-header {
    padding: 0 4px;
  }
  .dm-header .field-row {
    display: flex;
    align-items: center;
  }
  .dm-header .field-row label {
    font-weight: bold;
    min-width: 30px;
  }
  .info-btn {
    margin-left: auto;
    font-size: 10px;
    padding: 1px 8px;
  }
  .messages-wrap {
    flex: 1;
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .messages-area {
    flex: 1;
    overflow-y: auto;
    background: white;
    border: 2px inset #c0c0c0;
    padding: 4px;
    font-size: 11px;
  }
  .new-msg-hint {
    position: absolute;
    bottom: 4px;
    left: 50%;
    transform: translateX(-50%);
    background: #000080;
    color: white;
    border: 1px solid #c0c0c0;
    padding: 2px 12px;
    font-size: 10px;
    cursor: pointer;
    z-index: 10;
    white-space: nowrap;
  }
  .new-msg-hint:hover {
    background: #0000cc;
  }
  .message {
    margin-bottom: 2px;
  }
  .message-sender {
    font-weight: bold;
    color: #000080;
  }
  .message-image {
    max-width: 200px;
    max-height: 200px;
    display: block;
    margin: 2px 0;
    cursor: pointer;
  }
  .message-file {
    color: #0000ee;
    text-decoration: underline;
  }
  .message-body :global(a) {
    color: #0000ee;
    text-decoration: underline;
  }
  .reply-quote {
    border-left: 3px solid #808080;
    padding: 1px 6px;
    margin-bottom: 1px;
    font-size: 10px;
    color: #666;
    background: #f0f0f0;
  }
  .reply-quote-sender {
    font-weight: bold;
    margin-right: 4px;
  }
  .typing-indicator {
    font-size: 10px;
    color: #666;
    padding: 1px 8px;
    font-style: italic;
  }
  .reply-preview {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: #e8e8ff;
    border: 1px solid #c0c0d0;
    font-size: 10px;
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
    border: 1px solid #808080;
    background: #c0c0c0;
    line-height: 14px;
  }
  .dm-input {
    padding: 0 4px;
  }
  .dm-input label {
    font-size: 11px;
    display: block;
    margin-bottom: 2px;
  }
  .dm-input textarea {
    width: 100%;
    resize: none;
    box-sizing: border-box;
  }
  .dm-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    padding: 4px;
  }
  .loading-text, .empty-text {
    text-align: center;
    color: #888;
    padding: 20px;
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
    min-width: 80px;
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
    padding: 1px 0;
    flex-wrap: wrap;
  }
  .reaction-badge {
    font-size: 10px;
    padding: 0 4px;
    border: 1px solid #c0c0c0;
    background: #e8e8e8;
    border-radius: 8px;
    cursor: pointer;
    line-height: 16px;
  }
  .reaction-badge:hover {
    background: #d0d0ff;
  }
</style>
