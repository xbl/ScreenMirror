<template>
  <Teleport to="body">
    <div v-if="open" class="pd-backdrop" @click.self="$emit('close')">
      <div class="pd-card" role="dialog" :aria-label="t('privacy.title')">
        <header class="pd-head">
          <span class="pd-eyebrow">{{ t('privacy.title') }}</span>
          <button class="pd-close" @click="$emit('close')" aria-label="Close">×</button>
        </header>
        <p class="pd-intro">{{ t('privacy.intro') }}</p>
        <ul class="pd-list">
          <li>{{ t('privacy.bullet1') }}</li>
          <li>{{ t('privacy.bullet2') }}</li>
          <li>{{ t('privacy.bullet3') }}</li>
        </ul>
        <footer class="pd-foot">
          <button class="btn btn-accent" @click="$emit('close')">
            {{ t('privacy.close') }}
          </button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{ open: boolean }>();
defineEmits<{ (e: 'close'): void }>();

const { t } = useI18n();
</script>

<style scoped>
.pd-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(8, 10, 14, 0.7);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: var(--sp-6);
}

.pd-card {
  width: 100%;
  max-width: 460px;
  background: var(--surface);
  border: var(--line);
  border-radius: var(--radius-lg);
  padding: var(--sp-6);
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.pd-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.pd-eyebrow {
  font-size: var(--fs-12);
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--accent);
}

.pd-close {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-md);
  color: var(--muted);
  font-size: var(--fs-18);
  line-height: 1;
}

.pd-close:hover {
  color: var(--text);
  background: var(--surface-2);
}

.pd-intro {
  font-family: var(--font-display);
  font-size: var(--fs-22);
  line-height: 1.3;
  letter-spacing: -0.01em;
  color: var(--text-strong);
}

.pd-list {
  margin: 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  color: var(--muted);
  font-size: var(--fs-14);
}

.pd-list li::marker {
  color: var(--accent-line);
}

.pd-foot {
  display: flex;
  justify-content: flex-end;
  margin-top: var(--sp-2);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 10px 18px;
  border-radius: var(--radius-pill);
  font-size: var(--fs-14);
  font-weight: 500;
  border: 1px solid transparent;
  transition:
    background var(--motion) ease,
    color var(--motion) ease;
}

.btn-accent {
  background: var(--accent);
  color: #0a1413;
}
.btn-accent:hover {
  background: var(--accent-strong);
}
</style>