<template>
  <div class="player-view">
    <video
      v-if="status === 'streaming'"
      ref="videoEl"
      class="frame"
      autoplay
      playsinline
      muted
      controls
      controlslist="nodownload noremoteplayback"
      disablepictureinpicture
      @loadedmetadata="onLoadedMetadata"
      @error="onVideoError"
    />

    <div v-if="status !== 'streaming'" class="player-center">
      <div v-if="noFrames" class="player-disconnected" role="alert">
        <p class="player-center-title">{{ t('player.noFrames') }}</p>
        <button class="player-reconnect" type="button" @click="reconnect">
          {{ t('player.reconnect') }}
        </button>
      </div>
      <div v-else-if="status === 'disconnected'" class="player-disconnected" role="alert">
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

  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue';
import { useI18n } from 'vue-i18n';
import { useViewerStatus } from '../lib/viewerStatus';

const { t } = useI18n();
const { status, markStreaming, markDisconnected, reset } = useViewerStatus();

const videoEl = ref<HTMLVideoElement | null>(null);
const pendingStream = ref<MediaStream | null>(null);
const noFrames = ref(false);

// VideoToolbox can take several seconds to initialize on a real screen capture
// before the first keyframe arrives. Keep the element mounted during that cold
// start; the watchdog is for a genuinely stalled stream, not encoder startup.
const FRAME_WATCHDOG_MS = 10_000;

const emit = defineEmits<{
  (e: 'state', step: number): void;
  (e: 'error', message: string): void;
}>();

function attachStream(stream: MediaStream) {
  const v = videoEl.value;
  if (!v) {
    pendingStream.value = stream;
    return;
  }
  if (v.srcObject === stream) return;
  v.srcObject = stream;
  const playPromise = v.play();
  if (playPromise && typeof playPromise.then === 'function') {
    playPromise.catch((err) => {
      console.error('[player-view] play() rejected:', err);
    });
  }
  startFrameWatchdog();
}

function startFrameWatchdog() {
  if (frameWatchdog !== undefined) window.clearTimeout(frameWatchdog);
  frameWatchdog = window.setTimeout(() => {
    frameWatchdog = undefined;
    const v = videoEl.value;
    if (!v) {
      noFrames.value = true;
      markDisconnected();
      return;
    }
    if (v.videoWidth === 0 || v.videoHeight === 0) {
      console.error('[player-view] no frames after ' + (FRAME_WATCHDOG_MS / 1000) + 's; videoWidth=0 videoHeight=0 readyState=' + v.readyState + ' networkState=' + v.networkState + ' error=' + (v.error ? v.error.code : 'none'));
      noFrames.value = true;
      markDisconnected();
    }
  }, FRAME_WATCHDOG_MS);
}

function onLoadedMetadata() {
  if (frameWatchdog !== undefined) {
    window.clearTimeout(frameWatchdog);
    frameWatchdog = undefined;
  }
  noFrames.value = false;
  console.log('[player-view] loadedmetadata videoWidth=' + (videoEl.value?.videoWidth ?? 0) + ' videoHeight=' + (videoEl.value?.videoHeight ?? 0));
}

function onVideoError(event: Event) {
  const v = videoEl.value;
  console.error('[player-view] <video> error:', event, v?.error);
  noFrames.value = true;
  markDisconnected();
}

function onStream(event: Event) {
  const stream = (event as CustomEvent<MediaStream>).detail;
  if (!(stream instanceof MediaStream)) return;
  for (const track of stream.getTracks()) {
    if (track.readyState === 'ended') {
      console.error('[player-view] track arrived ended:', track.kind, track.id);
      noFrames.value = true;
      markDisconnected();
      return;
    }
    track.addEventListener('ended', () => {
      console.warn('[player-view] track ended:', track.kind, track.id);
      if (status.value === 'streaming') {
        noFrames.value = true;
        markDisconnected();
      }
    });
  }
  (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
  // Cache the stream FIRST, then flip status. Do NOT call attachStream here:
  // it would run while <video> is still unmounted (or mounted under the OLD
  // v-if branch) and either no-op into pendingStream (losing the race against
  // the watcher) or set srcObject on an element about to be torn down by the
  // v-if transition — which causes play() to reject with AbortError and the
  // next <video> mount never receives the stream. The watcher below is the
  // single source of truth for attachment.
  pendingStream.value = stream;
  markStreaming();
}

function reconnect() {
  window.location.reload();
}

let disconnectedTimer: number | undefined;
function scheduleDisconnected() {
  if (disconnectedTimer !== undefined) return;
  disconnectedTimer = window.setTimeout(() => {
    disconnectedTimer = undefined;
    if (status.value !== 'streaming') markDisconnected();
  }, 5000);
}

let frameWatchdog: number | undefined;
let pcRef: RTCPeerConnection | null = null;
function onPcFailure() {
  const pc = pcRef;
  if (!pc) return;
  const s = pc.connectionState ?? pc.iceConnectionState ?? '';
  if (s === 'disconnected' || s === 'failed' || s === 'closed') {
    if (status.value === 'streaming') markDisconnected();
  }
}

watch(
  [videoEl, pendingStream, () => status.value === 'streaming'],
  ([el, stream, isStreaming]) => {
    if (!isStreaming) return;
    if (!el || !stream) return;
    attachStream(stream);
    pendingStream.value = null;
  },
  // flush: 'post' guarantees the watcher runs AFTER Vue patches the DOM,
  // so videoEl.value points at the freshly-mounted <video> (not the one
  // that is about to be unmounted by a v-if transition). This eliminates
  // the race where attachStream runs against an element that gets torn
  // down before play() resolves, which manifested as
  //   [player-view] play() rejected: AbortError
  // followed 5s later by
  //   [player-view] no frames after 5s; videoWidth=0
  { immediate: true, flush: 'post' },
);

onMounted(() => {
  status.value = 'connecting';
  window.addEventListener('viewer-stream', onStream);
  const ws = (window as unknown as { __smDebugWs?: WebSocket }).__smDebugWs;
  if (ws) {
    ws.addEventListener('close', scheduleDisconnected);
    ws.addEventListener('error', scheduleDisconnected);
  }
  const pc = (window as unknown as { __smPc?: RTCPeerConnection }).__smPc;
  if (pc) {
    pcRef = pc;
    pc.addEventListener('iceconnectionstatechange', onPcFailure);
    pc.addEventListener('connectionstatechange', onPcFailure);
  }
  emit('state', 2);
});

onBeforeUnmount(() => {
  window.removeEventListener('viewer-stream', onStream);
  if (videoEl.value) videoEl.value.srcObject = null;
  if (disconnectedTimer !== undefined) window.clearTimeout(disconnectedTimer);
  if (frameWatchdog !== undefined) window.clearTimeout(frameWatchdog);
  if (pcRef) {
    pcRef.removeEventListener('iceconnectionstatechange', onPcFailure);
    pcRef.removeEventListener('connectionstatechange', onPcFailure);
    pcRef = null;
  }
  void reset;
});
</script>

<style scoped>
.player-view {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  background: #000;
}

.frame {
  flex: 1;
  width: 100%;
  min-height: 0;
  object-fit: contain;
  background: black;
  cursor: pointer;
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

</style>
