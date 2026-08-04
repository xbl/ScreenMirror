<template>
  <Teleport to="body">
    <div v-if="open" class="st-backdrop" @click.self="$emit('close')">
      <aside class="st-panel" role="dialog" :aria-label="t('settings.title')">
        <header class="st-head">
          <span class="st-eyebrow">{{ t('settings.title') }}</span>
          <button class="st-close" @click="$emit('close')" aria-label="Close">×</button>
        </header>

        <section class="st-section st-inline-section">
          <label class="st-label">{{ t('settings.language') }}</label>
          <LanguageSelector />
        </section>

        <section class="st-section st-inline-section">
          <div>
            <span class="st-label">{{ t('permission.title') }}</span>
            <strong class="st-status" :data-ok="permissionGranted">
              {{ permissionGranted ? t('permission.granted') : t('permission.required') }}
            </strong>
          </div>
          <button class="st-icon-button" type="button" :aria-label="t('permission.openSettings')" @click="openPermissionSettings">
            <span aria-hidden="true">↗</span>
          </button>
        </section>

        <section class="st-section">
          <span class="st-label">{{ t('settings.version') }}</span>
          <span class="st-value st-mono">{{ version || '—' }}</span>
        </section>

        <button class="st-section st-project" type="button" @click="openProjectHomepage">
          <span>
            <span class="st-label">{{ t('settings.projectHomepage') }}</span>
            <strong>github.com/xbl/ScreenMirror</strong>
          </span>
          <span class="st-project-icon" aria-hidden="true">↗</span>
        </button>

        <footer class="st-foot">
          <button class="btn btn-ghost" type="button" @click="onReset">
            {{ t('app.reset') }}
          </button>
          <button class="btn btn-accent" type="button" @click="$emit('close')">
            {{ t('settings.close') }}
          </button>
        </footer>
      </aside>
    </div>
  </Teleport>

</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import LanguageSelector from './LanguageSelector.vue';
import { api } from '../utils/api';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'reset'): void;
}>();

const { t } = useI18n();
const version = ref('');
const permissionGranted = ref(false);

onMounted(async () => {
  try {
    version.value = await api.getCurrentVersion();
  } catch {
    version.value = '';
  }
});

async function checkPermission() {
  try {
    permissionGranted.value = await api.checkScreenRecordingPermission();
    if (!permissionGranted.value) {
      // Some macOS development builds report a stale preflight result even
      // though the capture APIs are usable. Treat successful enumeration as
      // the authoritative runtime check.
      try {
        await api.enumerateCaptureSources();
        permissionGranted.value = true;
      } catch {
        // Keep the preflight result when enumeration is unavailable.
      }
    }
  } catch {
    permissionGranted.value = false;
  }
}

async function openPermissionSettings() {
  await api.openScreenRecordingSettings();
}

async function openProjectHomepage() {
  await api.openExternalLink('https://github.com/xbl/ScreenMirror');
}

watch(() => props.open, (open) => {
  if (open) void checkPermission();
}, { immediate: true });

async function onReset() {
  try {
    await api.resetWaitingSession();
    await api.createWaitingSession(undefined);
  } catch {
    /* ignore */
  }
  emit('reset');
}
</script>

<style scoped>
.st-backdrop {
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, var(--canvas) 64%, transparent);
  backdrop-filter: blur(10px);
  display: flex;
  align-items: stretch;
  justify-content: flex-end;
  z-index: 90;
}

.st-panel {
  width: 380px;
  max-width: 100%;
  height: 100%;
  background: color-mix(in srgb, var(--surface) 96%, transparent);
  border-left: var(--line);
  display: flex;
  flex-direction: column;
  padding: var(--sp-5);
  gap: var(--sp-4);
  animation: slidein var(--motion) ease-out;
}

@keyframes slidein {
  from {
    transform: translateX(20px);
    opacity: 0;
  }
}

.st-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.st-eyebrow {
  color: var(--text-strong);
  font-size: var(--fs-18);
  font-weight: 650;
}

.st-close {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--muted);
  font-size: var(--fs-18);
  line-height: 1;
}
.st-close:hover {
  color: var(--text);
  background: var(--surface-2);
}

.st-section {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3) 0;
  border-bottom: var(--line);
}

.st-section:last-of-type {
  border-bottom: none;
}

.st-project {
  width: 100%;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  color: var(--text);
  text-align: left;
}

.st-project > span:first-child {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.st-project strong {
  overflow: hidden;
  color: var(--text);
  font-size: var(--fs-13);
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.st-project-icon {
  color: var(--accent);
  font-size: var(--fs-16);
}

.st-project:hover strong {
  color: var(--accent);
}

.st-inline-section {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.st-inline-section > div {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.st-status {
  color: var(--danger);
  font-size: var(--fs-13);
  font-weight: 500;
}

.st-status[data-ok='true'] {
  color: var(--success);
}

.st-icon-button {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  color: var(--muted);
  border: var(--line);
  border-radius: var(--radius-md);
  background: var(--control);
  cursor: pointer;
}

.st-icon-button:hover {
  color: var(--text-strong);
  background: var(--accent-dim);
}

.st-icon-button svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
}

.st-label {
  font-size: var(--fs-13);
  color: var(--muted);
}

.st-value {
  color: var(--text);
  font-size: var(--fs-14);
}

.st-mono {
  font-family: var(--font-mono);
}

.st-foot {
  margin-top: auto;
  display: flex;
  justify-content: space-between;
  gap: var(--sp-3);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 10px 18px;
  border-radius: var(--radius-md);
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
  color: #fff;
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

@media (prefers-reduced-motion: reduce) {
  .st-panel {
    animation: none;
  }
}
</style>
