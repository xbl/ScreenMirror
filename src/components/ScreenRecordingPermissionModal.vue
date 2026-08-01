<template>
  <Teleport to="body">
    <div v-if="show" class="pm-backdrop" @keydown.esc="show = false">
      <div class="pm-card" role="alertdialog" :aria-label="t('permission.title')">
        <span class="pm-eyebrow">{{ t('permission.title') }}</span>
        <p class="pm-message">{{ t('permission.body') }}</p>
        <ol class="pm-steps">
          <li>{{ t('permission.step1') }}</li>
          <li>{{ t('permission.step2') }}</li>
          <li>{{ t('permission.step3') }}</li>
        </ol>
        <div class="pm-actions">
          <button
            class="btn btn-ghost"
            type="button"
            :disabled="opening"
            @click="openSettings"
          >
            {{ t('permission.openSettings') }}
          </button>
          <button
            class="btn btn-accent"
            type="button"
            :disabled="rechecking"
            @click="recheck"
          >
            {{ t('permission.recheck') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../utils/api';

const { t } = useI18n();
const show = ref(false);
const opening = ref(false);
const rechecking = ref(false);

async function checkAndShow(): Promise<void> {
  try {
    const ok = await api.checkScreenRecordingPermission();
    show.value = !ok;
  } catch {
    // running outside Tauri; leave hidden
  }
}

async function openSettings(): Promise<void> {
  opening.value = true;
  try {
    await api.openScreenRecordingSettings();
  } catch (e) {
    console.error('failed to open System Settings:', e);
  } finally {
    opening.value = false;
  }
}

async function recheck(): Promise<void> {
  rechecking.value = true;
  try {
    // Best-effort state-machine nudge. We don't depend on it showing a prompt;
    // the real status comes from the next check.
    try {
      await api.requestScreenRecordingPermission();
    } catch {
      /* ignore */
    }
    const ok = await api.checkScreenRecordingPermission();
    if (ok) show.value = false;
  } finally {
    rechecking.value = false;
  }
}

onMounted(() => {
  void checkAndShow();
  window.addEventListener('keydown', onKey);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKey);
});

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && show.value) show.value = false;
}

defineExpose({ checkAndShow });
</script>

<style scoped>
.pm-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(8, 10, 14, 0.7);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 110;
  padding: var(--sp-6);
}

.pm-card {
  width: 100%;
  max-width: 460px;
  background: var(--surface);
  border: var(--line);
  border-radius: var(--radius-lg);
  padding: var(--sp-6);
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

.pm-eyebrow {
  font-size: var(--fs-12);
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--warn);
}

.pm-message {
  font-family: var(--font-display);
  font-size: var(--fs-22);
  line-height: 1.3;
  letter-spacing: -0.01em;
  color: var(--text-strong);
}

.pm-steps {
  font-size: var(--fs-13);
  color: var(--muted);
  padding: var(--sp-3) var(--sp-5);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg);
  list-style: decimal;
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.pm-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--sp-2);
  margin-top: var(--sp-3);
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
    color var(--motion) ease,
    border-color var(--motion) ease,
    opacity var(--motion) ease;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-accent {
  background: var(--accent);
  color: #0a1413;
}
.btn-accent:hover:not(:disabled) {
  background: var(--accent-strong);
}

.btn-ghost {
  border-color: var(--border-strong);
  color: var(--text);
  background: transparent;
}
.btn-ghost:hover:not(:disabled) {
  background: var(--surface-2);
}
</style>
