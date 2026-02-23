<script lang="ts">
  import { joinRoom, createRoom } from '../lib/matrix'
  import { openChatRoomWindow } from '../lib/windows'
  import TitleBar from './TitleBar.svelte'

  let roomInput = $state('')
  let error = $state('')
  let success = $state('')
  let joining = $state(false)
  let showCreate = $state(false)

  async function handleJoin() {
    const input = roomInput.trim()
    if (!input) return
    error = ''
    success = ''
    showCreate = false
    joining = true
    try {
      const room = await joinRoom(input)
      success = `Joined ${room.name}`
      openChatRoomWindow(room.room_id, room.name)
    } catch (e) {
      const msg = String(e)
      if (msg.includes('M_NOT_FOUND')) {
        error = 'Room not found.'
        showCreate = true
      } else {
        error = msg
      }
    } finally {
      joining = false
    }
  }

  async function handleCreate() {
    const input = roomInput.trim()
    if (!input) return
    error = ''
    success = ''
    showCreate = false
    joining = true
    try {
      const room = await createRoom(input)
      success = `Created ${room.name}`
      openChatRoomWindow(room.room_id, room.name)
    } catch (e) {
      error = String(e)
    } finally {
      joining = false
    }
  }
</script>

<div class="window joinroom-window">
  <TitleBar title="Join Room" />
  <div class="window-body">
    <p class="hint">Enter a room alias or ID:</p>
    <form onsubmit={(e) => { e.preventDefault(); handleJoin() }}>
      <div class="field-row-stacked" style="width: 100%;">
        <label for="room-input">Room:</label>
        <input id="room-input" type="text" bind:value={roomInput} placeholder="#room:server.com" />
      </div>

      {#if error}
        <p class="error-text">{error}</p>
      {/if}
      {#if showCreate}
        <div class="create-hint">
          <span>Create this room instead?</span>
          <button onclick={handleCreate} disabled={joining}>Create</button>
        </div>
      {/if}
      {#if success}
        <p class="success-text">{success}</p>
      {/if}

      <div class="field-row" style="justify-content: flex-end; margin-top: 8px;">
        <button type="submit" disabled={joining}>
          {joining ? 'Joining...' : 'Join'}
        </button>
      </div>
    </form>
  </div>
</div>

<style>
  .joinroom-window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
  }
  .joinroom-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 8px;
  }
  .hint {
    font-size: 11px;
    margin: 0 0 6px;
  }
  .error-text {
    color: red;
    font-size: 11px;
    margin: 4px 0;
  }
  .success-text {
    color: green;
    font-size: 11px;
    margin: 4px 0;
  }
  .create-hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px;
    background: #ffffcc;
    border: 1px solid #ccc;
    font-size: 11px;
    margin: 2px 0;
  }
</style>
