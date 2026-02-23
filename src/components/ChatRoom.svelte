<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomMessages, getRoomMembers, sendMessage } from '../lib/matrix'
  import { listen } from '@tauri-apps/api/event'
  import type { Message, Buddy } from '../lib/types'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    roomId: string
    roomName: string
  }
  let { roomId, roomName }: Props = $props()

  let messages = $state<Message[]>([])
  let members = $state<Buddy[]>([])
  let newMessage = $state('')
  let loading = $state(true)
  let messagesDiv = $state<HTMLDivElement | undefined>(undefined)
  let unlistenNewMsg: (() => void) | null = null

  onMount(async () => {
    if (roomId) {
      loading = true
      try {
        const [msgs, mems] = await Promise.all([
          getRoomMessages(roomId, 50),
          getRoomMembers(roomId),
        ])
        messages = msgs
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

    <!-- Members sidebar -->
    <div class="members-panel">
      <div class="members-header">Participants</div>
      <div class="members-list">
        {#each members as member}
          <div class="member-row">
            <span class="member-dot"></span>
            {member.display_name}
          </div>
        {/each}
      </div>
    </div>
  </div>
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
  .members-panel {
    width: 140px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid #808080;
  }
  .members-header {
    font-weight: bold;
    font-size: 11px;
    padding: 4px 8px;
    background: #c0c0c0;
    border-bottom: 1px solid #808080;
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
