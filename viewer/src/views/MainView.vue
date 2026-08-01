<template>
  <div class="main-view">
    <ConnectionPrompts v-if="!streaming" :step="promptStep" @reinitiate="reload" />
    <PlayerView v-if="streaming" :room-id="roomId" @state="onState" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import ConnectionPrompts from './ConnectionPrompts.vue';
import PlayerView from './PlayerView.vue';

const props = defineProps<{ ws: WebSocket | null; connectSignal: number }>();
void props;
const roomId = ref('');
const promptStep = ref(1);
const streaming = ref(false);

function onState(step: number) {
  promptStep.value = step;
  if (step === 2) streaming.value = true;
}

function reload() {
  window.location.reload();
}

roomId.value = window.location.pathname.replace(/^\//, '').split('/')[0] || '';

window.addEventListener('viewer-signal', (ev) => {
  const msg = (ev as CustomEvent).detail as { type: string };
  if (msg.type === 'ALLOWED_TO_CONNECT' || msg.type === 'ANSWER') {
    promptStep.value = 2;
    streaming.value = true;
  }
});
</script>

<style scoped>
.main-view {
  min-height: 100vh;
  background: var(--bg);
}
</style>
