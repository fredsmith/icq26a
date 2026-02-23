<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { isLoggedIn, currentUserId } from './lib/stores'
  import { tryRestoreSession } from './lib/matrix'
  import { initNotifications, playMessageSound } from './lib/notifications'
  import type { Message } from './lib/types'
  import Login from './components/Login.svelte'
  import BuddyList from './components/BuddyList.svelte'
  import VerificationDialog from './components/VerificationDialog.svelte'

  let restoring = $state(true)

  onMount(async () => {
    initNotifications()
    await listen<Message>('new_message', (event) => {
      playMessageSound()
    })

    try {
      const userId = await tryRestoreSession()
      currentUserId.set(userId)
      isLoggedIn.set(true)
      await invoke('start_sync')
    } catch {
      // No saved session or restore failed — show login
    } finally {
      restoring = false
    }
  })
</script>

<main>
  {#if restoring}
    <div class="window" style="width: 200px; margin: 100px auto; text-align: center;">
      <div class="title-bar"><div class="title-bar-text">ICQ26a</div></div>
      <div class="window-body"><p>Connecting...</p></div>
    </div>
  {:else if $isLoggedIn}
    <BuddyList />
  {:else}
    <Login />
  {/if}
  <VerificationDialog />
</main>
