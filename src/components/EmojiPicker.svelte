<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  interface Props {
    x: number
    y: number
    onpick: (emoji: string) => void
    onclose: () => void
  }
  let { x, y, onpick, onclose }: Props = $props()

  const EMOJI_ROWS = [
    ['👍', '👎', '❤️', '😂', '😮', '😢', '😡', '🎉'],
    ['🔥', '👀', '🙏', '✅', '❌', '💯', '⭐', '💀'],
    ['😊', '😍', '🤔', '😎', '🥳', '😴', '🤣', '👋'],
    ['👏', '💪', '🤝', '🙌', '🫡', '🫠', '😬', '🤷'],
  ]

  let captureInput = $state<HTMLInputElement | undefined>(undefined)

  function handleOverlayClick(e: MouseEvent) {
    e.preventDefault()
    e.stopPropagation()
    onclose()
  }

  function openSystemPicker() {
    if (!captureInput) return
    captureInput.value = ''
    captureInput.focus()
    invoke('show_emoji_picker').catch(() => {})
  }

  function handleCaptureInput() {
    if (!captureInput) return
    const val = captureInput.value.trim()
    if (val) {
      // Take only the first emoji/grapheme cluster
      const segments = [...new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(val)]
      if (segments.length > 0) {
        onpick(segments[0].segment)
      }
    }
  }
</script>

<div class="emoji-overlay" onclick={handleOverlayClick} oncontextmenu={handleOverlayClick} role="presentation"></div>
<div class="emoji-picker window" style="left: {x}px; top: {y}px;">
  <div class="title-bar">
    <div class="title-bar-text">React</div>
    <div class="title-bar-controls">
      <button aria-label="Close" onclick={onclose}></button>
    </div>
  </div>
  <div class="window-body emoji-body">
    {#each EMOJI_ROWS as row}
      <div class="emoji-row">
        {#each row as emoji}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span class="emoji-btn" onclick={() => onpick(emoji)} role="button" tabindex="0" onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') onpick(emoji) }}>{emoji}</span>
        {/each}
      </div>
    {/each}
    <div class="emoji-more-row">
      <button class="emoji-more-btn" onclick={openSystemPicker}>More...</button>
    </div>
    <input
      class="emoji-capture"
      bind:this={captureInput}
      oninput={handleCaptureInput}
      aria-hidden="true"
      tabindex="-1"
    />
  </div>
</div>

<style>
  .emoji-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 199;
  }
  .emoji-picker {
    position: fixed;
    z-index: 200;
    width: auto;
  }
  .emoji-body {
    padding: 4px;
  }
  .emoji-row {
    display: flex;
    gap: 1px;
  }
  .emoji-btn {
    width: 28px;
    height: 28px;
    font-size: 16px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    user-select: none;
  }
  .emoji-btn:hover {
    background: #000080;
    outline: 1px solid #000080;
  }
  .emoji-more-row {
    display: flex;
    justify-content: center;
    margin-top: 4px;
  }
  .emoji-more-btn {
    font-size: 11px;
    padding: 2px 12px;
  }
  .emoji-capture {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
    overflow: hidden;
    pointer-events: none;
  }
</style>
