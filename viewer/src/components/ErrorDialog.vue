<template>
  <div v-if="open" class="error-dialog-backdrop">
    <div class="error-dialog">
      <h2>{{ t('viewer.unknown') }}</h2>
      <p>{{ message }}</p>
      <button @click="$emit('close')">×</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
defineProps<{ open: boolean; message: string }>();
defineEmits<{ (e: 'close'): void }>();
const { t } = useI18n();
</script>

<style scoped>
.error-dialog-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}
.error-dialog {
  background: white;
  padding: 24px;
  border-radius: 8px;
  max-width: 600px;
}
</style>