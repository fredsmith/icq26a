<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { activeRoomId, rooms } from '../lib/stores'
  import { getRoomMessages, sendMessage } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import type { Message } from '../lib/types'

  let messages = $state<Message[]>([])
  let newMessage = $state('')
  let loading = $state(true)
  let messagesDiv = $state<HTMLDivElement | undefined>(undefined)
  let unlistenNewMsg: (() => void) | null = null

  const roomName = $derived($rooms.find(r => r.room_id === $activeRoomId)?.name ?? 'Unknown')

  onMount(async () => {
    if ($activeRoomId) {
      await loadMessages()
    }
    unlistenNewMsg = await listen<Message>('new_message', (event) => {
      if (event.payload.sender !== '') {
        messages = [...messages, event.payload]
        scrollToBottom()
      }
    })
  })

  onDestroy(() => {
    if (unlistenNewMsg) unlistenNewMsg()
  })

  async function loadMessages() {
    if (!$activeRoomId) return
    loading = true
    try {
      messages = await getRoomMessages($activeRoomId, 50)
    } catch (e) {
      console.error('Failed to load messages:', e)
    } finally {
      loading = false
      scrollToBottom()
    }
  }

  function scrollToBottom() {
    setTimeout(() => {
      if (messagesDiv) messagesDiv.scrollTop = messagesDiv.scrollHeight
    }, 0)
  }

  async function handleSend() {
    if (!newMessage.trim() || !$activeRoomId) return
    const body = newMessage
    newMessage = ''
    try {
      await sendMessage($activeRoomId, body)
    } catch (e) {
      console.error('Failed to send:', e)
      newMessage = body
    }
  }

  async function handleAttach() {
    if (!$activeRoomId) return
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const file = await open({ multiple: false })
      if (file) {
        await invoke('upload_file', { roomId: $activeRoomId, filePath: file })
      }
    } catch (e) {
      console.error('Failed to attach file:', e)
    }
  }

  function closeChat() {
    activeRoomId.set(null)
  }
</script>

<div class="window dm-window">
  <div class="title-bar">
    <div class="title-bar-text">{roomName} - Message Session</div>
    <div class="title-bar-controls">
      <button aria-label="Close" onclick={closeChat}></button>
    </div>
  </div>
  <div class="window-body">
    <!-- Header fields -->
    <div class="dm-header">
      <div class="field-row">
        <label>To:</label>
        <span>{roomName}</span>
      </div>
    </div>

    <!-- Messages area -->
    <div class="messages-area" bind:this={messagesDiv}>
      {#if loading}
        <p class="loading-text">Loading messages...</p>
      {:else if messages.length === 0}
        <p class="empty-text">No messages yet</p>
      {:else}
        {#each messages as msg}
          <div class="message">
            <span class="message-sender">{msg.sender_name}:</span>
            <span class="message-body">{msg.body}</span>
          </div>
        {/each}
      {/if}
    </div>

    <!-- Input area -->
    <div class="dm-input">
      <label for="msg-input">Enter Message:</label>
      <textarea
        id="msg-input"
        bind:value={newMessage}
        onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend() }}}
        rows="3"
      ></textarea>
    </div>

    <!-- Buttons -->
    <div class="dm-buttons">
      <button onclick={closeChat}>Cancel</button>
      <button onclick={handleAttach}>Attach</button>
      <button onclick={handleSend}>Send</button>
    </div>
  </div>
</div>

<style>
  .dm-window {
    width: 400px;
    height: 500px;
    display: flex;
    flex-direction: column;
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
  .dm-header .field-row label {
    font-weight: bold;
    min-width: 30px;
  }
  .messages-area {
    flex: 1;
    overflow-y: auto;
    background: white;
    border: 2px inset #c0c0c0;
    padding: 4px;
    font-size: 11px;
  }
  .message {
    margin-bottom: 2px;
  }
  .message-sender {
    font-weight: bold;
    color: #000080;
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
</style>
