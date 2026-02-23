<script lang="ts">
  import { isLoggedIn, activeRoomId } from './lib/stores'
  import Login from './components/Login.svelte'
  import BuddyList from './components/BuddyList.svelte'
  import DirectMessage from './components/DirectMessage.svelte'
</script>

<main>
  {#if $isLoggedIn}
    <div class="app-layout">
      <BuddyList />
      {#if $activeRoomId}
        <DirectMessage />
      {/if}
    </div>
  {:else}
    <Login />
  {/if}
</main>

<style>
  .app-layout {
    display: flex;
    padding: 8px;
    gap: 8px;
    height: 100vh;
    box-sizing: border-box;
  }
</style>
