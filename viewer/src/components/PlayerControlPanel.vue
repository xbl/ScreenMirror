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
