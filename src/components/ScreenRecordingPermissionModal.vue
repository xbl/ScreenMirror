<template>
  <Teleport to="body">
    <div v-if="show" class="pm-backdrop">
      <div class="pm-card" role="alertdialog" :aria-label="t('permission.title')">
        <span class="pm-eyebrow">{{ t('permission.title') }}</span>
        <p class="pm-message">{{ t('permission.message') }}</p>
        <p class="pm-reminder">{{ t('permission.restartReminder') }}</p>
        <div class="pm-actions">
          <button class="btn btn-ghost" @click="openSettings">
            {{ t('permission.openSettings') }}
          </button>
          <button class="btn btn-accent" @click="relaunchApp">
            {{ t('permission.restart') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../utils/api';
import { relaunch } from '@tauri-apps/plugin-process';

const { t } = useI18n();
const show = ref(false);

onMounted(async () => {
  try {
    const ok = await api.checkScreenRecordingPermission();
    show.value = !ok;
  } catch {
    /* running outside Tauri — leave hidden */
  }
});

async function openSettings() {
  try {
    await api.openExternalLink(
      'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture',
    );
  } catch {
    /* ignore */
  }
}

async function relaunchApp() {
  try {
    await relaunch();
  } catch {
    try {
      await api.relaunchApp();
    } catch {
      /* ignore */
    }
  }
}
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

.pm-reminder {
  font-size: var(--fs-13);
  color: var(--muted);
  padding: var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg);
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
    border-color var(--motion) ease;
}

.btn-accent {
  background: var(--accent);
  color: #0a1413;
}
.btn-accent:hover {
  background: var(--accent-strong);
}

.btn-ghost {
  border-color: var(--border-strong);
  color: var(--text);
  background: transparent;
}
.btn-ghost:hover {
  background: var(--surface-2);
}
</style>