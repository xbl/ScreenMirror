<template>
  <div class="host-shell">
    <TopBar
      :viewer-count="viewerCount"
      @open-devices="showDevices = true"
      @open-settings="showSettings = true"
    />

    <main class="host-main">
      <div class="host-inner">
        <header class="host-hero">
          <h1 class="host-headline">{{ t('hero.headline') }}</h1>
          <p class="host-subhead">{{ t('hero.subhead') }}</p>
        </header>

        <QRCard class="host-qr" />

        <section
          v-if="pendingDevice"
          class="host-approval"
          role="region"
          :aria-label="t('devices.title')"
        >
          <div class="host-approval-meta">
            <span class="host-approval-eyebrow">{{ t('devices.title') }}</span>
            <span class="host-approval-id">{{ pendingDevice.ip }}</span>
            <span class="host-approval-sub">{{ pendingDevice.os }} · {{ pendingDevice.browser }}</span>
          </div>
          <div class="host-approval-actions">
            <button class="host-approval-deny" type="button" @click="onDeny">
              {{ t('devices.deny') }}
            </button>
            <button class="host-approval-approve" type="button" @click="onApprove">
              {{ t('devices.approve') }}
            </button>
          </div>
        </section>

        <SourcePicker class="host-source" />

        <StartButton
          class="host-start"
          v-model:sharing="sharing"
          v-model:viewerCount="viewerCount"
        />
      </div>
    </main>

    <ConnectedDevicesListDrawer
      :open="showDevices"
      @close="showDevices = false"
    />
    <SettingsOverlay
      :open="showSettings"
      @close="showSettings = false"
      @reset="onReset"
    />
    <ScreenRecordingPermissionModal />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { useI18n } from 'vue-i18n';
import TopBar from './TopBar.vue';
import QRCard from './QRCard.vue';
import SourcePicker from './SourcePicker.vue';
import StartButton from './StartButton.vue';
import ConnectedDevicesListDrawer from './ConnectedDevicesListDrawer.vue';
import SettingsOverlay from './SettingsOverlay.vue';
import ScreenRecordingPermissionModal from './ScreenRecordingPermissionModal.vue';
import { api, type Device } from '../utils/api';

const { t } = useI18n();

const showDevices = ref(false);
const showSettings = ref(false);
const sharing = ref(false);
const viewerCount = ref(0);
const pendingDevice = ref<Device | null>(null);

let poll: number | undefined;

async function refreshState() {
  try {
    const [devs, pending] = await Promise.all([
      api.getConnectedDevices(),
      api.getPendingDevice(),
    ]);
    viewerCount.value = devs.length;
    pendingDevice.value = pending;
    // sharing is now driven by user intent (Start/Stop), not by the
    // connected-devices count. A viewer may briefly be 'pending' before
    // being approved, and we don't want the button to flip back to
    // 'Start sharing' during that window.
    if (devs.length > 0 && !sharing.value) sharing.value = true;
  } catch {
    /* tolerate headless */
  }
}

async function onApprove() {
  try {
    await api.setDeviceConnectedStatus();
    pendingDevice.value = null;
    await refreshState();
  } catch {
    /* ignore */
  }
}

async function onDeny() {
  try {
    await api.disconnectAllDevices();
    pendingDevice.value = null;
  } catch {
    /* ignore */
  }
}

async function onReset() {
  sharing.value = false;
  viewerCount.value = 0;
  pendingDevice.value = null;
}

onMounted(async () => {
  // Make sure there is a waiting session so the QR has a room id.
  try {
    const roomId = await api.createWaitingSession(undefined);
    if (roomId) {
      try {
        window.localStorage.setItem('sm:roomId', roomId);
      } catch {
        /* ignore */
      }
    }
  } catch {
    /* ignore */
  }
  await refreshState();
  poll = window.setInterval(refreshState, 2000);
});

onBeforeUnmount(() => {
  if (poll) clearInterval(poll);
});
</script>

<style scoped>
.host-shell {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  background: var(--bg);
}

.host-main {
  flex: 1;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: var(--sp-12) var(--sp-6) var(--sp-10);
}

.host-inner {
  width: 100%;
  max-width: 720px;
  display: flex;
  flex-direction: column;
  gap: var(--sp-8);
}

.host-hero {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.host-headline {
  font-family: var(--font-display);
  font-size: var(--fs-36);
  line-height: 1.1;
  letter-spacing: -0.02em;
  color: var(--text-strong);
  font-weight: 400;
}

.host-subhead {
  font-size: var(--fs-15);
  color: var(--muted);
  max-width: 44ch;
}

.host-qr {
  /* No extra styling — the component owns its look. */
}

.host-approval {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-4);
  padding: var(--sp-4) var(--sp-5);
  border: var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  border-left: 3px solid var(--accent);
}

.host-approval-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.host-approval-eyebrow {
  font-size: var(--fs-12);
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--accent);
}

.host-approval-id {
  font-family: var(--font-mono);
  font-size: var(--fs-14);
  color: var(--text);
}

.host-approval-sub {
  font-size: var(--fs-12);
  color: var(--muted);
}

.host-approval-actions {
  display: flex;
  gap: var(--sp-2);
  flex-shrink: 0;
}

.host-approval-approve,
.host-approval-deny {
  padding: 8px 16px;
  border-radius: var(--radius-pill);
  font-size: var(--fs-13);
  border: 1px solid var(--border-strong);
  color: var(--text);
  background: transparent;
  transition: background var(--motion) ease, border-color var(--motion) ease;
}

.host-approval-approve {
  background: var(--accent);
  color: #0a1413;
  border-color: var(--accent);
  font-weight: 500;
}

.host-approval-approve:hover {
  background: var(--accent-strong);
  border-color: var(--accent-strong);
}

.host-approval-deny:hover {
  background: var(--surface-2);
  border-color: var(--danger);
  color: var(--danger);
}

.host-source {
  /* ditto */
}

.host-start {
  /* ditto */
}

@media (max-width: 720px) {
  .host-main {
    padding: var(--sp-8) var(--sp-4) var(--sp-6);
  }
  .host-headline {
    font-size: var(--fs-28);
  }
}
</style>