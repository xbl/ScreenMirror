<template>
  <section class="qr-card" :data-state="state">
    <div class="qr-frame">
      <img
        v-if="qrDataUrl"
        :src="qrDataUrl"
        alt="QR code"
        width="232"
        height="232"
        class="qr-img"
      />
      <div v-else class="qr-empty">
        <span class="qr-empty-dot" />
        <span class="qr-empty-dot" />
        <span class="qr-empty-dot" />
      </div>
    </div>
    <div class="qr-meta">
      <div class="qr-url" :title="url">{{ url || '—' }}</div>
      <div class="qr-status">
        <span class="qr-status-dot" :class="{ live: !!url }" />
        <span class="qr-status-text">{{ statusText }}</span>
      </div>
      <div class="qr-actions">
        <button
          class="btn btn-accent"
          :disabled="!url || copying"
          @click="onCopy"
        >
          {{ copyLabel }}
        </button>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import QRCode from 'qrcode';
import { api } from '../utils/api';

type HostInfo = {
  lan_ip?: string;
  port?: number;
  room_id?: string | null;
};

const { t } = useI18n();

const url = ref('');
const qrDataUrl = ref('');
const copying = ref(false);
const copyState = ref<'idle' | 'done' | 'failed'>('idle');
const errorMessage = ref('');
const lastFetchError = ref('');

let pollTimer: number | undefined;
let roomSessionReady = false;

const state = computed<'idle' | 'ready' | 'busy' | 'unavailable'>(() => {
  if (!url.value) return 'idle';
  return 'ready';
});

const statusText = computed(() => {
  if (!url.value) return errorMessage.value || t('card.waitingForServer');
  return t('card.sameWifi');
});

const copyLabel = computed(() => {
  if (copying.value) return t('card.copy');
  if (copyState.value === 'done') return t('card.copied');
  if (copyState.value === 'failed') return t('card.copyFailed');
  return t('card.copy');
});

async function fetchHostInfo(): Promise<HostInfo | null> {
  const errors: string[] = [];

  // 1) Primary: Tauri IPC. Inside the Tauri shell the WebView's origin is
  //    tauri://localhost (or the vite dev URL), neither of which routes
  //    /api/host-info to the axum server. IPC is the only path that works
  //    here, so try it first.
  let ip: string | null = null;
  let port = 0;
  try {
    [ip, port] = await Promise.all([api.getLanIp(), api.getPort()]);
  } catch (e: any) {
    errors.push(`ipc:${e?.message ?? e}`);
  }
  // The host shell writes the current waiting-room id here after
  // createWaitingSession(). The QR URL needs the room id in the path so
  // the viewer knows which room to dial over WebSocket.
  let roomId: string | null = null;
  if (!roomSessionReady) {
    try {
      roomId = await api.createWaitingSession(undefined);
      roomSessionReady = true;
      window.localStorage.setItem('sm:roomId', roomId);
    } catch {
      /* keep the last room id as a fallback while the backend starts */
      try {
        roomId = window.localStorage.getItem('sm:roomId');
      } catch {
        /* ignore */
      }
    }
  } else {
    try {
      roomId = window.localStorage.getItem('sm:roomId');
    } catch {
      /* ignore */
    }
  }
  if (ip && port) {
    try {
      return {
        lan_ip: ip,
        port,
        room_id: roomId,
      };
    } catch (e: any) {
      errors.push(`devices:${e?.message ?? e}`);
      return {
        lan_ip: ip,
        port,
        room_id: roomId,
      };
    }
  }

  // 1b) If we know the port but not the LAN IP, fall back to 127.0.0.1.
  if (port && !ip) {
    return {
      lan_ip: '127.0.0.1',
      port,
      room_id: roomId,
    };
  }

  // 2) Fallback: try fetching the host-info endpoint directly from the axum
  //    server. The Tauri WebView origin is tauri://localhost which won't
  //    resolve via relative fetch, so probe well-known loopback ports.
  const probePorts = [3131, 3132, 3133, 3134];
  for (const p of probePorts) {
    try {
      const r = await fetch(`http://127.0.0.1:${p}/api/host-info`, { cache: 'no-store' });
      if (r.ok) {
        const ct = r.headers.get('content-type') ?? '';
        if (ct.includes('application/json')) {
          const j = (await r.json()) as HostInfo;
          if (j.lan_ip && j.port) return j;
        }
      }
    } catch (e: any) {
      errors.push(`fetch:${p}:${e?.message ?? e}`);
    }
  }

  // 3) Last resort: relative fetch (works only when the page is served by
  //    the axum server itself — headless / preview environments).
  try {
    const r = await fetch('/api/host-info', { cache: 'no-store' });
    if (r.ok) {
      const ct = r.headers.get('content-type') ?? '';
      if (ct.includes('application/json')) {
        return (await r.json()) as HostInfo;
      }
      errors.push(`rel:${ct}`);
    }
  } catch (e: any) {
    errors.push(`rel:${e?.message ?? e}`);
  }

  // Stash the last error so the UI can show it for debugging.
  lastFetchError.value = errors.join(' | ');
  return null;
}

