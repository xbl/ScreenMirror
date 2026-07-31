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
:root {
  color-scheme: dark;
  --bg: #0e1116;
  --surface: #161a21;
  --surface-2: #1d222b;
  --surface-3: #242a35;
  --border: rgba(255, 255, 255, 0.06);
  --border-strong: rgba(255, 255, 255, 0.12);
  --text: #e8e4dc;
  --text-strong: #f7f4ee;
  --muted: #8a8579;
  --muted-2: #5d5952;
  --accent: #7be0d2;
  --accent-strong: #a3ecdf;
  --accent-dim: rgba(123, 224, 210, 0.12);
  --accent-line: rgba(123, 224, 210, 0.32);
  --danger: #e27d60;
  --font-display: 'Fraunces', 'Newsreader', 'Iowan Old Style', Georgia, 'Times New Roman', serif;
  --font-body: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Inter', 'Segoe UI', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 14px;
  --radius-pill: 999px;
  --fs-12: 0.75rem;
  --fs-14: 0.875rem;
  --fs-15: 0.9375rem;
  --fs-28: 1.75rem;
  --sp-6: 24px;
  --motion: 180ms;
}
html,
body,
#app {
  margin: 0;
  padding: 0;
  min-height: 100%;
  background: var(--bg);
  color: var(--text);
  font-family: var(--font-body);
  font-size: var(--fs-15);
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}
</style>
