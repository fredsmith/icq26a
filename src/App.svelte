<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { isLoggedIn } from './lib/stores'
  import { initNotifications, playMessageSound } from './lib/notifications'
  import type { Message } from './lib/types'
  import Login from './components/Login.svelte'
  import BuddyList from './components/BuddyList.svelte'

  onMount(async () => {
    initNotifications()
    await listen<Message>('new_message', (event) => {
      playMessageSound()
    })
  })
</script>

<main>
  {#if $isLoggedIn}
    <BuddyList />
  {:else}
    <Login />
  {/if}
</main>
