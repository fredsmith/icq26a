<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomMessages, getRoomMembers, sendMessage } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import type { Message } from '../lib/types'
  import { openUserInfoWindow } from '../lib/windows'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    roomId: string
    roomName: string
  }
  let { roomId, roomName }: Props = $props()

  let messages = $state<Message[]>([])
  let newMessage = $state('')
  let loading = $state(true)
  let messagesDiv = $state<HTMLDivElement | undefined>(undefined)
  let unlistenNewMsg: (() => void) | null = null
  let dmUserId = $state<string | null>(null)

  onMount(async () => {
    if (roomId) {
      await loadMessages()
      // Find the other user in this DM
      try {
        const members = await getRoomMembers(roomId)
        const myId = await invoke<string>('try_restore_session').catch(() => null)
        const other = members.find(m => m.user_id !== myId)
        if (other) dmUserId = other.user_id
      } catch { /* ignore */ }
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

  async function loadMessages() {
    if (!roomId) return
    loading = true
    try {
      messages = await getRoomMessages(roomId, 50)
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
      <button onclick={closeWindow}>Cancel</button>
      <button onclick={handleAttach}>Attach</button>
      <button onclick={handleSend}>Send</button>
    </div>
  </div>
</div>

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
