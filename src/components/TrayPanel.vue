<template>
  <main class="tray-panel">
    <header class="tray-head">
      <div class="tray-brand">
        <img class="tray-logo" :src="logoUrl" alt="ScreenMirror" />
        <div>
          <p class="tray-kicker">ScreenMirror</p>
          <h1>{{ t('tray.title') }}</h1>
        </div>
      </div>
      <div class="tray-head-actions">
        <button
          class="tray-devices"
          type="button"
          :aria-label="t('devices.title')"
          @click="showDevices = true"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M7 4h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Zm0 4h10M9 16h6"
            />
          </svg>
          <strong v-if="viewerCount > 0">{{ viewerCount }}</strong>
        </button>
        <button
          class="tray-settings"
          type="button"
          :aria-label="t('settings.title')"
          @click="showSettings = true"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" />
            <path
              d="m19.4 15 .1.1a1.8 1.8 0 0 1-2.5 2.5l-.1-.1a1.8 1.8 0 0 0-3 .9v.2a1.8 1.8 0 0 1-3.6 0v-.2a1.8 1.8 0 0 0-3-.9l-.1.1a1.8 1.8 0 1 1-2.5-2.5l.1-.1a1.8 1.8 0 0 0-.9-3H3.7a1.8 1.8 0 0 1 0-3h.2a1.8 1.8 0 0 0 .9-3l-.1-.1a1.8 1.8 0 1 1 2.5-2.5l.1.1a1.8 1.8 0 0 0 3-.9v-.2a1.8 1.8 0 0 1 3.6 0v.2a1.8 1.8 0 0 0 3 .9l.1-.1a1.8 1.8 0 1 1 2.5 2.5l-.1.1a1.8 1.8 0 0 0 .9 3h.2a1.8 1.8 0 0 1 0 3h-.2a1.8 1.8 0 0 0-.9 3Z"
            />
          </svg>
          <span v-if="newVersionAvailable" class="tray-settings-dot" aria-hidden="true" />
        </button>
      </div>
    </header>

    <div v-if="newVersionAvailable && showUpdateBanner" class="tray-update-banner" role="status">
      <div class="tray-update-badge">
        <span class="tray-update-icon" aria-hidden="true">🚀</span>
        <div class="tray-update-text">
          <strong>{{ t('updater.bannerTitle', { version: newVersionAvailable }) }}</strong>
        </div>
      </div>
      <div class="tray-update-actions">
        <button class="tray-update-btn" type="button" @click="showUpdateModal = true">
          {{ t('updater.bannerAction') }}
        </button>
        <button
          class="tray-update-dismiss"
          type="button"
          aria-label="Dismiss"
          @click="showUpdateBanner = false"
        >
          ×
        </button>
      </div>
    </div>

    <div v-if="!permissionGranted" class="tray-permission-banner" role="alert">
      <div class="tray-perm-badge">
        <span class="tray-perm-icon" aria-hidden="true">⚠️</span>
        <div class="tray-perm-text">
          <strong>{{ t('permission.bannerTitle') }}</strong>
          <p>{{ t('permission.bannerDesc') }}</p>
        </div>
      </div>
      <button class="tray-perm-btn" type="button" @click="showPermissionGuide = true">
        {{ t('permission.grantAction') }}
      </button>
    </div>

    <QRCard class="tray-qr" />
    <SourcePicker class="tray-source" external-chooser />

    <section class="tray-controls" aria-label="Share controls">
      <div class="tray-actions">
        <button class="tray-action tray-exit" type="button" @click="exitApp">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M10 5H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h4M13 8l4 4-4 4M9 12h8" />
          </svg>
          <span>{{ t('app.exit') }}</span>
        </button>
      </div>
    </section>

    <ConnectedDevicesListDrawer :open="showDevices" @close="showDevices = false" />
    <SettingsOverlay :open="showSettings" @close="showSettings = false" @reset="onReset" />
    <PermissionGuideModal
      :open="showPermissionGuide"
      @close="showPermissionGuide = false"
      @granted="checkPermission"
    />
    <UpdateModal
      :open="showUpdateModal"
      :update-info="updateInfo"
      :checking="checkingUpdate"
      :error="updateError"
      @close="showUpdateModal = false"
      @check="() => runUpdateCheck(true)"
    />
  </main>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import QRCard from './QRCard.vue';
import SourcePicker from './SourcePicker.vue';
import ConnectedDevicesListDrawer from './ConnectedDevicesListDrawer.vue';
import SettingsOverlay from './SettingsOverlay.vue';
import PermissionGuideModal from './PermissionGuideModal.vue';
import UpdateModal from './UpdateModal.vue';
import { api, type UpdateInfo } from '../utils/api';
import logoUrl from '../../src-tauri/icons/icon.png';

const { t } = useI18n();
const showDevices = ref(false);
const showSettings = ref(false);
const showPermissionGuide = ref(false);
const permissionGranted = ref(true);
const viewerCount = ref(0);

