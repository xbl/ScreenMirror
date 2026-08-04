<template>
  <div class="connection-prompts">
    <div class="signal-mark" aria-hidden="true">
      <span />
      <span />
      <span />
    </div>
    <p class="eyebrow">ScreenMirror</p>
    <h1>{{ text }}</h1>
    <button v-if="props.step !== 1" type="button" @click="$emit('reinitiate')">
      {{ t('viewer.reinitiate') }}
    </button>
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

const text = computed(() => prompts[props.step] ?? t('viewer.disconnected'));
</script>

<style scoped>
.connection-prompts {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  min-height: 100vh;
  box-sizing: border-box;
  padding: 24px;
  color: var(--text);
  background: var(--bg);
  text-align: center;
}
.signal-mark {
  display: flex;
  align-items: end;
  gap: 4px;
  height: 28px;
  margin-bottom: 10px;
}
.signal-mark span {
  width: 4px;
  border-radius: 3px;
  background: var(--accent);
  animation: breathe 1.6s ease-in-out infinite;
}
.signal-mark span:nth-child(1) { height: 10px; animation-delay: -0.35s; }
.signal-mark span:nth-child(2) { height: 18px; animation-delay: -0.15s; }
.signal-mark span:nth-child(3) { height: 28px; }
.eyebrow {
  margin: 0;
  color: var(--accent);
  font: 650 var(--fs-12)/1.2 var(--font-body);
  letter-spacing: .12em;
}
h1 {
  max-width: 360px;
  margin: 0;
  color: var(--text-strong);
  font-family: var(--font-display);
  font-size: 22px;
  font-weight: 650;
  letter-spacing: 0;
  line-height: 1.2;
}
button {
  margin-top: 16px;
  min-height: 32px;
  padding: 6px 13px;
  border: 1px solid var(--accent-line);
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--accent);
  font: 500 var(--fs-14)/1.2 var(--font-body);
  cursor: pointer;
  transition: background var(--motion-fast) ease-out, color var(--motion-fast) ease-out;
}
button:hover {
  background: var(--accent);
  color: #fff;
}
@keyframes breathe {
  0%, 100% { opacity: 0.35; transform: scaleY(0.72); }
  50% { opacity: 1; transform: scaleY(1); }
}
@media (prefers-reduced-motion: reduce) {
  .signal-mark span { animation: none; }
}
</style>
