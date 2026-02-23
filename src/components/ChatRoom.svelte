<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { activeRoomId, rooms } from '../lib/stores'
  import { getRoomMessages, sendMessage } from '../lib/matrix'
  import { listen } from '@tauri-apps/api/event'
  import type { Message } from '../lib/types'

  let messages = $state<Message[]>([])
  let newMessage = $state('')
  let loading = $state(true)
  let messagesDiv = $state<HTMLDivElement | undefined>(undefined)
  let unlistenNewMsg: (() => void) | null = null

  const room = $derived($rooms.find(r => r.room_id === $activeRoomId))
  const roomName = $derived(room?.name ?? 'Chat')

  onMount(async () => {
    if ($activeRoomId) {
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

  function closeChat() {
    activeRoomId.set(null)
  }

  function formatTime(ts: number): string {
    const d = new Date(ts * 1000)
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  }
</script>

<div class="window chat-window">
  <div class="title-bar">
    <div class="title-bar-text">ICQ Chat - {roomName}</div>
    <div class="title-bar-controls">
      <button aria-label="Close" onclick={closeChat}></button>
    </div>
  </div>

  <!-- Menu bar -->
  <div class="menu-bar">
    <button>File</button>
    <button>Edit</button>
    <button>Display</button>
  </div>

  <div class="window-body chat-body">
    <!-- Messages pane -->
    <div class="chat-messages" bind:this={messagesDiv}>
      {#if loading}
        <p class="loading-text">Loading...</p>
      {:else}
        {#each messages as msg}
          <div class="chat-message">
            <div class="chat-message-header">
              <span class="chat-sender">{msg.sender_name}</span>
              <span class="chat-time">{formatTime(msg.timestamp)}</span>
            </div>
            <div class="chat-message-body">{msg.body}</div>
          </div>
        {/each}
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
</div>

<style>
  .chat-window {
    width: 500px;
    height: 500px;
    display: flex;
    flex-direction: column;
  }
  .menu-bar {
    display: flex;
    gap: 0;
    padding: 2px 4px;
    background: #c0c0c0;
    border-bottom: 1px solid #808080;
  }
  .menu-bar button {
    border: none;
    background: transparent;
    padding: 2px 8px;
    font-size: 11px;
    cursor: pointer;
  }
  .menu-bar button:hover {
    background: #000080;
    color: white;
  }
  .chat-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
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
  .chat-input {
    display: flex;
    gap: 4px;
    padding: 4px;
  }
  .chat-input textarea {
    flex: 1;
    resize: none;
  }
  .loading-text {
    text-align: center;
    color: #888;
    padding: 20px;
  }
</style>
