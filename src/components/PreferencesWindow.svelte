<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { preferences } from '../lib/stores'
  import type { AppPreferences } from '../lib/types'
  import TitleBar from './TitleBar.svelte'

  let localPrefs = $state<AppPreferences>({ ...$preferences })

  function handleSave() {
    preferences.set({ ...localPrefs })
    getCurrentWindow().close()
  }

  function handleCancel() {
    getCurrentWindow().close()
  }
</script>

<div class="window prefs-window">
  <TitleBar title="Preferences" />
  <div class="window-body">
    <fieldset>
      <legend>Connection</legend>
      <div class="field-row-stacked" style="width: 280px;">
        <label for="pref-homeserver">Homeserver URL:</label>
        <input id="pref-homeserver" type="text" bind:value={localPrefs.homeserver} />
      </div>
    </fieldset>

    <fieldset>
      <legend>Notifications</legend>
      <div class="field-row">
        <input id="pref-sounds" type="checkbox" bind:checked={localPrefs.notification_sounds} />
        <label for="pref-sounds">Enable notification sounds</label>
      </div>
    </fieldset>

    <div class="prefs-buttons">
      <button onclick={handleSave}>OK</button>
      <button onclick={handleCancel}>Cancel</button>
    </div>
  </div>
</div>

<style>
  .prefs-window {
    width: 350px;
    margin: 0 auto;
  }
  fieldset {
    margin-bottom: 8px;
  }
  .prefs-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    margin-top: 8px;
  }
</style>
