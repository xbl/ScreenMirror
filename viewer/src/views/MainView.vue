<template>
  <div class="main-view">
    <ConnectionPrompts v-if="!streaming" :step="promptStep" @reinitiate="reload" />
    <PlayerView v-if="streaming" :room-id="roomId" @state="onState" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, inject, isRef, type Ref, onMounted, onBeforeUnmount } from 'vue';
import ConnectionPrompts from './ConnectionPrompts.vue';
import PlayerView from './PlayerView.vue';
import { useViewerStream } from '../lib/viewerStream';

const props = defineProps<{ ws: WebSocket | null; connectSignal: number }>();
const roomId = ref('');
const promptStep = ref(1);
const streaming = ref(false);

const { activeStream } = useViewerStream();

function onState(step: number) {
  promptStep.value = step;
  if (step === 2) streaming.value = true;
}

function reload() {
  window.location.reload();
}

const injectedRoomId = inject<Ref<string>>('roomId');
if (injectedRoomId && isRef(injectedRoomId)) {
  watch(
    injectedRoomId,
    (val: string) => {
      if (val) roomId.value = val;
    },
    { immediate: true },
  );
} else {
  roomId.value = window.location.pathname.replace(/^\//, '').split('/')[0] || '';
}

function onViewerSignal(ev: Event) {
  const msg = (ev as CustomEvent).detail as { type: string };
  if (msg.type === 'ALLOWED_TO_CONNECT' || msg.type === 'ANSWER') {
    promptStep.value = 2;
    streaming.value = true;
  }
}

watch(
  () => props.connectSignal,
  (val) => {
    if (val > 0) {
      promptStep.value = 2;
      streaming.value = true;
    }
  },
);

watch(
  activeStream,
  (stream) => {
    if (stream) {
      promptStep.value = 2;
      streaming.value = true;
    }
  },
  { immediate: true },
);

onMounted(() => {
  window.addEventListener('viewer-signal', onViewerSignal);
});

onBeforeUnmount(() => {
  window.removeEventListener('viewer-signal', onViewerSignal);
});
</script>

<style scoped>
.main-view {
  min-height: 100vh;
  background: var(--bg);
}
</style>
