<template>
  <div class="player-view">
    <PlayerControlPanel
      v-if="status === 'streaming'"
      :playing="playing"
      :quality="quality"
      @play-pause="togglePlay"
      @quality="setQuality"
      @fullscreen="toggleFullscreen"
    />
    <video
      v-show="status === 'streaming'"
      ref="videoEl"
      class="frame"
      autoplay
      playsinline
      muted
    />

    <div v-if="status !== 'streaming'" class="player-center">
      <div v-if="status === 'disconnected'" class="player-disconnected" role="alert">
        <p class="player-center-title">{{ t('player.disconnected') }}</p>
        <button class="player-reconnect" type="button" @click="reconnect">
          {{ t('player.reconnect') }}
        </button>
      </div>
      <div v-else class="player-loading" role="status" aria-live="polite">
        <svg class="spinner" viewBox="0 0 32 32" width="32" height="32" aria-hidden="true">
          <circle
            cx="16"
            cy="16"
            r="12"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-dasharray="60 30"
          />
        </svg>
        <p class="player-center-title">{{ t('player.connecting') }}</p>
      </div>
    </div>

    <div v-if="status !== 'idle'" class="player-status" :data-state="status">
      <span class="player-status-dot" />
      <span>{{ statusText }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useI18n } from 'vue-i18n';
import PlayerControlPanel from '../components/PlayerControlPanel.vue';
import { useViewerStatus } from '../lib/viewerStatus';

const { t } = useI18n();
const { status, markStreaming, markDisconnected, reset } = useViewerStatus();

const playing = ref(true);
const quality = ref('100%');
const videoEl = ref<HTMLVideoElement | null>(null);

const statusText = computed(() => {
  if (status.value === 'streaming') return t('player.streaming');
  if (status.value === 'disconnected') return t('player.disconnected');
  return t('player.connecting');
});

const emit = defineEmits<{
  (e: 'state', step: number): void;
  (e: 'error', message: string): void;
}>();

function togglePlay() {
  playing.value = !playing.value;
  if (!videoEl.value) return;
  if (playing.value) void videoEl.value.play().catch(() => {});
  else videoEl.value.pause();
}

function setQuality(q: string) {
  quality.value = q;
}

function toggleFullscreen() {
  if (!videoEl.value) return;
  if (document.fullscreenElement) void document.exitFullscreen();
  else void videoEl.value.requestFullscreen();
}

function onStream(event: Event) {
  const stream = (event as CustomEvent<MediaStream>).detail;
  if (!(stream instanceof MediaStream) || !videoEl.value) return;
  videoEl.value.srcObject = stream;
  void videoEl.value.play().catch(() => {});
  (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
  markStreaming();
}

function reconnect() {
  // Full page reload is the simplest reliable way to re-establish both the
  // signaling WebSocket and the RTCPeerConnection. Matches the existing
  // 'reinitiate' button behavior in ConnectionPrompts.vue.
  window.location.reload();
}

let disconnectedTimer: number | undefined;
function scheduleDisconnected() {
  if (disconnectedTimer !== undefined) return;
  disconnectedTimer = window.setTimeout(() => {
    disconnectedTimer = undefined;
    // Only flip if we never reached streaming (host went away during
    // connect). If we *were* streaming, the pc.ontrack ended / ws close
    // path keeps status at 'streaming' until the caller decides.
    if (status.value !== 'streaming') markDisconnected();
  }, 5000);
}

onMounted(() => {
  status.value = 'connecting';
  window.addEventListener('viewer-stream', onStream);
  // If the signaling socket closes before media arrives, surface that
  // to the viewer with a short delay so we don't flash disconnected on
  // transient reconnects.
  const ws = (window as unknown as { __smDebugWs?: WebSocket }).__smDebugWs;
  if (ws) {
    ws.addEventListener('close', scheduleDisconnected);
    ws.addEventListener('error', scheduleDisconnected);
  }
  emit('state', 2);
});

onBeforeUnmount(() => {
  window.removeEventListener('viewer-stream', onStream);
  if (videoEl.value) videoEl.value.srcObject = null;
  if (disconnectedTimer !== undefined) window.clearTimeout(disconnectedTimer);
  // reset is exported so callers / tests can drive transitions if needed.
  void reset;
});
</script>

<style scoped>
.player-view {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: black;
}

.frame {
  flex: 1;
  width: 100%;
  min-height: 0;
  object-fit: contain;
  background: black;
}

.player-center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  color: var(--muted);
  pointer-events: none;
}

.player-center > * {
  pointer-events: auto;
}

.player-center-title {
  margin: 0;
  font-size: 14px;
  letter-spacing: 0.02em;
}

.spinner {
  color: var(--muted);
  animation: spin 0.9s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spinner {
    animation: none;
  }
}

.player-disconnected {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.player-reconnect {
  background: var(--accent);
  color: #0a1413;
  border: none;
  padding: 8px 20px;
  border-radius: 999px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s ease;
}

.player-reconnect:hover {
  background: #a8efe4;
}

.player-status {
  position: absolute;
  bottom: 12px;
  right: 12px;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-radius: 999px;
  background: rgba(14, 17, 22, 0.72);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: var(--text);
  font-size: 12px;
  letter-spacing: 0.02em;
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  pointer-events: none;
}

.player-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--muted);
  flex-shrink: 0;
}

.player-status[data-state='connecting'] .player-status-dot {
  background: var(--muted);
  animation: pulse 1.4s ease-in-out infinite;
}

.player-status[data-state='streaming'] .player-status-dot {
  background: var(--accent);
}

.player-status[data-state='disconnected'] .player-status-dot {
  background: #e27d60;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.4;
  }
  50% {
    opacity: 1;
  }
}

@media (prefers-reduced-motion: reduce) {
  .player-status[data-state='connecting'] .player-status-dot {
    animation: none;
    opacity: 1;
  }
}
</style>
