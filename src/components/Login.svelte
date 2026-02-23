<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { matrixLogin } from '../lib/matrix'
  import { isLoggedIn, currentUserId, preferences } from '../lib/stores'
  import type { LoginCredentials } from '../lib/types'
  import TitleBar from './TitleBar.svelte'
  import { openServerLogWindow, openPreferencesWindow } from '../lib/windows'

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
  <TitleBar title="ICQ26a Login" showMinimize />
  <div class="window-body">
    <div class="login-content">
      <div class="login-logo">
        <img src="/logo.png" alt="ICQ26a" class="logo-img" />
        <p class="logo-text">ICQ26a</p>
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
  </div>

  <div class="login-toolbar">
    <button onclick={openPreferencesWindow}>Settings</button>
    <button onclick={openServerLogWindow}>Log</button>
  </div>

  <div class="status-bar">
    <p class="status-bar-field">{loading ? 'Connecting...' : 'Ready'}</p>
  </div>
</div>

<style>
  .login-window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
  }
  .login-window .window-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 0 16px;
  }
  .login-content {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .login-logo {
    text-align: center;
    margin-bottom: 12px;
  }
  .logo-img {
    width: 96px;
    height: 96px;
  }
  .logo-text {
    font-size: 18px;
    font-weight: bold;
    margin: 4px 0 0;
  }
  .error-text {
    color: red;
    font-size: 11px;
    margin: 4px 0;
  }
  .login-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
    border-top: 1px solid #808080;
  }
</style>
