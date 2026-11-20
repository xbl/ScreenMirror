<template>
  <div class="player-view">
    <video
      v-if="status === 'streaming'"
      ref="videoEl"
      class="frame"
      :class="{ 'touch-control-active': touchControlEnabled }"
      :style="videoTransformStyle"
      autoplay
      playsinline
      :muted="isAudioMuted"
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
      :style="videoTransformStyle"
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

    <div v-if="status === 'streaming'" class="player-top-controls">
      <button
        type="button"
        class="player-btn player-audio-toggle"
        :class="{ 'is-active': !isAudioMuted }"
        :title="isAudioMuted ? t('player.unmute') : t('player.mute')"
        :aria-label="isAudioMuted ? t('player.unmute') : t('player.mute')"
        @click="toggleAudio"
      >
        <span class="btn-icon audio-icon" aria-hidden="true">
          <svg v-if="!isAudioMuted" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/>
          </svg>
          <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M16.5 12c0-1.77-1.02-3.29-2.5-4.03v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51C20.63 14.91 21 13.5 21 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3L3 4.27l4.73 4.73H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06c1.38-.31 2.63-.95 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4L9.91 6.09 12 8.18V4z"/>
          </svg>
        </span>
        <span class="btn-label audio-label">{{ isAudioMuted ? t('player.unmute') : t('player.mute') }}</span>
      </button>

      <button
        type="button"
        class="player-btn player-touch-toggle"
        :class="{ 'is-active': touchControlEnabled }"
        :title="touchControlEnabled ? t('remoteControl.enabled') : t('remoteControl.disabled')"
        :aria-label="touchControlEnabled ? t('remoteControl.enabled') : t('remoteControl.disabled')"
        @click="toggleTouchControl"
      >
        <span class="btn-icon touch-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M9 11.24V7.5a2.5 2.5 0 0 1 5 0v3.74c1.21-.81 2-2.18 2-3.74a4.5 4.5 0 0 0-9 0c0 1.56.79 2.93 2 3.74zm9.84 4.63l-4.54-2.26A2 2 0 0 0 13.4 13.5H13v-6a1.5 1.5 0 0 0-3 0v10.74l-3.44-.72a1.5 1.5 0 0 0-1.54.49l-.78.89 4.93 4.93c.53.53 1.25.83 2 .83h6.24c1.47 0 2.71-1.07 2.95-2.53l.63-3.79a3 3 0 0 0-2.15-3.41z" />
          </svg>
        </span>
        <span class="btn-label touch-label">{{ touchControlEnabled ? t('remoteControl.enabled') : t('remoteControl.disabled') }}</span>
      </button>

      <button
        type="button"
        class="player-btn player-keyboard-toggle"
        :class="{ 'is-active': isKeyboardActive }"
        :title="isKeyboardActive ? t('remoteControl.keyboardClose') : t('remoteControl.keyboardOpen')"
        :aria-label="isKeyboardActive ? t('remoteControl.keyboardClose') : t('remoteControl.keyboardOpen')"
        @click="toggleKeyboard"
      >
        <span class="btn-icon keyboard-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M20 5H4c-1.1 0-1.99.9-1.99 2L2 17c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm-9 3h2v2h-2V8zm0 3h2v2h-2v-2zM8 8h2v2H8V8zm0 3h2v2H8v-2zm-1 2H5v-2h2v2zm0-3H5V8h2v2zm9 7H8v-2h8v2zm0-4h-2v-2h2v2zm0-3h-2V8h2v2zm3 3h-2v-2h2v2zm0-3h-2V8h2v2z" />
          </svg>
        </span>
        <span class="btn-label keyboard-label">{{ isKeyboardActive ? t('remoteControl.keyboardClose') : t('remoteControl.keyboardOpen') }}</span>
      </button>

      <button
        type="button"
        class="player-btn player-fullscreen-toggle"
        :class="{ 'is-active': isFullscreen }"
        :title="isFullscreen ? t('player.exitFullscreen') : t('player.fullscreen')"
        :aria-label="isFullscreen ? t('player.exitFullscreen') : t('player.fullscreen')"
        @click="toggleFullscreen"
      >
        <span class="btn-icon fullscreen-icon" aria-hidden="true">
          <svg v-if="!isFullscreen" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z"/>
          </svg>
          <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-11V5h-2v5h5V8h-3z"/>
          </svg>
        </span>
        <span class="btn-label fullscreen-label">{{ isFullscreen ? t('player.exitFullscreen') : t('player.fullscreen') }}</span>
      </button>
    </div>

    <button
      v-if="status === 'streaming' && zoomScale > 1.05"
      type="button"
      class="player-zoom-badge"
      :title="t('remoteControl.resetZoom')"
      @click="resetZoom"
    >
      <span class="zoom-text">{{ zoomScale.toFixed(1) }}x</span>
      <span class="zoom-reset-label">{{ t('remoteControl.resetZoom') }}</span>
    </button>

    <input
      ref="keyboardInputEl"
      type="text"
      class="player-keyboard-input"
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      inputmode="text"
      enterkeyhint="go"
      tabindex="-1"
      aria-hidden="true"
      @input="onKeyboardInput"
      @keydown="onKeyboardKeyDown"
      @focus="isKeyboardActive = true"
      @blur="isKeyboardActive = false"
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
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
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
const {
  zoomScale,
  panX,
  panY,
  resetZoom,
  sendRemoteInput,
} = useRemoteInput({ videoEl, canvasEl });

