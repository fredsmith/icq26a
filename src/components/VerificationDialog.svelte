<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { acceptVerification, confirmVerification, cancelVerification } from '../lib/matrix'
  import type { VerificationRequestEvent, VerificationEmojisEvent, VerificationEmoji } from '../lib/types'
  import TitleBar from './TitleBar.svelte'

  let visible = $state(false)
  let phase = $state<'request' | 'waiting' | 'emojis' | 'done' | 'cancelled'>('request')
  let flowId = $state('')
  let userId = $state('')
  let emojis = $state<VerificationEmoji[]>([])
  let isSelfVerification = $state(false)
  let unlisteners: (() => void)[] = []

  onMount(async () => {
    unlisteners.push(await listen<VerificationRequestEvent>('verification_request', (event) => {
      flowId = event.payload.flow_id
      userId = event.payload.user_id
      isSelfVerification = event.payload.is_self_verification
      phase = 'request'
      visible = true
    }))

    unlisteners.push(await listen<VerificationEmojisEvent>('verification_emojis', (event) => {
      if (event.payload.flow_id === flowId) {
        emojis = event.payload.emojis
        phase = 'emojis'
      }
    }))

    unlisteners.push(await listen<{ flow_id: string; user_id?: string }>('verification_done', (event) => {
      if (!event.payload.flow_id || event.payload.flow_id === flowId) {
        phase = 'done'
        setTimeout(() => { visible = false }, 3000)
      }
    }))

    unlisteners.push(await listen<{ flow_id: string; reason?: string }>('verification_cancelled', (event) => {
      if (!event.payload.flow_id || event.payload.flow_id === flowId) {
        phase = 'cancelled'
        setTimeout(() => { visible = false }, 3000)
      }
    }))
  })

  onDestroy(() => {
    unlisteners.forEach(fn => fn())
  })

  async function handleAccept() {
    phase = 'waiting'
    try {
      await acceptVerification(userId, flowId)
    } catch (e) {
      console.error('Failed to accept verification:', e)
      phase = 'cancelled'
      setTimeout(() => { visible = false }, 3000)
    }
  }

  async function handleConfirm() {
    await confirmVerification(userId, flowId)
  }

  async function handleCancel() {
    await cancelVerification(userId, flowId)
    visible = false
  }

  function handleClose() {
    if (phase === 'request' || phase === 'emojis' || phase === 'waiting') {
      handleCancel()
    } else {
      visible = false
    }
  }
</script>

{#if visible}
<div class="verification-overlay">
  <div class="window verification-window">
    <TitleBar title="Session Verification" onclose={handleClose} />
    <div class="window-body verification-body">
      {#if phase === 'request'}
        <p class="verification-text">
          {#if isSelfVerification}
            Verification request from another session.
          {:else}
            Verification request from <strong>{userId}</strong>.
          {/if}
        </p>
        <div class="button-row">
          <button onclick={handleAccept}>Accept</button>
          <button onclick={handleCancel}>Reject</button>
        </div>
      {:else if phase === 'waiting'}
        <p class="verification-text">Starting verification...</p>
      {:else if phase === 'emojis'}
        <p class="verification-text">Confirm these emojis match on both devices:</p>
        <div class="emoji-grid">
          {#each emojis as emoji}
            <div class="emoji-item">
              <span class="emoji-symbol">{emoji.symbol}</span>
              <span class="emoji-desc">{emoji.description}</span>
            </div>
          {/each}
        </div>
        <div class="button-row">
          <button onclick={handleConfirm}>They Match</button>
          <button onclick={handleCancel}>They Don't Match</button>
        </div>
      {:else if phase === 'done'}
        <p class="verification-text">Verification successful!</p>
      {:else if phase === 'cancelled'}
        <p class="verification-text">Verification cancelled.</p>
      {/if}
    </div>
  </div>
</div>
{/if}

<style>
  .verification-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .verification-window {
    width: 340px;
  }
  .verification-body {
    padding: 12px;
  }
  .verification-text {
    font-size: 11px;
    margin: 0 0 12px 0;
  }
  .emoji-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 4px;
    margin-bottom: 12px;
    text-align: center;
  }
  .emoji-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .emoji-symbol {
    font-size: 24px;
    line-height: 1;
  }
  .emoji-desc {
    font-size: 9px;
    color: #444;
  }
  .button-row {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
