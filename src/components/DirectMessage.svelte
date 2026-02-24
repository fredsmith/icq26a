<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomMessages, getRoomMembers, sendMessage, fetchMedia } from '../lib/matrix'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import type { Message } from '../lib/types'
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
  let unlistenNewMsg: (() => void) | null = null
  let dmUserId = $state<string | null>(null)
  let showNewMsgHint = $state(false)

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
            <div class="message">
              <span class="message-sender">{msg.sender_name}:</span>
              {#if msg.msg_type === 'image' && msg.media_url}
                <span class="message-body"><img class="message-image" use:loadMedia={msg.media_url} alt={msg.filename || msg.body} /></span>
              {:else if (msg.msg_type === 'file' || msg.msg_type === 'audio' || msg.msg_type === 'video') && msg.media_url}
                <span class="message-body"><a class="message-file" use:downloadFile={{ mxcUrl: msg.media_url, filename: msg.filename || msg.body }}>{msg.filename || msg.body}</a></span>
              {:else}
                <span class="message-body">{@html linkify(msg.body)}</span>
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