const videoTransformStyle = computed(() => {
  if (zoomScale.value <= 1.0 && panX.value === 0 && panY.value === 0) {
    return {};
  }
  return {
    transform: `translate(${panX.value}px, ${panY.value}px) scale(${zoomScale.value})`,
    transformOrigin: '0 0',
  };
});

const keyboardInputEl = ref<HTMLInputElement | null>(null);
const isKeyboardActive = ref(false);

function toggleKeyboard() {
  const el = keyboardInputEl.value;
  if (!el) return;
  if (isKeyboardActive.value) {
    el.blur();
  } else {
    el.focus();
  }
}

function onKeyboardInput(e: Event) {
  const input = e.target as HTMLInputElement;
  const val = input.value;
  if (val && val.length > 0) {
    sendRemoteInput({
      action: 'text_input',
      text: val,
    });
    input.value = '';
  }
}

function onKeyboardKeyDown(e: KeyboardEvent) {
  if (e.key === 'Backspace') {
    e.preventDefault();
    sendRemoteInput({
      action: 'key_click',
      key: 'Backspace',
    });
  } else if (e.key === 'Enter') {
    e.preventDefault();
    sendRemoteInput({
      action: 'key_click',
      key: 'Enter',
    });
  } else if (e.key === 'Tab') {
    e.preventDefault();
    sendRemoteInput({
      action: 'key_click',
      key: 'Tab',
    });
  } else if (e.key === 'Escape') {
    e.preventDefault();
    sendRemoteInput({
      action: 'key_click',
      key: 'Escape',
    });
  }
}
const pendingStream = ref<MediaStream | null>(null);
const noFrames = ref(false);
const isUsingWebCodecs = ref(false);
const isAudioMuted = ref(true);
const renderer = new WebCodecsRenderer();

