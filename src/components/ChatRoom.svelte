<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomMessages, getRoomMembers, sendMessage, getRooms, createDmRoom, fetchMedia } from '../lib/matrix'
  import { listen } from '@tauri-apps/api/event'
  import { ask } from '@tauri-apps/plugin-dialog'
  import type { Message, Buddy } from '../lib/types'
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
  let unlistenNewMsg: (() => void) | null = null
  let showNewMsgHint = $state(false)

  // Sort members by most recent message, then filter by search term
  const sortedFilteredMembers = $derived.by(() => {
    // Build a map of sender → most recent timestamp
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
    unlistenNewMsg = await listen<Message>('new_message', (event) => {
      if (event.payload.room_id === roomId && event.payload.sender !== '') {
        messages = [...messages, event.payload]
        if (isNearBottom()) {
          scrollToBottom()
        } else {
          showNewMsgHint = true
        }
      }
    })
  })

  onDestroy(() => {
    if (unlistenNewMsg) unlistenNewMsg()
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

  async function handleSend() {
    if (!newMessage.trim() || !roomId) return
    const body = newMessage
    newMessage = ''
    try {
      await sendMessage(roomId, body)
    } catch (e) {
      console.error('Failed to send:', e)
      newMessage = body
    }
  }

  let contextMenu = $state<{ x: number; y: number; member: Buddy } | null>(null)

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
              <div class="chat-message">
                <div class="chat-message-header">
                  <span class="chat-sender">{msg.sender_name}</span>
                  <span class="chat-time">{formatTime(msg.timestamp)}</span>
                </div>
                {#if msg.msg_type === 'image' && msg.media_url}
                  <div class="chat-message-body"><img class="message-image" use:loadMedia={msg.media_url} alt={msg.filename || msg.body} /></div>
                {:else if (msg.msg_type === 'file' || msg.msg_type === 'audio' || msg.msg_type === 'video') && msg.media_url}
                  <div class="chat-message-body"><a class="message-file" use:downloadFile={{ mxcUrl: msg.media_url, filename: msg.filename || msg.body }}>{msg.filename || msg.body}</a></div>
                {:else}
                  <div class="chat-message-body">{@html linkify(msg.body)}</div>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
        {#if showNewMsgHint}
          <button class="new-msg-hint" onclick={jumpToBottom}>New messages below</button>
        {/if}
      </div>

      <!-- Input area -->
      <div class="chat-input">
        <textarea
          bind:value={newMessage}
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend() }}}
          rows="3"
          placeholder="Type a message..."
        ></textarea>
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

  <!-- Context menu -->
  {#if contextMenu}
    <div class="context-overlay" onclick={closeContextMenu} onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') closeContextMenu() }} role="presentation">
    </div>
    <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
      <button class="context-item" onclick={handleContextMessage}>Message</button>
      <button class="context-item" onclick={handleContextUserInfo}>User Info</button>
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
    background: transparent;
    text-align: left;
    font-size: 11px;
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
