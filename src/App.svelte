<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
  import { isLoggedIn, currentUserId, syncing } from './lib/stores'
  import { tryRestoreSession } from './lib/matrix'
  import { initNotifications, playMessageSound } from './lib/notifications'
  import { openServerLogWindow } from './lib/windows'
  import type { Message } from './lib/types'
  import Login from './components/Login.svelte'
  import BuddyList from './components/BuddyList.svelte'
  import VerificationDialog from './components/VerificationDialog.svelte'

  const WINDOW_SIZE = new LogicalSize(300, 480)

  let restoring = $state(true)

  async function resizeWindow(size: LogicalSize) {
    try { await getCurrentWindow().setSize(size) } catch {}
  }

  onMount(async () => {
    initNotifications()
    await listen<Message>('new_message', (event) => {
      playMessageSound()
    })

    await listen<string>('sync_status', (event) => {
      syncing.set(event.payload !== 'synced')
    })

    try {
      const userId = await tryRestoreSession()
      currentUserId.set(userId)
      isLoggedIn.set(true)
      syncing.set(true)
      await resizeWindow(WINDOW_SIZE)
      await invoke('start_sync')
    } catch {
      // No saved session or restore failed — show login
      await resizeWindow(WINDOW_SIZE)
    } finally {
      restoring = false
    }
  })
</script>

<main>
  {#if restoring}
    <div class="window" style="width: 200px; margin: 100px auto; text-align: center;">
      <div class="title-bar"><div class="title-bar-text">ICQ26a</div></div>
      <div class="window-body">
        <img src="/loading-flower.gif" alt="Connecting" class="connecting-flower" />
        <p>Connecting...</p>
        <button onclick={openServerLogWindow}>Log</button>
      </div>
    </div>
  {:else if $isLoggedIn}
    <BuddyList />
  {:else}
    <Login />
  {/if}
  <VerificationDialog />
</main>

<style>
  .connecting-flower {
    width: 64px;
    height: 64px;
    margin: 8px auto 4px;
    display: block;
  }
</style>