function toggleAudio() {
  isAudioMuted.value = !isAudioMuted.value;
  const v = videoEl.value;
  if (v) {
    v.muted = isAudioMuted.value;
    if (!isAudioMuted.value) {
      v.volume = 1.0;
      if (v.paused) {
        void v.play().catch((err) => {
          console.warn('[player-view] play after unmute failed:', err);
        });
      }
    }
  }
}

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
    v.muted = isAudioMuted.value;
    v.defaultMuted = isAudioMuted.value;
    const playPromise = v.play();
    if (playPromise && typeof playPromise.then === 'function') {
      playPromise.catch((err) => {
        console.error('[player-view] play() rejected:', err);
        // On iOS Safari, force muted play to bypass strict autoplay policies
        if (!v.muted) {
          v.muted = true;
          isAudioMuted.value = true;
          void v.play().catch(() => {});
        }
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

const isFullscreen = ref(false);

function updateFullscreenState() {
  if (typeof document === 'undefined') return;
  isFullscreen.value = Boolean(
    document.fullscreenElement || (document as any).webkitFullscreenElement,
  );
}

const preventGesture = (e: Event) => e.preventDefault();

async function toggleFullscreen() {
  if (typeof document === 'undefined') return;
  try {
    if (!document.fullscreenElement && !(document as any).webkitFullscreenElement) {
      const docEl = document.documentElement as HTMLElement & {
        webkitRequestFullscreen?: () => Promise<void>;
      };
      if (docEl.requestFullscreen) {
        await docEl.requestFullscreen();
      } else if (docEl.webkitRequestFullscreen) {
        await docEl.webkitRequestFullscreen();
      } else if ((videoEl.value as any)?.webkitEnterFullscreen) {
        (videoEl.value as any).webkitEnterFullscreen();
      }
      // If mobile device, attempt to lock landscape orientation
      if (typeof screen !== 'undefined' && screen.orientation && 'lock' in screen.orientation) {
        try {
          await (screen.orientation as any).lock('landscape');
        } catch {
          // ignore orientation lock error
        }
      }
    } else {
      const doc = document as Document & {
        webkitExitFullscreen?: () => Promise<void>;
      };
      if (doc.exitFullscreen) {
        await doc.exitFullscreen();
      } else if (doc.webkitExitFullscreen) {
        await doc.webkitExitFullscreen();
      }
      if (typeof screen !== 'undefined' && screen.orientation && 'unlock' in screen.orientation) {
        try {
          (screen.orientation as any).unlock();
        } catch {}
      }
    }
  } catch (err) {
    console.warn('[player-view] toggleFullscreen error:', err);
  } finally {
    updateFullscreenState();
  }
}

function onVideoDblClick() {
  if (!touchControlEnabled.value) {
    void toggleFullscreen();
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
    document.addEventListener('fullscreenchange', updateFullscreenState);
    document.addEventListener('webkitfullscreenchange', updateFullscreenState);
    document.addEventListener('gesturestart', preventGesture);
    document.addEventListener('gesturechange', preventGesture);
    document.addEventListener('gestureend', preventGesture);
    updateFullscreenState();
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
    document.removeEventListener('fullscreenchange', updateFullscreenState);
    document.removeEventListener('webkitfullscreenchange', updateFullscreenState);
    document.removeEventListener('gesturestart', preventGesture);
    document.removeEventListener('gesturechange', preventGesture);
    document.removeEventListener('gestureend', preventGesture);
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

.player-top-controls {
  position: absolute;
  top: 14px;
  right: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
  z-index: 10;
}

.player-btn {
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
}

.player-btn:hover {
  background: var(--surface);
  color: var(--text);
}

.player-btn.is-active {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
}

.player-btn.is-active:hover {
  background: color-mix(in srgb, var(--accent) 20%, var(--surface));
}

.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
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

@media (max-width: 600px) {
  .player-top-controls {
    top: 10px;
    right: 10px;
    gap: 6px;
  }
  .player-streaming {
    top: 10px;
    left: 10px;
    padding: 5px 8px;
    font-size: 11px;
  }
  .player-btn {
    padding: 5px 9px;
    font-size: 11px;
    gap: 5px;
  }
}

@media (max-width: 440px) {
  .player-btn .btn-label {
    display: none;
  }
  .player-btn {
    padding: 7px;
    border-radius: var(--radius-sm);
  }
}

.player-keyboard-input {
  position: absolute;
  top: 0;
  left: 0;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
  border: none;
  padding: 0;
  margin: 0;
}

.player-zoom-badge {
  position: absolute;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 14px;
  color: var(--text);
  border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--surface) 85%, transparent);
  backdrop-filter: blur(16px);
  font-size: var(--fs-12);
  font-weight: 500;
  cursor: pointer;
  z-index: 10;
  transition: all var(--motion-fast) ease-out;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.player-zoom-badge:hover {
  background: var(--surface);
  border-color: var(--accent);
}

.zoom-text {
  color: var(--accent);
  font-weight: 600;
}
</style>
