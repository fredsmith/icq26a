<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { matrixLogin, matrixRegister } from '../lib/matrix'
  import { isLoggedIn, currentUserId, preferences } from '../lib/stores'
  import type { LoginCredentials } from '../lib/types'
  import TitleBar from './TitleBar.svelte'
  import { openServerLogWindow, openPreferencesWindow } from '../lib/windows'

  let username = $state('')
  let password = $state('')
  let confirmPassword = $state('')
  let homeserver = $state('')
  let error = $state('')
  let loading = $state(false)
  let mode: 'login' | 'register' = $state('login')

  $effect(() => {
    const p = $preferences
    if (!homeserver) homeserver = p.homeserver
  })

  async function handleSubmit() {
    error = ''

    if (mode === 'register' && password !== confirmPassword) {
      error = 'Passwords do not match'
      return
    }

    loading = true
    try {
      const credentials: LoginCredentials = {
        homeserver,
        username,
        password,
      }
      const userId = mode === 'register'
        ? await matrixRegister(credentials)
        : await matrixLogin(credentials)
      currentUserId.set(userId)
      isLoggedIn.set(true)
      await invoke('start_sync')
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  function toggleMode() {
    mode = mode === 'login' ? 'register' : 'login'
    error = ''
    confirmPassword = ''
  }
</script>

<div class="window login-window">
  <TitleBar title={mode === 'register' ? 'ICQ26a Register' : 'ICQ26a Login'} showMinimize />
  <div class="window-body">
    <div class="login-content">
      <div class="login-logo">
        <img src="/logo.png" alt="ICQ26a" class="logo-img" />
        <p class="logo-text">ICQ26a</p>
      </div>

      <form onsubmit={(e) => { e.preventDefault(); handleSubmit() }}>
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

        {#if mode === 'register'}
          <div class="field-row-stacked" style="width: 200px;">
            <label for="confirm-password">Confirm Password:</label>
            <input id="confirm-password" type="password" bind:value={confirmPassword} />
          </div>
        {/if}

        {#if error}
          <p class="error-text">{error}</p>
        {/if}

        <div class="field-row" style="justify-content: flex-end; margin-top: 8px;">
          <button type="submit" disabled={loading}>
            {loading ? 'Connecting...' : mode === 'register' ? 'Register' : 'Login'}
          </button>
        </div>
      </form>

      <p class="toggle-text">
        {#if mode === 'login'}
          Don't have an account? <button class="link-btn" onclick={toggleMode}>Register</button>
        {:else}
          Already have an account? <button class="link-btn" onclick={toggleMode}>Log in</button>
        {/if}
      </p>
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
  .toggle-text {
    font-size: 11px;
    margin-top: 8px;
  }
  .link-btn {
    background: none;
    border: none;
    color: #0000ee;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font-size: 11px;
  }
  .login-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
    border-top: 1px solid #808080;
  }
</style>
