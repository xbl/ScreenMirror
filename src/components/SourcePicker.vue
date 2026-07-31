<template>
  <section class="source-picker">
    <header class="sp-head">
      <span class="sp-eyebrow">{{ t('source.label') }}</span>
    </header>
    <div class="sp-options">
      <label
        class="sp-option"
        :class="{ active: source === 'screen' }"
      >
        <input
          v-model="source"
          type="radio"
          name="source"
          value="screen"
          @change="onChange"
        />
        <span class="sp-radio" aria-hidden="true" />
        <span class="sp-text">{{ t('source.screen') }}</span>
      </label>
      <label
        class="sp-option"
        :class="{ active: source === 'window' }"
      >
        <input
          v-model="source"
          type="radio"
          name="source"
          value="window"
          @change="onChange"
        />
        <span class="sp-radio" aria-hidden="true" />
        <span class="sp-text">{{ t('source.window') }}</span>
      </label>
    </div>
    <label class="sp-quality">
      <span class="sp-quality-label">{{ t('source.quality') }}</span>
      <select v-model="quality" @change="onChange">
        <option value="balanced">{{ t('source.qualityBalanced') }}</option>
        <option value="high">{{ t('source.qualityHigh') }}</option>
        <option value="ultra">{{ t('source.qualityUltra') }}</option>
      </select>
    </label>
  </section>
</template>

<script setup lang="ts">
import { ref, inject, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../utils/api';
import { PermissionModalKey, type ProvidedPermissionModal } from './PermissionModalHost';

const { t } = useI18n();
const source = ref<'screen' | 'window'>('screen');
const quality = ref<'balanced' | 'high' | 'ultra'>('high');

const qualityValue = () => ({ balanced: 0.5, high: 0.75, ultra: 1.0 }[quality.value]);

const permissionModal = inject<ProvidedPermissionModal>(
  PermissionModalKey,
  // Safe fallback when the host doesn't provide a modal (e.g. tests, storybook).
  ref(null) as ProvidedPermissionModal,
);

async function onChange() {
  // Gate: xcap enumeration will silently return incomplete data when
  // Screen Recording permission is denied. Surface the modal instead.
  try {
    const ok = await api.checkScreenRecordingPermission();
    if (!ok) {
      await permissionModal.value?.checkAndShow();
      return;
    }
  } catch {
    /* outside Tauri: tolerate and continue */
  }
  try {
    const sources = await api.enumerateCaptureSources();
    const first = sources.find((s) => s.kind === source.value);
    if (!first) return;
    const idx = parseInt(first.id.split(':')[1] ?? '0', 10);
    await api.setCaptureTarget({ kind: source.value, id: idx, quality: qualityValue() });
  } catch {
    /* tolerate: when running outside Tauri, just remember the choice */
  }
}

onMounted(() => {
  void onChange();
});
</script>

<style scoped>
.source-picker {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  padding: var(--sp-5);
  background: var(--surface);
  border: var(--line);
  border-radius: var(--radius-lg);
}

.sp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.sp-eyebrow {
  font-size: var(--fs-12);
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: var(--muted);
}

.sp-options {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--sp-3);
}

.sp-option {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 14px 18px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  cursor: pointer;
  background: transparent;
  color: var(--text);
  font-size: var(--fs-14);
  transition:
    border-color var(--motion) ease,
    background var(--motion) ease;
}

.sp-option:hover {
  border-color: var(--border-strong);
}

.sp-option.active {
  border-color: var(--accent-line);
  background: var(--accent-dim);
}

.sp-option input[type='radio'] {
  position: absolute;
  opacity: 0;
  pointer-events: none;
  width: 0;
  height: 0;
}

.sp-radio {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 1.5px solid var(--muted);
  position: relative;
  flex-shrink: 0;
  transition: border-color var(--motion) ease;
}

.sp-option.active .sp-radio {
  border-color: var(--accent);
}

.sp-radio::after {
  content: '';
  position: absolute;
  inset: 3px;
  border-radius: 50%;
  background: var(--accent);
  transform: scale(0);
  transition: transform var(--motion) ease;
}

.sp-option.active .sp-radio::after {
  transform: scale(1);
}

.sp-text {
  font-weight: 500;
}

.sp-quality {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  color: var(--muted);
  font-size: var(--fs-12);
}

.sp-quality select {
  min-width: 150px;
  padding: 8px 10px;
  color: var(--text);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}
</style>