function buildUrl(info: HostInfo): string {
  if (!info.lan_ip || !info.port) return '';
  const path = info.room_id ? `/${info.room_id}` : '';
  return `http://${info.lan_ip}:${info.port}${path}`;
}

async function renderQr(value: string) {
  if (!value) {
    qrDataUrl.value = '';
    return;
  }
  try {
    qrDataUrl.value = await QRCode.toDataURL(value, {
      errorCorrectionLevel: 'H',
      width: 232,
      margin: 1,
      color: { dark: '#0E1116', light: '#E8E4DC' },
    });
  } catch {
    qrDataUrl.value = '';
  }
}

async function refresh() {
  const info = await fetchHostInfo();
  if (!info) {
    url.value = '';
    errorMessage.value = lastFetchError.value || '';
    return;
  }
  url.value = buildUrl(info);
  errorMessage.value = '';
  await renderQr(url.value);
}

async function onCopy() {
  if (!url.value || copying.value) return;
  copying.value = true;
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(url.value);
      copyState.value = 'done';
    } else {
      // Legacy fallback for older WebViews.
      const ta = document.createElement('textarea');
      ta.value = url.value;
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand('copy');
      document.body.removeChild(ta);
      copyState.value = ok ? 'done' : 'failed';
    }
    setTimeout(() => {
      copyState.value = 'idle';
    }, 1600);
  } catch {
    copyState.value = 'failed';
    setTimeout(() => {
      copyState.value = 'idle';
    }, 2200);
  } finally {
    copying.value = false;
  }
}

watch(url, (v) => {
  void renderQr(v);
});

onMounted(() => {
  void refresh();
  pollTimer = window.setInterval(refresh, 1500);
});
onBeforeUnmount(() => {
  if (pollTimer) clearInterval(pollTimer);
});
</script>

<style scoped>
.qr-card {
  display: grid;
  grid-template-columns: 264px 1fr;
  gap: var(--sp-6);
  padding: var(--sp-5);
  background: var(--surface);
  border: var(--line);
  border-radius: var(--radius-lg);
}

.qr-frame {
  width: 232px;
  height: 232px;
  background: var(--text);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: opacity var(--motion) ease;
}

.qr-card[data-state='idle'] .qr-frame {
  opacity: 0.55;
}

.qr-img {
  display: block;
  width: 100%;
  height: 100%;
  image-rendering: pixelated;
}

.qr-empty {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  width: 60%;
  height: 60%;
  align-items: center;
  justify-items: center;
}

.qr-empty-dot {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--muted-2, #5d5952);
  animation: pulse 1.4s ease-in-out infinite;
}
.qr-empty-dot:nth-child(2) {
  animation-delay: 0.2s;
}
.qr-empty-dot:nth-child(3) {
  animation-delay: 0.4s;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.25;
    transform: scale(0.9);
  }
  50% {
    opacity: 1;
    transform: scale(1.05);
  }
}

.qr-meta {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 0;
  gap: var(--sp-3);
}

.qr-url {
  font-family: var(--font-mono);
  font-size: var(--fs-13);
  color: var(--text);
  letter-spacing: -0.01em;
  word-break: break-all;
  line-height: 1.45;
  max-height: 4.4em;
  overflow: hidden;
}

.qr-status {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-13);
  color: var(--muted);
}

.qr-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--muted-2, #5d5952);
  flex-shrink: 0;
}

.qr-status-dot.live {
  background: var(--accent);
  box-shadow: 0 0 0 4px var(--accent-dim);
}

.qr-actions {
  display: flex;
  gap: var(--sp-2);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 10px 18px;
  border-radius: var(--radius-pill);
  font-size: var(--fs-14);
  font-weight: 500;
  letter-spacing: -0.005em;
  transition:
    background var(--motion) ease,
    color var(--motion) ease,
    border-color var(--motion) ease,
    opacity var(--motion) ease;
  border: 1px solid transparent;
}

.btn-accent {
  background: var(--accent);
  color: #0a1413;
}
.btn-accent:hover:not(:disabled) {
  background: var(--accent-strong);
}
.btn-accent:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

@media (max-width: 720px) {
  .qr-card {
    grid-template-columns: 1fr;
    justify-items: center;
  }
  .qr-meta {
    align-items: center;
    text-align: center;
  }
}

@media (prefers-reduced-motion: reduce) {
  .qr-empty-dot {
    animation: none;
  }
  .qr-frame {
    transition: none;
  }
}
</style>
