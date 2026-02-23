<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { isLoggedIn, activeRoomId, rooms } from './lib/stores'
  import { initNotifications, playMessageSound } from './lib/notifications'
  import type { Message } from './lib/types'
  import Login from './components/Login.svelte'
  import BuddyList from './components/BuddyList.svelte'
  import DirectMessage from './components/DirectMessage.svelte'
  import ChatRoom from './components/ChatRoom.svelte'
  import Preferences from './components/Preferences.svelte'

  let showPreferences = $state(false)

  onMount(async () => {
    initNotifications()
    await listen<Message>('new_message', (event) => {
      playMessageSound()
    })
  })
</script>

<main>
  {#if $isLoggedIn}
    <div class="app-layout">
      <BuddyList />
      {#if $activeRoomId}
        {#if $rooms.find(r => r.room_id === $activeRoomId)?.is_direct}
          <DirectMessage />
        {:else}
          <ChatRoom />
        {/if}
      {/if}
    </div>
    <div class="toolbar">
      <button onclick={() => showPreferences = true}>Preferences</button>
    </div>
    {#if showPreferences}
      <Preferences onclose={() => showPreferences = false} />
    {/if}
  {:else}
    <Login />
  {/if}
</main>

<style>
  .app-layout {
    display: flex;
    padding: 8px;
    gap: 8px;
    height: calc(100vh - 32px);
    box-sizing: border-box;
  }
  .toolbar {
    display: flex;
    justify-content: flex-end;
    padding: 4px 8px;
  }
</style>
