<template>
  <div class="player-view">
    <video
      v-if="status === 'streaming'"
      ref="videoEl"
      class="frame"
      :class="{ 'touch-control-active': touchControlEnabled }"
      autoplay
      playsinline
      muted
      disablepictureinpicture
      @loadedmetadata="onLoadedMetadata"
      @error="onVideoError"
      @pause="onVideoPause"
      @dblclick="onVideoDblClick"
    />

    <canvas
      v-if="status === 'streaming'"
      ref="canvasEl"
      v-show="isUsingWebCodecs"
      class="webcodecs-canvas"
      aria-hidden="true"
    />

    <div
      v-if="status === 'streaming'"
      class="player-streaming player-status"
      :data-state="status"
      role="status"
    >
      <span class="player-status-dot" aria-hidden="true" />
      {{ t('player.streaming') }}
    </div>

    <button
      v-if="status === 'streaming'"
      type="button"
      class="player-touch-toggle"
      :class="{ 'is-active': touchControlEnabled }"
      :title="touchControlEnabled ? t('remoteControl.enabled') : t('remoteControl.disabled')"
      @click="toggleTouchControl"
    >
      <span class="touch-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
          <path d="M9 11.24V7.5a2.5 2.5 0 0 1 5 0v3.74c1.21-.81 2-2.18 2-3.74a4.5 4.5 0 0 0-9 0c0 1.56.79 2.93 2 3.74zm9.84 4.63l-4.54-2.26A2 2 0 0 0 13.4 13.5H13v-6a1.5 1.5 0 0 0-3 0v10.74l-3.44-.72a1.5 1.5 0 0 0-1.54.49l-.78.89 4.93 4.93c.53.53 1.25.83 2 .83h6.24c1.47 0 2.71-1.07 2.95-2.53l.63-3.79a3 3 0 0 0-2.15-3.41z" />
        </svg>
      </span>
      <span class="touch-label">{{ touchControlEnabled ? t('remoteControl.enabled') : t('remoteControl.disabled') }}</span>
    </button>

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
import { useViewerStream } from '../lib/viewerStream';
import { isWebCodecsSupported, WebCodecsRenderer } from '../lib/webcodecsRenderer';
import { useRemoteControl } from '../lib/remoteControl';
import { useRemoteInput } from '../lib/useRemoteInput';

const { t } = useI18n();
const { status, markStreaming, markDisconnected, reset } = useViewerStatus();
const { activeStream, peerConnection, setActiveStream } = useViewerStream();
const { touchControlEnabled, toggleTouchControl } = useRemoteControl();

const videoEl = ref<HTMLVideoElement | null>(null);
const canvasEl = ref<HTMLCanvasElement | null>(null);
useRemoteInput({ videoEl, canvasEl });
const pendingStream = ref<MediaStream | null>(null);
const noFrames = ref(false);
const isUsingWebCodecs = ref(false);
const renderer = new WebCodecsRenderer();

if (typeof window !== 'undefined') {
  (window as unknown as { __smWebCodecsRenderer?: WebCodecsRenderer }).__smWebCodecsRenderer = renderer;
}

// VideoToolbox can take several seconds to initialize on a real screen capture
// before the first keyframe arrives. Keep the element mounted during that cold
// start; the watchdog is for a genuinely stalled stream, not encoder startup.
const FRAME_WATCHDOG_MS = 10_000;

const emit = defineEmits<{
  (e: 'state', step: number): void;
}>();

async function startWebCodecs(stream: MediaStream) {
  if (!isWebCodecsSupported()) {
    isUsingWebCodecs.value = false;
    return;
  }
  const canvas = canvasEl.value;
  if (!canvas) {
    isUsingWebCodecs.value = false;
    return;
  }
  const tracks = stream.getVideoTracks();
  if (tracks.length === 0) {
    isUsingWebCodecs.value = false;
    return;
  }
  const track = tracks[0];
  if (renderer.isRunning) {
    return;
  }
  try {
    renderer.onError = (err) => {
      console.warn('[player-view] WebCodecs runtime error, falling back to <video>:', err);
      isUsingWebCodecs.value = false;
    };
    renderer.onFrame = () => {
      if (frameWatchdog !== undefined) {
        window.clearTimeout(frameWatchdog);
        frameWatchdog = undefined;
      }
      noFrames.value = false;
    };
    await renderer.start(track, canvas);
    isUsingWebCodecs.value = true;
    console.log('[player-view] WebCodecs hardware direct rendering active');
  } catch (err) {
    console.warn('[player-view] WebCodecs start failed, falling back to <video>:', err);
    isUsingWebCodecs.value = false;
  }
}

function stopWebCodecs() {
  if (renderer.isRunning) {
    void renderer.stop();
  }
  isUsingWebCodecs.value = false;
}

