<template>
  <div class="main-view">
    <ConnectionPrompts v-if="!streaming" :step="promptStep" @reinitiate="reload" />
    <PlayerView v-if="streaming" :room-id="roomId" @state="onState" @error="onError" />
    <MyDeviceInfoCard :device="device" />
    <ErrorDialog :open="!!errorMessage" :message="errorMessage" @close="errorMessage = ''" />
    <button class="privacy-btn" @click="showPrivacy = true" :aria-label="t('privacy.title')">
      {{ t('privacy.title') }}
    </button>
    <PrivacyDialog :open="showPrivacy" @close="showPrivacy = false" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import ConnectionPrompts from './ConnectionPrompts.vue';
import PlayerView from './PlayerView.vue';
import MyDeviceInfoCard from '../components/MyDeviceInfoCard.vue';
import ErrorDialog from '../components/ErrorDialog.vue';
import PrivacyDialog from '../components/PrivacyDialog.vue';

const props = defineProps<{ ws: WebSocket | null; connectSignal: number }>();
void props;
const { t } = useI18n();
const roomId = ref('');
const promptStep = ref(1);
const streaming = ref(false);
const errorMessage = ref('');
const device = ref({ ip: '127.0.0.1', os: '', browser: '', deviceType: 'browser', roomId: '' });
const showPrivacy = ref(false);

function onState(step: number) {
  promptStep.value = step;
  if (step === 2) streaming.value = true;
}

function onError(message: string) {
  if (message === 'NOT_ALLOWED') errorMessage.value = t('viewer.notAllowed');
  else if (message === 'DISCONNECTED') errorMessage.value = t('viewer.disconnected');
  else errorMessage.value = message;
}

function reload() {
  window.location.reload();
}

onMounted(() => {
  roomId.value = window.location.pathname.replace(/^\//, '').split('/')[0] || '';
  device.value.roomId = roomId.value;
  const pageHost = window.location.hostname;
  if (pageHost && pageHost !== 'localhost' && pageHost !== '127.0.0.1') device.value.ip = pageHost;
  device.value.os = navigator.platform || '';
  device.value.browser = navigator.userAgent.split(' ').pop() ?? '';

  const onViewerIp = (ev: Event) => {
    const ip = (ev as CustomEvent<string>).detail;
    if (ip && ip !== '127.0.0.1') device.value.ip = ip;
  };
  window.addEventListener('viewer-ip', onViewerIp);

  window.addEventListener('viewer-signal', (ev) => {
    const msg = (ev as CustomEvent).detail as { type: string };
    if (msg.type === 'ALLOWED_TO_CONNECT' || msg.type === 'ANSWER') {
      promptStep.value = 2;
      streaming.value = true;
    } else if (msg.type === 'NOT_ALLOWED') onError('NOT_ALLOWED');
  });
});
</script>

<style scoped>
.main-view { display: flex; flex-direction: column; min-height: 100vh; background: rgba(240, 248, 250, 1); }
.privacy-btn { position: fixed; bottom: 16px; right: 16px; background: white; border: 1px solid #ddd; border-radius: 999px; padding: 6px 14px; font-size: 12px; cursor: pointer; z-index: 100; }
.privacy-btn:hover { background: #f6f6f6; }
</style>
