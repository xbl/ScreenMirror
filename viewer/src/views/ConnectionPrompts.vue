<template>
  <div class="connection-prompts">
    <h2>{{ text }}</h2>
    <button @click="$emit('reinitiate')">{{ t('viewer.reinitiate') }}</button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

const props = defineProps<{ step: number }>();
defineEmits<{ (e: 'reinitiate'): void }>();

const { t } = useI18n();

const prompts: Record<number, string> = {
  1: t('viewer.waitingForAllow'),
  2: t('viewer.connected'),
  3: t('viewer.waitingForSource'),
};

const text = computed(() => prompts[props.step] ?? t('viewer.unknown'));
</script>

<style scoped>
.connection-prompts {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 24px;
  background: rgba(240, 248, 250, 1);
  min-height: 100vh;
}
button {
  background: #ffb84d;
  color: white;
  border: none;
  padding: 8px 16px;
  border-radius: 999px;
  cursor: pointer;
}
</style>