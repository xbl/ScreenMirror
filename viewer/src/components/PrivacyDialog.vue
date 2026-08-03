<template>
  <div v-if="open" class="privacy-dialog">
    <div class="card">
      <h2>{{ t('privacy.title') }}</h2>
      <p>{{ t('privacy.intro') }}</p>
      <ul>
        <li>{{ t('privacy.bullet1') }}</li>
        <li>{{ t('privacy.bullet2') }}</li>
        <li>{{ t('privacy.bullet3') }}</li>
      </ul>
      <button @click="$emit('close')">{{ t('privacy.close') }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{ open: boolean }>();
defineEmits<{ (e: 'close'): void }>();
const { t } = useI18n();
</script>

<style scoped>
.privacy-dialog {
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, var(--canvas) 64%, transparent);
  backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}
.card {
  background: var(--group);
  padding: 24px;
  border: var(--line);
  border-radius: var(--radius-lg);
  max-width: 520px;
}
button {
  background: var(--accent);
  color: white;
  border: none;
  min-height: 32px;
  padding: 6px 14px;
  border-radius: var(--radius-md);
  cursor: pointer;
  margin-top: 12px;
}
</style>
