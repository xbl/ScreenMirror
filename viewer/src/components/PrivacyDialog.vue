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
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}
.card {
  background: white;
  padding: 24px;
  border-radius: 8px;
  max-width: 520px;
}
button {
  background: #137cbd;
  color: white;
  border: none;
  padding: 8px 24px;
  border-radius: 999px;
  cursor: pointer;
  margin-top: 12px;
}
</style>
