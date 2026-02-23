<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getServerLog } from '../lib/matrix'
  import type { LogEntry } from '../lib/types'
  import TitleBar from './TitleBar.svelte'

  let entries = $state<LogEntry[]>([])
  let logDiv = $state<HTMLDivElement | undefined>(undefined)
  let autoScroll = $state(true)
  let unlistenLog: (() => void) | null = null

  onMount(async () => {
    try {
      entries = await getServerLog()
    } catch (e) {
      entries = [{ timestamp: Date.now() / 1000, level: 'error', message: `Failed to load log: ${e}` }]
    }
    scrollToBottom()

    unlistenLog = await listen<LogEntry>('server_log', (event) => {
      entries = [...entries, event.payload]
      if (autoScroll) scrollToBottom()
    })
  })

  onDestroy(() => {
    if (unlistenLog) unlistenLog()
  })

  function scrollToBottom() {
    setTimeout(() => {
      if (logDiv) logDiv.scrollTop = logDiv.scrollHeight
    }, 0)
  }

  function formatTime(ts: number): string {
    const d = new Date(ts * 1000)
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  }

  function levelClass(level: string): string {
    if (level === 'error') return 'log-error'
    if (level === 'warn') return 'log-warn'
    return 'log-info'
  }

  function handleScroll() {
    if (!logDiv) return
    const atBottom = logDiv.scrollHeight - logDiv.scrollTop - logDiv.clientHeight < 30
    autoScroll = atBottom
  }

  function closeWindow() {
    getCurrentWindow().close()
  }
</script>

<div class="window log-window">
  <TitleBar title="Server Log" onclose={closeWindow} />
  <div class="log-body" bind:this={logDiv} onscroll={handleScroll}>
    {#each entries as entry}
      <div class="log-line {levelClass(entry.level)}">
        <span class="log-time">{formatTime(entry.timestamp)}</span>
        <span class="log-level">[{entry.level.toUpperCase()}]</span>
        <span class="log-msg">{entry.message}</span>
      </div>
    {/each}
    {#if entries.length === 0}
      <div class="log-line log-info">
        <span class="log-msg">No log entries yet.</span>
      </div>
    {/if}
  </div>
  <div class="log-status">
    <span>{entries.length} entries</span>
    <label>
      <input type="checkbox" bind:checked={autoScroll} />
      Auto-scroll
    </label>
  </div>
</div>

<style>
  .log-window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }
  .log-body {
    flex: 1;
    overflow-y: auto;
    background: #000;
    color: #c0c0c0;
    font-family: 'Courier New', monospace;
    font-size: 11px;
    padding: 4px;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .log-line {
    padding: 1px 0;
  }
  .log-time {
    color: #666;
    margin-right: 6px;
  }
  .log-level {
    margin-right: 6px;
    font-weight: bold;
  }
  .log-info .log-level { color: #00cc00; }
  .log-warn .log-level { color: #cccc00; }
  .log-warn .log-msg { color: #cccc00; }
  .log-error .log-level { color: #ff4444; }
  .log-error .log-msg { color: #ff4444; }
  .log-status {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 2px 8px;
    background: #c0c0c0;
    border-top: 1px solid #808080;
    font-size: 11px;
  }
  .log-status label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
  }
</style>
