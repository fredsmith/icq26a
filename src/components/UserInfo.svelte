<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getUserProfile, getRooms, createDmRoom } from '../lib/matrix'
  import { ask } from '@tauri-apps/plugin-dialog'
  import type { UserProfile } from '../lib/types'
  import { openDirectMessageWindow } from '../lib/windows'
  import TitleBar from './TitleBar.svelte'

  interface Props {
    userId: string
    displayName: string
  }
  let { userId, displayName }: Props = $props()

  let profile = $state<UserProfile | null>(null)
  let loading = $state(true)
  let error = $state('')

  onMount(async () => {
    try {
      profile = await getUserProfile(userId)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  })

  function closeWindow() {
    getCurrentWindow().close()
  }

  async function handleMessage() {
    try {
      const allRooms = await getRooms()
      const name = profile?.display_name ?? displayName
      const dmRoom = allRooms.find(r => r.is_direct && r.name === name)
      if (dmRoom) {
        openDirectMessageWindow(dmRoom.room_id, dmRoom.name)
      } else {
        const confirmed = await ask(
          `Start a new conversation with ${name}? They will be notified.`,
          { title: 'New Message', kind: 'info' },
        )
        if (confirmed) {
          const newRoom = await createDmRoom(userId)
          openDirectMessageWindow(newRoom.room_id, newRoom.name)
        }
      }
    } catch (e) {
      console.error('Failed to open DM:', e)
    }
  }

  function presenceLabel(p: string): string {
    switch (p) {
      case 'online': return 'Online'
      case 'away': return 'Away'
      case 'offline': return 'Offline'
      default: return 'Unknown'
    }
  }

  function presenceColor(p: string): string {
    switch (p) {
      case 'online': return '#00cc00'
      case 'away': return '#cccc00'
      case 'offline': return '#999'
      default: return '#999'
    }
  }

  function formatLastSeen(seconds: number): string {
    if (seconds < 60) return 'just now'
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
    return `${Math.floor(seconds / 86400)}d ago`
  }
</script>

<div class="window userinfo-window">
  <TitleBar title="User Info - {displayName}" onclose={closeWindow} />
  <div class="window-body">
    {#if loading}
      <p class="loading-text">Loading...</p>
    {:else if error}
      <p class="error-text">Failed to load profile</p>
    {:else if profile}
      <!-- Avatar + name + status -->
      <div class="profile-header">
        <div class="avatar-area">
          {#if profile.avatar_url}
            <img class="avatar" src={profile.avatar_url} alt="avatar" />
          {:else}
            <div class="avatar-placeholder">👤</div>
          {/if}
        </div>
        <div class="name-area">
          <div class="display-name">{profile.display_name}</div>
          <div class="presence-row">
            <span class="presence-dot" style="background: {presenceColor(profile.presence)}"></span>
            {presenceLabel(profile.presence)}
          </div>
        </div>
      </div>

      <!-- Info fields -->
      <fieldset>
        <legend>Details</legend>
        <div class="info-row">
          <span class="info-label">User ID:</span>
          <span class="info-value">{profile.user_id}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Presence:</span>
          <span class="info-value">{presenceLabel(profile.presence)}</span>
        </div>
        {#if profile.last_seen_ago != null}
          <div class="info-row">
            <span class="info-label">Last Seen:</span>
            <span class="info-value">{formatLastSeen(profile.last_seen_ago)}</span>
          </div>
        {/if}
      </fieldset>

      <!-- Shared rooms -->
      {#if profile.shared_rooms.length > 0}
        <fieldset>
          <legend>Shared Rooms ({profile.shared_rooms.length})</legend>
          <div class="shared-rooms-list">
            {#each profile.shared_rooms as room}
              <div class="shared-room">{room.name}</div>
            {/each}
          </div>
        </fieldset>
      {/if}
    {/if}

    <div class="button-row">
      <button onclick={handleMessage}>Message</button>
      <button onclick={closeWindow}>Close</button>
    </div>
  </div>
</div>

<style>
  .userinfo-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }
  .userinfo-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow-y: auto;
  }
  .profile-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px;
  }
  .avatar-area {
    flex-shrink: 0;
  }
  .avatar, .avatar-placeholder {
    width: 48px;
    height: 48px;
    border: 2px inset #c0c0c0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: white;
    font-size: 24px;
  }
  .avatar {
    object-fit: cover;
  }
  .name-area {
    min-width: 0;
  }
  .display-name {
    font-weight: bold;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .presence-row {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    margin-top: 2px;
  }
  .presence-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  fieldset {
    margin: 0 4px;
    padding: 4px 8px 6px;
    font-size: 11px;
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
  .shared-rooms-list {
    max-height: 80px;
    overflow-y: auto;
  }
  .shared-room {
    padding: 1px 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
