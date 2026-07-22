<template>
  <MainView
    :ws="ws"
    :connect-signal="connectSignal"
  />
  <EarlyOffer v-if="ws && roomId" :ws="ws" :room-id="roomId" @ready="onEarlyOffer" />
</template>

<script setup lang="ts">
import { provide, ref } from 'vue';
import MainView from './views/MainView.vue';
import EarlyOffer from './components/EarlyOffer.vue';
import { connectSignaling, type WireMessage } from './lib/signaling';

const roomId = (window.location.pathname.replace(/^\//, '').split('/')[0] || '');
const ws = ref<WebSocket | null>(null);
const connectSignal = ref(0); // increment to trigger MainView to mount PlayerView

function onEarlyOffer() {
  // PlayerView is mounted; it will create its own PC and reuse the same data channel.
  connectSignal.value++;
}

if (roomId) {
  const socket = connectSignaling(
    roomId,
    (msg: WireMessage) => {
      console.log('[app-vue] ws msg:', msg.type, msg.payload ? JSON.stringify(msg.payload).slice(0, 80) : '');
      window.dispatchEvent(new CustomEvent('viewer-signal', { detail: msg }));
      if (msg.type === 'ALLOWED_TO_CONNECT') {
        connectSignal.value++;
      }
    },
  );
  socket.addEventListener('message', (e) => {
    console.log('[ws-raw]', String(e.data).slice(0, 120));
  });
  (window as unknown as { __smDebugWs: WebSocket }).__smDebugWs = socket;
  ws.value = socket;
}

provide('ws', ws);
provide('connectSignal', connectSignal);
provide('roomId', roomId);
</script>

<style>
/* Minimal baseline so the viewer page is never transparent white before
   the dialog cards paint. */
:root {
  color-scheme: dark;
  --bg: #0e1116;
  --surface: #161a21;
  --surface-2: #1d222b;
  --text: #e8e4dc;
  --muted: #8a8579;
  --accent: #7be0d2;
}
html,
body,
#app {
  margin: 0;
  padding: 0;
  min-height: 100%;
  background: var(--bg);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui,
    sans-serif;
  font-size: 15px;
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}
</style>