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
  background: rgba(14, 17, 22, 0.78);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}
.error-dialog {
  position: relative;
  width: min(420px, calc(100vw - 32px));
  box-sizing: border-box;
  padding: var(--sp-6, 24px);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.28);
}
.error-dialog h2 {
  margin: 0 0 var(--sp-2, 8px);
  color: var(--text-strong);
  font-family: var(--font-display);
  font-size: var(--fs-22, 1.375rem);
  font-weight: 500;
}
.error-dialog p {
  margin: 0;
  color: var(--muted);
  font-size: var(--fs-14, 0.875rem);
}
.error-dialog button {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  color: var(--muted);
  font-size: 22px;
  line-height: 1;
  transition: color var(--motion) ease, background var(--motion) ease;
}
.error-dialog button:hover {
  color: var(--text);
  background: var(--surface-2);
}
</style>
