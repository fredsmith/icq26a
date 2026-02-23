<script lang="ts">
  import { searchUsers, createDmRoom } from '../lib/matrix'
  import { openDirectMessageWindow } from '../lib/windows'
  import type { Buddy } from '../lib/types'
  import TitleBar from './TitleBar.svelte'

  let query = $state('')
  let results: Buddy[] = $state([])
  let error = $state('')
  let searching = $state(false)

  // Detect @user:server pattern
  const isUserId = $derived(/^@[^:]+:.+$/.test(query.trim()))

  async function handleSearch() {
    const input = query.trim()
    if (!input) return
    error = ''
    searching = true
    results = []
    try {
      const found = await searchUsers(input)
      // If input is a user ID, prepend it as a direct entry (deduped against search results)
      if (isUserId && !found.some(b => b.user_id === input)) {
        results = [{ user_id: input, display_name: input.split(':')[0].slice(1), avatar_url: null, presence: 'unknown' as const }, ...found]
      } else if (found.length === 0 && !isUserId) {
        error = 'No users found. Try a full user ID like @user:server'
      } else {
        results = found
      }
    } catch (e) {
      // If search fails but we have a user ID, still show the direct option
      if (isUserId) {
        results = [{ user_id: input, display_name: input.split(':')[0].slice(1), avatar_url: null, presence: 'unknown' as const }]
      } else {
        error = String(e)
      }
    } finally {
      searching = false
    }
  }

  async function handleMessage(buddy: Buddy) {
    error = ''
    try {
      const room = await createDmRoom(buddy.user_id)
      openDirectMessageWindow(room.room_id, buddy.display_name)
    } catch (e) {
      error = String(e)
    }
  }

  async function handleDirectMessage() {
    const input = query.trim()
    error = ''
    try {
      const room = await createDmRoom(input)
      const name = input.split(':')[0].slice(1)
      openDirectMessageWindow(room.room_id, name)
    } catch (e) {
      error = String(e)
    }
  }
</script>

<div class="window finduser-window">
  <TitleBar title="Find/Add Users" />
  <div class="window-body">
    <form onsubmit={(e) => { e.preventDefault(); handleSearch() }}>
      <div class="field-row">
        <label for="search">Search:</label>
        <input id="search" type="text" bind:value={query} placeholder="@user:server or name" style="flex: 1;" />
        <button type="submit" disabled={searching}>
          {searching ? '...' : 'Find'}
        </button>
      </div>
    </form>

    {#if isUserId && results.length === 0 && !searching}
      <div class="direct-hint">
        <span class="hint-text">Message this user directly?</span>
        <button onclick={handleDirectMessage}>Message {query.trim().split(':')[0]}</button>
      </div>
    {/if}

    {#if error}
      <p class="error-text">{error}</p>
    {/if}

    <div class="results-list">
      {#each results as buddy}
        <div class="result-row">
          <div class="result-info">
            <span class="result-name">{buddy.display_name}</span>
            {#if buddy.display_name !== buddy.user_id}
              <span class="result-id">{buddy.user_id}</span>
            {/if}
          </div>
          <button onclick={() => handleMessage(buddy)}>Message</button>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .finduser-window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
  }
  .finduser-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 8px;
    gap: 6px;
    overflow: hidden;
  }
  .field-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .direct-hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px;
    background: #ffffcc;
    border: 1px solid #ccc;
    font-size: 11px;
  }
  .hint-text {
    font-size: 11px;
  }
  .error-text {
    color: red;
    font-size: 11px;
    margin: 0;
  }
  .results-list {
    flex: 1;
    overflow-y: auto;
    border: 2px inset #c0c0c0;
    background: white;
  }
  .result-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 6px;
    border-bottom: 1px solid #ddd;
    gap: 4px;
  }
  .result-row:last-child {
    border-bottom: none;
  }
  .result-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .result-name {
    font-size: 11px;
    font-weight: bold;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result-id {
    font-size: 10px;
    color: #666;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