function attachStream(stream: MediaStream) {
  const v = videoEl.value;
  if (!v) {
    pendingStream.value = stream;
    return;
  }
  if (v.srcObject !== stream) {
    v.srcObject = stream;
    const playPromise = v.play();
    if (playPromise && typeof playPromise.then === 'function') {
      playPromise.catch((err) => {
        console.error('[player-view] play() rejected:', err);
      });
    }
    startFrameWatchdog();
  }
  void startWebCodecs(stream);
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
      console.error(
        '[player-view] no frames after ' +
          FRAME_WATCHDOG_MS / 1000 +
          's; videoWidth=0 videoHeight=0 readyState=' +
          v.readyState +
          ' networkState=' +
          v.networkState +
          ' error=' +
          (v.error ? v.error.code : 'none'),
      );
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
  startLiveEdgeChaser();
  console.log(
    '[player-view] loadedmetadata videoWidth=' +
      (videoEl.value?.videoWidth ?? 0) +
      ' videoHeight=' +
      (videoEl.value?.videoHeight ?? 0),
  );
}

let liveEdgeInterval: number | undefined;

function startLiveEdgeChaser() {
  stopLiveEdgeChaser();
  liveEdgeInterval = window.setInterval(() => {
    const v = videoEl.value;
    if (!v || isUsingWebCodecs.value || v.paused || v.ended) return;
    try {
      if (v.buffered.length > 0) {
        const liveEdge = v.buffered.end(v.buffered.length - 1);
        const lag = liveEdge - v.currentTime;
        // Never perform abrupt seeking during normal playback.
        // Seeking on a live WebRTC stream resets the video decoder,
        // causing visible flickering, black flashes, and frame teleports.
        if (lag > 2.0) {
          // If lag is massive (> 2s, e.g. tab was inactive in background),
          // snap once directly to live edge.
          v.currentTime = Math.max(0, liveEdge - 0.05);
        } else if (lag > 0.35) {
          // Gently accelerate to catch up smoothly without seeking or decoding restarts
          if (v.playbackRate !== 1.05) v.playbackRate = 1.05;
        } else if (lag < 0.15 && v.playbackRate !== 1.0) {
          v.playbackRate = 1.0;
        }
      }
    } catch {
      // ignore
    }
  }, 250);
}

function stopLiveEdgeChaser() {
  if (liveEdgeInterval !== undefined) {
    window.clearInterval(liveEdgeInterval);
    liveEdgeInterval = undefined;
  }
  const v = videoEl.value;
  if (v && v.playbackRate !== 1.0) {
    try {
      v.playbackRate = 1.0;
    } catch {
      // ignore
    }
  }
}

function onVideoError(event: Event) {
  const v = videoEl.value;
  console.error('[player-view] <video> error:', event, v?.error);
  noFrames.value = true;
  markDisconnected();
}

function onVideoPause() {
  const v = videoEl.value;
  if (v && status.value === 'streaming' && !v.ended) {
    void v.play().catch(() => {});
  }
}

function toggleFullscreen() {
  if (typeof document === 'undefined') return;
  if (!document.fullscreenElement) {
    void document.documentElement.requestFullscreen?.().catch(() => {});
  } else {
    void document.exitFullscreen?.().catch(() => {});
  }
}

function onVideoDblClick() {
  if (!touchControlEnabled.value) {
    toggleFullscreen();
  }
}

// Screen Wake Lock API: keeps mobile/desktop screen awake during active streaming
let wakeLockSentinel: WakeLockSentinel | null = null;

async function requestWakeLock() {
  if (typeof navigator !== 'undefined' && 'wakeLock' in navigator) {
    try {
      wakeLockSentinel = await navigator.wakeLock.request('screen');
      wakeLockSentinel.addEventListener('release', () => {
        wakeLockSentinel = null;
      });
      console.log('[player-view] screen wake lock acquired');
    } catch {
      // Wake lock can fail if low battery or tab hidden; ignore safely
    }
  }
}

async function releaseWakeLock() {
  if (wakeLockSentinel) {
    try {
      await wakeLockSentinel.release();
    } catch {
      // ignore
    }
    wakeLockSentinel = null;
  }
}

function onVisibilityChange() {
  if (typeof document !== 'undefined' && document.visibilityState === 'visible' && status.value === 'streaming') {
    void requestWakeLock();
  }
}

function isMediaStream(val: unknown): val is MediaStream {
  if (typeof MediaStream !== 'undefined') {
    return val instanceof MediaStream;
  }
  return val !== null && typeof val === 'object';
}