const newVersionAvailable = ref<string | null>(null);
const showUpdateBanner = ref(true);
const showUpdateModal = ref(false);
const checkingUpdate = ref(false);
const updateInfo = ref<UpdateInfo | null>(null);
const updateError = ref<string | null>(null);

let poll: number | undefined;
let unlistenTray: UnlistenFn | undefined;

async function runUpdateCheck(manual = false) {
  if (manual) {
    showUpdateModal.value = true;
  }
  checkingUpdate.value = true;
  updateError.value = null;
  try {
    const info = await api.checkForUpdates();
    updateInfo.value = info;
    if (info.hasUpdate) {
      newVersionAvailable.value = info.latestVersion;
      showUpdateBanner.value = true;
    } else {
      newVersionAvailable.value = null;
    }
  } catch (err) {
    updateError.value = err instanceof Error ? err.message : String(err);
  } finally {
    checkingUpdate.value = false;
  }
}

async function checkUpdateOnStartup() {
  try {
    const autoCheck = await api.getAutoCheckUpdate();
    if (autoCheck) {
      window.setTimeout(() => {
        void runUpdateCheck(false);
      }, 1500);
    }
  } catch {
    /* ignore */
  }
}

async function checkPermission() {
  try {
    let granted = await api.checkScreenRecordingPermission();
    if (!granted) {
      try {
        const sources = await api.enumerateCaptureSources();
        if (sources.length > 0) {
          granted = true;
        }
      } catch {
        // keep false
      }
    }
    permissionGranted.value = granted;
  } catch {
    /* best effort */
  }
}

async function refreshState() {
  try {
    viewerCount.value = (await api.getConnectedDevices()).length;
    if (!permissionGranted.value) {
      void checkPermission();
    }
  } catch {
    /* tolerate startup races while the host server comes up */
  }
}

async function onReset() {
  viewerCount.value = 0;
  void checkPermission();
}

async function exitApp() {
  await api.exitApp();
}

onMounted(() => {
  void refreshState();
  void checkPermission();
  void checkUpdateOnStartup();
  poll = window.setInterval(refreshState, 2000);
  void listen('tray-panel-opened', () => {
    void checkPermission();
  }).then((unlisten) => {
    unlistenTray = unlisten;
  });
});

onBeforeUnmount(() => {
  if (poll) clearInterval(poll);
  if (unlistenTray) unlistenTray();
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
  width: 44px;
  height: 44px;
  object-fit: contain;
}

.tray-kicker {
  margin: 0;
  color: var(--muted);
  font-size: 10px;
  letter-spacing: 0.14em;
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

.tray-settings {
  position: relative;
}

.tray-settings-dot {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 2px var(--control);
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

.tray-update-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 12px;
  padding: 8px 12px;
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  border: 1px solid color-mix(in srgb, var(--accent) 32%, transparent);
  border-radius: var(--radius-md);
  animation: slideDown 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes slideDown {
  from {
    transform: translateY(-8px);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

.tray-update-badge {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.tray-update-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.tray-update-text {
  min-width: 0;
}

.tray-update-text strong {
  display: block;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-strong);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tray-update-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.tray-update-btn {
  padding: 4px 10px;
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  background: var(--accent);
  border: none;
  border-radius: var(--radius-sm, 6px);
  cursor: pointer;
  transition: background 0.15s ease;
  white-space: nowrap;
}

.tray-update-btn:hover {
  background: var(--accent-strong);
}

.tray-update-dismiss {
  width: 20px;
  height: 20px;
  background: transparent;
  border: none;
  color: var(--muted);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}

.tray-update-dismiss:hover {
  background: color-mix(in srgb, var(--border) 60%, transparent);
  color: var(--text);
}

.tray-permission-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 12px;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  border: 1px solid color-mix(in srgb, var(--warn, #e08b38) 35%, transparent);
  background: color-mix(in srgb, var(--warn, #e08b38) 10%, var(--surface));
}

.tray-perm-badge {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.tray-perm-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.tray-perm-text {
  min-width: 0;
}

.tray-perm-text strong {
  display: block;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-strong);
  line-height: 1.3;
}

.tray-perm-text p {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--muted);
  line-height: 1.25;
}

.tray-perm-btn {
  flex-shrink: 0;
  padding: 5px 10px;
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  background: var(--warn, #e08b38);
  border: none;
  border-radius: var(--radius-sm, 6px);
  cursor: pointer;
  transition: opacity 0.15s ease;
}

.tray-perm-btn:hover {
  opacity: 0.9;
}

.tray-controls {
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

.tray-exit svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}

.tray-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
  padding: 10px;
  color: var(--text);
  font-size: 12px;
  text-align: left;
  border: var(--line);
  border-radius: var(--radius-md);
  background: var(--control);
}

.tray-action:hover {
  color: var(--text-strong);
  border-color: var(--accent);
}

.tray-exit:hover {
  color: #fff;
  border-color: var(--danger);
  background: var(--danger);
}

:deep(.source-picker) {
  padding: 14px;
}
</style>
