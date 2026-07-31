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
    <ScreenRecordingPermissionModal ref="permissionModal" />
  </div>
</template>

<script setup lang="ts">
import { ref, provide, onMounted, onBeforeUnmount } from 'vue';
import { PermissionModalKey, type ProvidedPermissionModal } from './PermissionModalHost';
import { useI18n } from 'vue-i18n';
import TopBar from './TopBar.vue';
import QRCard from './QRCard.vue';
import SourcePicker from './SourcePicker.vue';
import StartButton from './StartButton.vue';
import ConnectedDevicesListDrawer from './ConnectedDevicesListDrawer.vue';
import SettingsOverlay from './SettingsOverlay.vue';
import ScreenRecordingPermissionModal from './ScreenRecordingPermissionModal.vue';
import { api } from '../utils/api';

const { t } = useI18n();

const showDevices = ref(false);
const showSettings = ref(false);
const sharing = ref(false);
const viewerCount = ref(0);
const permissionModal: ProvidedPermissionModal = ref(null);
provide(PermissionModalKey, permissionModal);

let poll: number | undefined;

async function refreshState() {
  try {
    const devs = await api.getConnectedDevices();
    viewerCount.value = devs.length;
    // sharing is now driven by user intent (Start/Stop), not by the
    // connected-devices count. A viewer may briefly be 'pending' before
    // being approved, and we don't want the button to flip back to
    // 'Start sharing' during that window.
    if (devs.length > 0 && !sharing.value) sharing.value = true;
  } catch {
    /* tolerate headless */
  }
}

async function onReset() {
  sharing.value = false;
  viewerCount.value = 0;
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