function handleStream(stream: MediaStream) {
  if (!isMediaStream(stream)) return;
  if (typeof stream.getTracks === 'function') {
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

function onStream(event: Event) {
  const stream = (event as CustomEvent<MediaStream>).detail;
  if (isMediaStream(stream)) {
    if (activeStream.value !== stream) {
      setActiveStream(stream);
    } else if (stream !== videoEl.value?.srcObject && stream !== pendingStream.value) {
      handleStream(stream);
    }
  }
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

// Watch activeStream from the reactive stream layer
watch(activeStream, (newStream) => {
  if (newStream && newStream !== videoEl.value?.srcObject && newStream !== pendingStream.value) {
    stopWebCodecs();
    handleStream(newStream);
  }
});

// Watch status leaving streaming to stop WebCodecs rendering
watch(
  () => status.value === 'streaming',
  (isStreaming) => {
    if (!isStreaming) {
      stopWebCodecs();
      void releaseWakeLock();
    } else {
      void requestWakeLock();
    }
  },
);

// Watch peerConnection changes
watch(
  peerConnection,
  (newPc, oldPc) => {
    if (oldPc) {
      oldPc.removeEventListener('iceconnectionstatechange', onPcFailure);
      oldPc.removeEventListener('connectionstatechange', onPcFailure);
    }
    if (newPc) {
      pcRef = newPc;
      newPc.addEventListener('iceconnectionstatechange', onPcFailure);
      newPc.addEventListener('connectionstatechange', onPcFailure);
    }
  },
  { immediate: true },
);

onMounted(() => {
  status.value = 'connecting';
  window.addEventListener('viewer-stream', onStream);
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', onVisibilityChange);
  }
  startLiveEdgeChaser();

  // If activeStream was already received before PlayerView mounted, immediately handle it
  if (activeStream.value) {
    handleStream(activeStream.value);
  }

  const ws = (window as unknown as { __smDebugWs?: WebSocket }).__smDebugWs;
  if (ws) {
    ws.addEventListener('close', scheduleDisconnected);
    ws.addEventListener('error', scheduleDisconnected);
  }
  if (!pcRef) {
    const pc = (window as unknown as { __smPc?: RTCPeerConnection }).__smPc;
    if (pc) {
      pcRef = pc;
      pc.addEventListener('iceconnectionstatechange', onPcFailure);
      pc.addEventListener('connectionstatechange', onPcFailure);
    }
  }
  emit('state', 2);
});

onBeforeUnmount(() => {
  stopLiveEdgeChaser();
  stopWebCodecs();
  void releaseWakeLock();
  if (typeof document !== 'undefined') {
    document.removeEventListener('visibilitychange', onVisibilityChange);
  }
  window.removeEventListener('viewer-stream', onStream);
  if (videoEl.value) videoEl.value.srcObject = null;
  if (disconnectedTimer !== undefined) window.clearTimeout(disconnectedTimer);
  if (frameWatchdog !== undefined) window.clearTimeout(frameWatchdog);
  if (pcRef) {
    pcRef.removeEventListener('iceconnectionstatechange', onPcFailure);
    pcRef.removeEventListener('connectionstatechange', onPcFailure);
    pcRef = null;
  }
  const ws = (window as unknown as { __smDebugWs?: WebSocket }).__smDebugWs;
  if (ws) {
    ws.removeEventListener('close', scheduleDisconnected);
    ws.removeEventListener('error', scheduleDisconnected);
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
  background: var(--canvas);
}

.frame {
  flex: 1;
  width: 100%;
  min-height: 0;
  object-fit: contain;
  background: var(--canvas);
  cursor: pointer;
}

.frame.touch-control-active {
  touch-action: none;
  cursor: crosshair;
}

.player-touch-toggle {
  position: absolute;
  top: 14px;
  right: 14px;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 6px 12px;
  color: var(--muted);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 78%, transparent);
  backdrop-filter: blur(16px);
  font-size: var(--fs-12);
  font-weight: 500;
  cursor: pointer;
  transition: all var(--motion-fast) ease-out;
  user-select: none;
  z-index: 10;
}

.player-touch-toggle:hover {
  background: var(--surface);
  color: var(--text);
}

.player-touch-toggle.is-active {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
}

.player-touch-toggle.is-active:hover {
  background: color-mix(in srgb, var(--accent) 20%, var(--surface));
}

.webcodecs-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
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
  letter-spacing: 0;
  color: var(--text);
}

.spinner {
  color: var(--accent);
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
  color: #fff;
  border: none;
  min-height: 32px;
  padding: 6px 14px;
  border-radius: var(--radius-md);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background var(--motion-fast) ease-out;
}

.player-reconnect:hover {
  background: var(--accent-strong);
}

.player-streaming {
  position: absolute;
  top: 14px;
  left: 14px;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 6px 9px;
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 78%, transparent);
  backdrop-filter: blur(16px);
  font-size: var(--fs-12);
  opacity: 0.86;
  pointer-events: none;
}

.player-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--success);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 18%, transparent);
}

@media (prefers-reduced-motion: reduce) {
  .player-streaming {
    transition: none;
  }
}
</style>
