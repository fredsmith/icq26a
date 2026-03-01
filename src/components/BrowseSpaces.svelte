<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { searchSpaces, getSpaceHierarchy, joinRoom } from '../lib/matrix'
  import { openChatRoomWindow } from '../lib/windows'
  import type { PublicSpace, SpaceChild } from '../lib/types'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    spaceId?: string
    spaceName?: string
  }
  let { spaceId, spaceName }: Props = $props()

  let query = $state('')
  let server = $state('')
  let results = $state<PublicSpace[]>([])
  let error = $state('')
  let searching = $state(false)

  let selectedSpace = $state<PublicSpace | null>(null)
  let children = $state<SpaceChild[]>([])
  let loadingChildren = $state(false)

  let joiningIds = $state<Set<string>>(new Set())

  onMount(() => {
    if (spaceId && spaceName) {
      handleViewSpace({
        room_id: spaceId,
        name: spaceName,
        topic: null,
        num_joined_members: 0,
        alias: null,
      })
    } else {
      loadSpaces('', 10)
    }
  })

  async function loadSpaces(searchQuery: string, limit?: number, targetServer?: string) {
    error = ''
    searching = true
    results = []
    selectedSpace = null
    children = []
    try {
      results = await searchSpaces(searchQuery, limit, targetServer)
      if (results.length === 0) {
        error = 'No spaces found.'
      }
    } catch (e) {
      error = String(e)
    } finally {
      searching = false
    }
  }

  async function handleSearch() {
    const input = query.trim()
    const srv = server.trim() || undefined
    loadSpaces(input, undefined, srv)
  }

  async function handleViewSpace(space: PublicSpace) {
    error = ''
    selectedSpace = space
    loadingChildren = true
    children = []
    try {
      children = await getSpaceHierarchy(space.room_id)
    } catch (e) {
      error = String(e)
    } finally {
      loadingChildren = false
    }
  }

  function handleBack() {
    if (spaceId) {
      getCurrentWindow().close()
    } else {
      selectedSpace = null
      children = []
      error = ''
    }
  }

  async function handleJoin(roomId: string, name: string) {
    error = ''
    joiningIds = new Set([...joiningIds, roomId])
    try {
      await joinRoom(roomId)
      children = children.map(c =>
        c.room_id === roomId ? { ...c, is_joined: true } : c
      )
      const child = children.find(c => c.room_id === roomId)
      if (!child || child.room_type !== 'm.space') {
        openChatRoomWindow(roomId, name)
      }
    } catch (e) {
      error = String(e)
    } finally {
      joiningIds = new Set([...joiningIds].filter(id => id !== roomId))
    }
  }
</script>

<div class="window browsespaces-window">
  <TitleBar title={spaceName ? `Browse: ${spaceName}` : 'Browse Spaces'} />
  <div class="window-body">
    {#if selectedSpace === null}
      <form onsubmit={(e) => { e.preventDefault(); handleSearch() }}>
        <div class="field-row">
          <label for="search">Search:</label>
          <input id="search" type="text" bind:value={query} placeholder="Space name or topic" style="flex: 1;" />
          <button type="submit" disabled={searching}>
            {searching ? '...' : 'Find'}
          </button>
        </div>
        <div class="field-row">
          <label for="server">Server:</label>
          <input id="server" type="text" bind:value={server} placeholder="e.g. matrix.org" style="flex: 1;" />
        </div>
      </form>

      {#if error}
        <p class="error-text">{error}</p>
      {/if}

      <div class="results-list">
        {#each results as space}
          <div class="result-row">
            <div class="result-info">
              <span class="result-name">{space.name}</span>
              {#if space.alias}
                <span class="result-id">{space.alias}</span>
              {/if}
              {#if space.topic}
                <span class="result-topic">{space.topic}</span>
              {/if}
              <span class="result-meta">{space.num_joined_members} members</span>
            </div>
            <div class="result-actions">
              <button onclick={() => handleViewSpace(space)}>Browse</button>
              <button onclick={() => handleJoin(space.room_id, space.name)} disabled={joiningIds.has(space.room_id)}>
                {joiningIds.has(space.room_id) ? '...' : 'Join'}
              </button>
            </div>
          </div>
        {/each}
      </div>

    {:else}
      <div class="hierarchy-header">
        <button onclick={handleBack}>&laquo; Back</button>
        <div class="space-title">
          <span class="result-name">{selectedSpace.name}</span>
          <span class="result-meta">{selectedSpace.num_joined_members} members</span>
        </div>
      </div>

      {#if selectedSpace.topic}
        <p class="space-topic">{selectedSpace.topic}</p>
      {/if}

      {#if error}
        <p class="error-text">{error}</p>
      {/if}

      {#if loadingChildren}
        <p class="loading-text">Loading rooms...</p>
      {:else}
        <div class="results-list">
          {#each children as child}
            <div class="result-row">
              <div class="result-info">
                <span class="result-name">
                  {#if child.room_type === 'm.space'}[Space] {/if}{child.name}
                </span>
                {#if child.topic}
                  <span class="result-topic">{child.topic}</span>
                {/if}
                <span class="result-meta">{child.num_joined_members} members</span>
              </div>
              <div class="result-actions">
                {#if child.is_joined}
                  <button disabled>Joined</button>
                {:else}
                  <button onclick={() => handleJoin(child.room_id, child.name)} disabled={joiningIds.has(child.room_id)}>
                    {joiningIds.has(child.room_id) ? '...' : 'Join'}
                  </button>
                {/if}
              </div>
            </div>
          {/each}
          {#if children.length === 0}
            <p class="empty-text">No rooms found in this space.</p>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .browsespaces-window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
  }
  .browsespaces-window .window-body {
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
  .error-text {
    color: red;
    font-size: 11px;
    margin: 0;
  }
  .loading-text, .empty-text {
    font-size: 11px;
    color: #666;
    text-align: center;
    padding: 12px;
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
    align-items: flex-start;
    justify-content: space-between;
    padding: 4px 6px;
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
    flex: 1;
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
  .result-topic {
    font-size: 10px;
    color: #444;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .result-meta {
    font-size: 10px;
    color: #888;
  }
  .result-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
    padding-top: 2px;
  }
  .hierarchy-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .space-title {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .space-topic {
    font-size: 10px;
    color: #444;
    margin: 0;
    max-height: 28px;
    overflow: hidden;
  }
</style>
