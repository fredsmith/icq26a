<script lang="ts">
  import { preferences } from '../lib/stores'
  import type { AppPreferences } from '../lib/types'

  interface Props {
    onclose: () => void
  }
  let { onclose }: Props = $props()

  let localPrefs = $state<AppPreferences>({ ...$preferences })

  function handleSave() {
    preferences.set({ ...localPrefs })
    onclose()
  }

  function handleCancel() {
    onclose()
  }
</script>

<div class="window prefs-window">
  <div class="title-bar">
    <div class="title-bar-text">Preferences</div>
    <div class="title-bar-controls">
      <button aria-label="Close" onclick={handleCancel}></button>
    </div>
  </div>
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
    margin: 40px auto;
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
