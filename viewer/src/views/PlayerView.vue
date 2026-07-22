<template>
  <div class="player-view">
    <PlayerControlPanel
      :playing="playing"
      :quality="quality"
      @play-pause="togglePlay"
      @quality="setQuality"
      @fullscreen="toggleFullscreen"
    />
    <video ref="videoEl" class="frame" autoplay playsinline muted :class="{ hidden: !playing }" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import PlayerControlPanel from '../components/PlayerControlPanel.vue';

const playing = ref(true);
const quality = ref('100%');
const videoEl = ref<HTMLVideoElement | null>(null);

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
}

onMounted(() => {
  window.addEventListener('viewer-stream', onStream);
  emit('state', 2);
});

onBeforeUnmount(() => {
  window.removeEventListener('viewer-stream', onStream);
  if (videoEl.value) videoEl.value.srcObject = null;
});
</script>

<style scoped>
.player-view {
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
.frame.hidden {
  opacity: 0;
}
</style>
