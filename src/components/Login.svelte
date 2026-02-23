<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { matrixLogin } from '../lib/matrix'
  import { isLoggedIn, currentUserId, preferences } from '../lib/stores'
  import type { LoginCredentials } from '../lib/types'

  let username = $state('')
  let password = $state('')
  let homeserver = $state('')
  let error = $state('')
  let loading = $state(false)

  $effect(() => {
    const p = $preferences
    if (!homeserver) homeserver = p.homeserver
  })

  async function handleLogin() {
    error = ''
    loading = true
    try {
      const credentials: LoginCredentials = {
        homeserver,
        username,
        password,
      }
      const userId = await matrixLogin(credentials)
      currentUserId.set(userId)
      isLoggedIn.set(true)
      await invoke('start_sync')
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="window login-window">
  <div class="title-bar">
    <div class="title-bar-text">ICQ26a Login</div>
    <div class="title-bar-controls">
      <button aria-label="Close"></button>
    </div>
  </div>
  <div class="window-body">
    <div class="login-logo">
      <p style="font-size: 24px; font-weight: bold; text-align: center; margin: 8px 0;">ICQ26a</p>
    </div>

    <form onsubmit={(e) => { e.preventDefault(); handleLogin() }}>
      <div class="field-row-stacked" style="width: 200px;">
        <label for="homeserver">Homeserver:</label>
        <input id="homeserver" type="text" bind:value={homeserver} placeholder="https://matrix.org" />
      </div>
      <div class="field-row-stacked" style="width: 200px;">
        <label for="username">Username:</label>
        <input id="username" type="text" bind:value={username} placeholder="your_username" />
      </div>
      <div class="field-row-stacked" style="width: 200px;">
        <label for="password">Password:</label>
        <input id="password" type="password" bind:value={password} />
      </div>

      {#if error}
        <p class="error-text">{error}</p>
      {/if}

      <div class="field-row" style="justify-content: flex-end; margin-top: 8px;">
        <button type="submit" disabled={loading}>
          {loading ? 'Connecting...' : 'Login'}
        </button>
      </div>
    </form>
  </div>
  <div class="status-bar">
    <p class="status-bar-field">{loading ? 'Connecting...' : 'Ready'}</p>
  </div>
</div>

<style>
  .login-window {
    width: 280px;
    margin: 80px auto;
  }
  .error-text {
    color: red;
    font-size: 11px;
    margin: 4px 0;
  }
</style>
