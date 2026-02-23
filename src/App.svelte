<script lang="ts">
  import { isLoggedIn } from './lib/stores'
  import Login from './components/Login.svelte'
</script>

<main>
  {#if $isLoggedIn}
    <p>Logged in! (Buddy list coming next)</p>
  {:else}
    <Login />
  {/if}
</main>
