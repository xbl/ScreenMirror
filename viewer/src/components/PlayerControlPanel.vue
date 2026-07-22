<template>
  <div class="control-panel">
    <button class="play" @click="$emit('play-pause')">
      {{ playing ? t('controls.pause') : t('controls.play') }}
    </button>
    <label>
      {{ t('controls.quality') }}:
      <select :value="quality" @change="onQuality">
        <option v-for="q in qualities" :key="q" :value="q">{{ q }}</option>
      </select>
    </label>
    <button @click="$emit('fullscreen')">{{ t('controls.fullscreen') }}</button>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';

const { t } = useI18n();
const qualities = ['100%', '80%', '60%', '40%', '25%'] as const;

defineProps<{ playing: boolean; quality: string }>();
const emit = defineEmits<{
  (e: 'play-pause'): void;
  (e: 'quality', v: string): void;
  (e: 'fullscreen'): void;
}>();

function onQuality(ev: Event) {
  emit('quality', (ev.target as HTMLSelectElement).value);
}
</script>

<style scoped>
.control-panel {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 8px 16px;
  background: #137cbd;
  color: white;
  border-radius: 8px;
}
.play {
  background: white;
  color: #137cbd;
  border: none;
  padding: 6px 16px;
  border-radius: 999px;
  cursor: pointer;
  font-weight: 600;
}
</style>