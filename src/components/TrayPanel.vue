<template>
  <main class="tray-panel">
    <header class="tray-head">
      <div class="tray-brand">
        <span class="tray-mark" aria-hidden="true"></span>
        <div>
          <p class="tray-kicker">Screenmirror</p>
          <h1>{{ t('tray.title') }}</h1>
        </div>
      </div>
      <button class="tray-close" type="button" :aria-label="t('tray.close')" @click="close">
        ×
      </button>
    </header>

    <QRCard class="tray-qr" />
    <SourcePicker class="tray-source" external-chooser />

    <section class="tray-controls" aria-label="Share controls">
      <StartButton
        v-model:sharing="sharing"
        v-model:viewerCount="viewerCount"
      />
      <div class="tray-actions">
        <button class="tray-action" type="button" @click="showDevices = true">
          <span>{{ t('devices.title') }}</span>
          <strong>{{ viewerCount }}</strong>
        </button>
        <button class="tray-action" type="button" @click="showSettings = true">
          {{ t('settings.title') }}
        </button>
      </div>
    </section>

    <section class="tray-permission" :data-ok="permissionGranted">
      <div>
        <span class="tray-section-label">{{ t('permission.title') }}</span>
        <strong>{{ permissionGranted ? t('permission.granted') : t('permission.required') }}</strong>
      </div>
      <div class="tray-permission-actions">
        <button class="tray-text-action" type="button" @click="checkPermission">
          {{ t('permission.recheck') }}
        </button>
        <button class="tray-text-action" type="button" @click="openPermissionSettings">
          {{ t('permission.openSettings') }}
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
    <ScreenRecordingPermissionModal ref="permissionModal" />
  </main>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, provide, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import QRCard from './QRCard.vue';
import SourcePicker from './SourcePicker.vue';
import StartButton from './StartButton.vue';
import ConnectedDevicesListDrawer from './ConnectedDevicesListDrawer.vue';
import SettingsOverlay from './SettingsOverlay.vue';
import ScreenRecordingPermissionModal from './ScreenRecordingPermissionModal.vue';
import { PermissionModalKey, type ProvidedPermissionModal } from './PermissionModalHost';
import { api } from '../utils/api';

const { t } = useI18n();
const showDevices = ref(false);
const showSettings = ref(false);
const sharing = ref(false);
const viewerCount = ref(0);
const permissionGranted = ref(false);
const permissionModal: ProvidedPermissionModal = ref(null);
provide(PermissionModalKey, permissionModal);
let poll: number | undefined;

async function refreshState() {
  try {
    viewerCount.value = (await api.getConnectedDevices()).length;
    if (viewerCount.value > 0) sharing.value = true;
  } catch {
    /* tolerate startup races while the host server comes up */
  }
  await checkPermission();
}

async function checkPermission() {
  try {
    permissionGranted.value = await api.checkScreenRecordingPermission();
  } catch {
    permissionGranted.value = false;
  }
}

async function openPermissionSettings() {
  try {
    await api.openScreenRecordingSettings();
  } catch {
    await permissionModal.value?.checkAndShow();
  }
}

async function onReset() {
  sharing.value = false;
  viewerCount.value = 0;
  await checkPermission();
}

async function close() {
  try {
    await api.closeTrayPanel();
  } catch {
    window.close();
  }
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
  padding: 18px;
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
  margin-bottom: 16px;
}

.tray-brand {
  gap: 10px;
}

.tray-mark {
  width: 30px;
  height: 30px;
  border: 2px solid var(--accent);
  border-radius: 9px;
  box-shadow: 5px 5px 0 -2px var(--bg), 5px 5px 0 0 var(--accent);
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
  font-size: 22px;
  font-weight: 500;
}

.tray-close {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  color: var(--muted);
  font-size: 23px;
  line-height: 1;
  border: 0;
  border-radius: 50%;
  background: var(--surface-2);
  cursor: pointer;
}

.tray-close:hover {
  color: var(--text-strong);
  background: var(--surface-3);
}

:deep(.qr-card) {
  grid-template-columns: 176px minmax(0, 1fr);
  gap: 14px;
  padding: 14px;
}

:deep(.qr-frame) {
  width: 160px;
  height: 160px;
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
  margin-top: 14px;
}

.tray-controls,
.tray-permission {
  margin-top: 14px;
  padding-top: 14px;
  border-top: var(--line);
}

.tray-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-top: 10px;
}

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
  background: var(--surface-2);
}

.tray-action:hover,
.tray-text-action:hover {
  color: var(--text-strong);
  border-color: var(--accent);
}

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
