<template>
  <div class="start-bar">
    <button
      v-if="!sharing"
      class="start-btn"
      type="button"
      @click="onStart"
    >
      <span class="start-btn-text">{{ t('start.idle') }}</span>
      <span class="start-btn-arrow" aria-hidden="true">→</span>
    </button>
    <div v-else class="start-status" role="status">
      <span class="start-pulse" aria-hidden="true" />
      <span class="start-status-text">
        {{ t('start.sharing') }}
      </span>
      <span class="start-status-sep" aria-hidden="true">·</span>
      <span class="start-status-meta">{{ viewerLabel }}</span>
      <button class="start-stop" type="button" @click="onStop">
        {{ t('start.stop') }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, inject } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../utils/api';
import { PermissionModalKey, type ProvidedPermissionModal } from './PermissionModalHost';

const props = defineProps<{
  sharing: boolean;
  viewerCount: number;
}>();

const emit = defineEmits<{
  (e: 'update:sharing', value: boolean): void;
  (e: 'update:viewerCount', value: number): void;
}>();

const { t } = useI18n();

const permissionModal = inject<ProvidedPermissionModal>(
  PermissionModalKey,
  // Safe fallback when the host doesn't provide a modal.
  { value: null } as ProvidedPermissionModal,
);

const viewerLabel = computed(() => {
  const n = props.viewerCount;
  if (n <= 0) return '';
  if (n === 1) return t('card.oneViewer');
  return t('card.manyViewers', { n });
});

async function onStart() {
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
    await api.startSharing();
    emit('update:sharing', true);
  } catch {
    /* ignore — surface elsewhere */
  }
}

async function onStop() {
  try {
    await api.disconnectAllDevices();
  } catch {
    /* ignore */
  }
  emit('update:sharing', false);
  emit('update:viewerCount', 0);
}
</script>

<style scoped>
.start-bar {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.start-btn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 18px 24px;
  border-radius: var(--radius-md);
  background: var(--accent);
  color: #0a1413;
  font-size: var(--fs-16);
  font-weight: 500;
  letter-spacing: -0.005em;
  transition:
    background var(--motion) ease,
    transform var(--motion) ease;
}

.start-btn:hover {
  background: var(--accent-strong);
}

.start-btn:active {
  transform: translateY(1px);
}

.start-btn-arrow {
  font-size: var(--fs-18);
  margin-left: var(--sp-3);
}

.start-status {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 16px 20px;
  border-radius: var(--radius-md);
  background: var(--surface-2);
  border: 1px solid var(--accent-line);
  color: var(--text);
  font-size: var(--fs-15);
}

.start-pulse {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 4px var(--accent-dim);
  animation: ring 1.6s ease-out infinite;
  flex-shrink: 0;
}

@keyframes ring {
  0% {
    box-shadow: 0 0 0 0 var(--accent-line);
  }
  100% {
    box-shadow: 0 0 0 8px transparent;
  }
}

.start-status-text {
  font-weight: 500;
  color: var(--text-strong);
}

.start-status-sep {
  color: var(--muted);
}

.start-status-meta {
  color: var(--muted);
  font-size: var(--fs-14);
}

.start-stop {
  margin-left: auto;
  padding: 6px 14px;
  border-radius: var(--radius-pill);
  border: 1px solid var(--border-strong);
  color: var(--text);
  font-size: var(--fs-13);
  background: transparent;
  transition:
    background var(--motion) ease,
    border-color var(--motion) ease;
}

.start-stop:hover {
  background: var(--surface-3);
  border-color: var(--muted);
}

@media (prefers-reduced-motion: reduce) {
  .start-pulse {
    animation: none;
  }
  .start-btn {
    transition: none;
  }
}
</style>
