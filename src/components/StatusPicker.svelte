<script lang="ts">
  import { currentStatus } from '../lib/stores'
  import { setPresence } from '../lib/matrix'
  import type { PresenceStatus } from '../lib/types'

  interface Props {
    presenceAvailable?: boolean
  }
  let { presenceAvailable = true }: Props = $props()

  let menuOpen = $state(false)

  const allStatuses: { value: PresenceStatus; label: string; color: string }[] = [
    { value: 'online', label: 'Online', color: '#00cc00' },
    { value: 'free_for_chat', label: 'Free For Chat', color: '#00cc00' },
    { value: 'away', label: 'Away', color: '#cccc00' },
    { value: 'na', label: 'N/A (Extended Away)', color: '#cccc00' },
    { value: 'occupied', label: 'Occupied (Urgent Msgs)', color: '#cc0000' },
    { value: 'dnd', label: 'DND (Do not Disturb)', color: '#cc0000' },
    { value: 'invisible', label: 'Privacy (Invisible)', color: '#999999' },
    { value: 'offline', label: 'Offline/Disconnect', color: '#999999' },
  ]

  const simpleStatuses: { value: PresenceStatus; label: string; color: string }[] = [
    { value: 'online', label: 'Online', color: '#00cc00' },
    { value: 'offline', label: 'Offline/Disconnect', color: '#999999' },
  ]

  const statuses = $derived(presenceAvailable ? allStatuses : simpleStatuses)

  async function selectStatus(status: PresenceStatus) {
    currentStatus.set(status)
    menuOpen = false
    await setPresence(status)
  }

  function currentStatusInfo() {
    return statuses.find(s => s.value === $currentStatus)
  }
</script>

<svelte:window onclick={() => { menuOpen = false }} />

<div class="status-picker">
  <button class="status-button" onclick={(e: MouseEvent) => { e.stopPropagation(); menuOpen = !menuOpen }}>
    <span class="status-dot" style="background: {currentStatusInfo()?.color}"></span>
    {currentStatusInfo()?.label}
  </button>
  {#if menuOpen}
    <div class="status-menu">
      {#each statuses as status}
        <button
          class="status-menu-item"
          class:active={$currentStatus === status.value}
          onclick={(e: MouseEvent) => { e.stopPropagation(); selectStatus(status.value) }}
        >
          <span class="status-dot" style="background: {status.color}"></span>
          {status.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .status-picker {
    position: relative;
  }
  .status-button {
    width: 100%;
    text-align: left;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .status-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .status-menu {
    position: absolute;
    bottom: 100%;
    left: 0;
    right: 0;
    background: #c0c0c0;
    border: 2px outset #c0c0c0;
    z-index: 100;
  }
  .status-menu-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    border: none;
    background: transparent;
    padding: 3px 8px;
    text-align: left;
    cursor: pointer;
    font-size: 11px;
  }
  .status-menu-item:hover {
    background: #000080;
    color: white;
  }
  .status-menu-item.active {
    font-weight: bold;
  }
</style>
