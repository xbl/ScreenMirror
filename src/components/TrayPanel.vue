<template>
  <main class="tray-panel">
    <header class="tray-head">
      <div class="tray-brand">
        <img class="tray-logo" :src="logoUrl" alt="Screenmirror" />
        <div>
          <p class="tray-kicker">Screenmirror</p>
          <h1>{{ t('tray.title') }}</h1>
        </div>
      </div>
      <div class="tray-head-actions">
        <button class="tray-devices" type="button" :aria-label="t('devices.title')" @click="showDevices = true">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 4h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Zm0 4h10M9 16h6" /></svg>
          <strong v-if="viewerCount > 0">{{ viewerCount }}</strong>
        </button>
        <button class="tray-settings" type="button" :aria-label="t('settings.title')" @click="showSettings = true">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" />
            <path d="m19.4 15 .1.1a1.8 1.8 0 0 1-2.5 2.5l-.1-.1a1.8 1.8 0 0 0-3 .9v.2a1.8 1.8 0 0 1-3.6 0v-.2a1.8 1.8 0 0 0-3-.9l-.1.1a1.8 1.8 0 1 1-2.5-2.5l.1-.1a1.8 1.8 0 0 0-.9-3H3.7a1.8 1.8 0 0 1 0-3h.2a1.8 1.8 0 0 0 .9-3l-.1-.1a1.8 1.8 0 1 1 2.5-2.5l.1.1a1.8 1.8 0 0 0 3-.9v-.2a1.8 1.8 0 0 1 3.6 0v.2a1.8 1.8 0 0 0 3 .9l.1-.1a1.8 1.8 0 1 1 2.5 2.5l-.1.1a1.8 1.8 0 0 0 .9 3h.2a1.8 1.8 0 0 1 0 3h-.2a1.8 1.8 0 0 0-.9 3Z" />
          </svg>
        </button>
      </div>
    </header>

    <QRCard class="tray-qr" />
    <SourcePicker class="tray-source" external-chooser />

    <section class="tray-controls" aria-label="Share controls">
      <div class="tray-actions">
        <button class="tray-action tray-exit" type="button" @click="exitApp">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M10 5H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h4M13 8l4 4-4 4M9 12h8" /></svg>
          <span>{{ t('app.exit') }}</span>
        </button>
      </div>
    </section>

    <ConnectedDevicesListDrawer
      :open="showDevices"
      @close="showDevices = false"
    />
    <SettingsOverlay
      :open="showSettings"
      @close="showSettings = false"
      @reset="onReset"
    />
  </main>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import QRCard from './QRCard.vue';
import SourcePicker from './SourcePicker.vue';
import ConnectedDevicesListDrawer from './ConnectedDevicesListDrawer.vue';
import SettingsOverlay from './SettingsOverlay.vue';
import { api } from '../utils/api';
import logoUrl from '../../src-tauri/icons/icon.png';

const { t } = useI18n();
const showDevices = ref(false);
const showSettings = ref(false);
const viewerCount = ref(0);
let poll: number | undefined;

async function refreshState() {
  try {
    viewerCount.value = (await api.getConnectedDevices()).length;
  } catch {
    /* tolerate startup races while the host server comes up */
  }
}

async function onReset() {
  viewerCount.value = 0;
}

async function exitApp() {
  await api.exitApp();
}

onMounted(() => {
  void refreshState();
  poll = window.setInterval(refreshState, 2000);
});

onBeforeUnmount(() => {
  if (poll) clearInterval(poll);
});
</script>

<style scoped>
.tray-panel {
  min-height: 100vh;
  padding: 16px;
  color: var(--text);
  background: var(--bg);
}

.tray-head,
.tray-brand {
  display: flex;
  align-items: center;
}

.tray-head {
  justify-content: space-between;
  margin-bottom: 14px;
}

.tray-brand {
  gap: 10px;
}

.tray-head-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.tray-logo {
  width: 28px;
  height: 28px;
  object-fit: contain;
}

.tray-kicker {
  margin: 0;
  color: var(--muted);
  font-size: 10px;
  letter-spacing: .14em;
  text-transform: uppercase;
}

.tray-head h1 {
  margin: 2px 0 0;
  color: var(--text-strong);
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 650;
}

.tray-settings,
.tray-devices {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  color: var(--muted);
  border: 0;
  border: var(--line);
  border-radius: var(--radius-md);
  background: var(--control);
  cursor: pointer;
}

.tray-settings svg,
.tray-devices svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
}

.tray-settings:hover,
.tray-devices:hover {
  color: var(--text-strong);
  border-color: var(--accent-line);
  background: var(--accent-dim);
}

.tray-devices {
  position: relative;
}

.tray-devices strong {
  position: absolute;
  top: -3px;
  right: -3px;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  color: #fff;
  font-size: 9px;
  line-height: 14px;
  text-align: center;
  border-radius: 8px;
  background: var(--accent);
}

:deep(.qr-card) {
  grid-template-columns: 168px minmax(0, 1fr);
  gap: 14px;
  padding: 12px;
}

:deep(.qr-frame) {
  width: 152px;
  height: 152px;
  padding: 8px;
}

:deep(.qr-meta) {
  gap: 8px;
}

:deep(.qr-url) {
  max-width: 180px;
  font-size: 11px;
}

:deep(.qr-actions .btn) {
  width: 100%;
}

.tray-source {
  margin-top: 12px;
}

.tray-controls,
.tray-permission {
  margin-top: 12px;
  padding-top: 12px;
  border-top: var(--line);
}

.tray-actions {
  display: grid;
  grid-template-columns: 1fr;
  gap: 8px;
  margin-top: 10px;
}

.tray-exit {
  justify-content: center;
  gap: 8px;
  color: var(--danger, #d86b6b);
  border: 1px solid color-mix(in srgb, var(--danger) 38%, transparent);
  background: color-mix(in srgb, var(--danger) 9%, var(--surface));
}

.tray-exit svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.8; }

.tray-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 36px;
  padding: 8px 10px;
  color: var(--text);
  font-size: 12px;
  text-align: left;
  border: var(--line);
  border-radius: var(--radius-md);
  background: var(--control);
}

.tray-action:hover,
.tray-text-action:hover {
  color: var(--text-strong);
  border-color: var(--accent);
}

.tray-exit:hover { color: #fff; border-color: var(--danger); background: var(--danger); }

.tray-permission {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.tray-section-label {
  display: block;
  color: var(--muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: .08em;
}

.tray-permission strong {
  display: block;
  margin-top: 3px;
  color: var(--warn);
  font-size: 12px;
}

.tray-permission[data-ok='true'] strong {
  color: var(--accent);
}

.tray-permission-actions {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
}

.tray-text-action {
  padding: 2px 0;
  color: var(--muted);
  font-size: 11px;
  border: 0;
  background: transparent;
}

:deep(.source-picker) {
  padding: 14px;
}

</style>
