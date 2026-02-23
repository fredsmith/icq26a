<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'

  interface Props {
    title: string
    showMinimize?: boolean
    onclose?: () => void
  }
  let { title, showMinimize = false, onclose }: Props = $props()

  function handleClose() {
    if (onclose) {
      onclose()
    } else {
      getCurrentWindow().close()
    }
  }

  function handleMinimize() {
    getCurrentWindow().minimize()
  }
</script>

<div class="title-bar" data-tauri-drag-region>
  <div class="title-bar-text" data-tauri-drag-region>{title}</div>
  <div class="title-bar-controls">
    {#if showMinimize}
      <button aria-label="Minimize" onclick={handleMinimize}></button>
    {/if}
    <button aria-label="Close" onclick={handleClose}></button>
  </div>
</div>
