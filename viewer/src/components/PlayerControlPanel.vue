<template>
  <div class="control-panel">
    <button class="play" @click="$emit('play-pause')">
      {{ playing ? t('controls.pause') : t('controls.play') }}
    </button>
    <button @click="$emit('fullscreen')">{{ t('controls.fullscreen') }}</button>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';

const { t } = useI18n();
defineProps<{ playing: boolean }>();
defineEmits<{
  (e: 'play-pause'): void;
  (e: 'fullscreen'): void;
}>();
</script>

<style scoped>
.control-panel {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 7px 10px;
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  color: var(--text);
  border-radius: var(--radius-md);
  backdrop-filter: blur(16px);
}
.play {
  background: var(--accent);
  color: #fff;
  border: 0;
  min-height: 28px;
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-weight: 600;
}
</style>
