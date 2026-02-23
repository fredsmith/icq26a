<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getRoomInfo, getRoomMembers } from '../lib/matrix'
  import type { RoomProfile, Buddy } from '../lib/types'
  import { openUserInfoWindow } from '../lib/windows'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    roomId: string
    roomName: string
  }
  let { roomId, roomName }: Props = $props()

  let profile = $state<RoomProfile | null>(null)
  let members = $state<Buddy[]>([])
  let loading = $state(true)
  let error = $state('')

  onMount(async () => {
    try {
      const [info, mems] = await Promise.all([
        getRoomInfo(roomId),
        getRoomMembers(roomId),
      ])
      profile = info
      members = mems
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  })

  function closeWindow() {
    getCurrentWindow().close()
  }
</script>

<div class="window roominfo-window">
  <TitleBar title="Room Info - {roomName}" onclose={closeWindow} />
  <div class="window-body">
    {#if loading}
      <p class="loading-text">Loading...</p>
    {:else if error}
      <p class="error-text">Failed to load room info</p>
    {:else if profile}
      <!-- Room name -->
      <div class="room-header">
        <div class="room-name">{profile.name}</div>
        {#if profile.is_direct}
          <div class="room-type">Direct Message</div>
        {:else}
          <div class="room-type">Group Room</div>
        {/if}
      </div>

      <!-- Info fields -->
      <fieldset>
        <legend>Details</legend>
        <div class="info-row">
          <span class="info-label">Room ID:</span>
          <span class="info-value">{profile.room_id}</span>
        </div>
        {#if profile.topic}
          <div class="info-row">
            <span class="info-label">Topic:</span>
            <span class="info-value" title={profile.topic}>{profile.topic}</span>
          </div>
        {/if}
        <div class="info-row">
          <span class="info-label">Members:</span>
          <span class="info-value">{profile.member_count}</span>
        </div>
      </fieldset>

      <!-- Members list -->
      {#if members.length > 0}
        <fieldset class="members-fieldset">
          <legend>Members ({members.length})</legend>
          <div class="members-list">
            {#each members as member}
              <button class="member-row" onclick={() => openUserInfoWindow(member.user_id, member.display_name)}>
                {member.display_name}
              </button>
            {/each}
          </div>
        </fieldset>
      {/if}
    {/if}

    <div class="button-row">
      <button onclick={closeWindow}>Close</button>
    </div>
  </div>
</div>

<style>
  .roominfo-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }
  .roominfo-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow-y: auto;
  }
  .room-header {
    padding: 8px 8px 0;
  }
  .room-name {
    font-weight: bold;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .room-type {
    font-size: 11px;
    color: #666;
    margin-top: 2px;
  }
  fieldset {
    margin: 0 4px;
    padding: 4px 8px 6px;
    font-size: 11px;
  }
  .members-fieldset {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  legend {
    font-size: 11px;
    font-weight: bold;
  }
  .info-row {
    display: flex;
    gap: 4px;
    margin-bottom: 2px;
  }
  .info-label {
    font-weight: bold;
    flex-shrink: 0;
    min-width: 70px;
  }
  .info-value {
    word-break: break-all;
    min-width: 0;
  }
  .members-list {
    flex: 1;
    overflow-y: auto;
    max-height: 120px;
  }
  .member-row {
    display: block;
    width: 100%;
    border: none;
    background: transparent;
    padding: 1px 4px;
    text-align: left;
    font-size: 11px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .member-row:hover {
    background: #000080;
    color: white;
  }
  .button-row {
    display: flex;
    justify-content: center;
    padding: 4px;
    margin-top: auto;
  }
  .loading-text, .error-text {
    text-align: center;
    color: #888;
    padding: 20px;
    font-size: 11px;
  }
  .error-text {
    color: #cc0000;
  }
</style>
