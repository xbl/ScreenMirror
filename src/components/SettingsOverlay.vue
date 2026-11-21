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
          <button
            class="st-icon-button"
            type="button"
            :aria-label="t('permission.openSettings')"
            @click="openPermissionSettings"
          >
            <span aria-hidden="true">↗</span>
          </button>
        </section>

        <section class="st-section st-version-section">
          <div class="st-version-info">
            <span class="st-label">{{ t('settings.version') }}</span>
            <div class="st-version-row">
              <span class="st-value st-mono">{{ version ? `v${version}` : '—' }}</span>
              <span v-if="updateInfo?.hasUpdate" class="st-badge-update">
                {{ t('settings.newVersionFound') }}
              </span>
              <span v-else-if="checkedOnce && !checking" class="st-badge-ok">
                {{ t('settings.upToDate') }}
              </span>
            </div>
          </div>
          <button
            class="st-btn-check"
            type="button"
            :disabled="checking"
            @click="handleCheckUpdates"
          >
            <span v-if="checking" class="st-spinner-small" aria-hidden="true" />
            <span>{{
              checking ? t('settings.checkingUpdates') : t('settings.checkForUpdates')
            }}</span>
          </button>
        </section>

        <section class="st-section st-inline-section">
          <label class="st-label" for="auto-update-toggle">{{
            t('settings.autoCheckUpdates')
          }}</label>
          <input
            id="auto-update-toggle"
            type="checkbox"
            class="st-switch"
            :checked="autoCheckEnabled"
            @change="toggleAutoCheck"
          />
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
    <PermissionGuideModal
      :open="showPermissionGuide"
      @close="showPermissionGuide = false"
      @granted="checkPermission"
    />
    <UpdateModal
      :open="showUpdateModal"
      :update-info="updateInfo"
      :checking="checking"
      :error="updateError"
      @close="showUpdateModal = false"
      @check="handleCheckUpdates"
    />
  </Teleport>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import LanguageSelector from './LanguageSelector.vue';
import PermissionGuideModal from './PermissionGuideModal.vue';
import UpdateModal from './UpdateModal.vue';
import { api, type UpdateInfo } from '../utils/api';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'reset'): void;
}>();

const { t } = useI18n();
const version = ref('');
const permissionGranted = ref(false);
const showPermissionGuide = ref(false);

const showUpdateModal = ref(false);
const checking = ref(false);
const checkedOnce = ref(false);
const updateInfo = ref<UpdateInfo | null>(null);
const updateError = ref<string | null>(null);
const autoCheckEnabled = ref(true);

onMounted(async () => {
  try {
    version.value = await api.getCurrentVersion();
  } catch {
    version.value = '';
  }
  try {
    autoCheckEnabled.value = await api.getAutoCheckUpdate();
  } catch {
    autoCheckEnabled.value = true;
  }
});

async function toggleAutoCheck(e: Event) {
  const target = e.target as HTMLInputElement;
  autoCheckEnabled.value = target.checked;
  try {
    await api.setAutoCheckUpdate(target.checked);
  } catch (err) {
    console.error('Failed to set auto check update:', err);
  }
}

async function handleCheckUpdates() {
  checking.value = true;
  updateError.value = null;
  showUpdateModal.value = true;
  try {
    const info = await api.checkForUpdates();
    updateInfo.value = info;
    checkedOnce.value = true;
  } catch (err) {
    updateError.value = err instanceof Error ? err.message : String(err);
  } finally {
    checking.value = false;
  }
}

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
  if (!permissionGranted.value) {
    showPermissionGuide.value = true;
  } else {
    await api.requestAndOpenScreenRecordingSettings();
  }
}

async function openProjectHomepage() {
  await api.openExternalLink('https://github.com/xbl/ScreenMirror');
}

watch(
  () => props.open,
  (open) => {
    if (open) void checkPermission();
  },
  { immediate: true },
);

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

.st-version-section {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.st-version-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.st-version-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.st-badge-update {
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  background: var(--accent);
  padding: 1px 6px;
  border-radius: 4px;
}

.st-badge-ok {
  font-size: 11px;
  font-weight: 500;
  color: var(--success);
}

.st-btn-check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--control);
  border: var(--line);
  border-radius: var(--radius-md);
  color: var(--text);
  font-size: var(--fs-13);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.st-btn-check:hover:not(:disabled) {
  background: var(--accent-dim);
  color: var(--text-strong);
  border-color: var(--accent);
}

.st-btn-check:disabled {
  opacity: 0.6;
  cursor: default;
}

.st-spinner-small {
  width: 12px;
  height: 12px;
  border: 2px solid color-mix(in srgb, var(--accent) 30%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.st-switch {
  appearance: none;
  width: 36px;
  height: 20px;
  background: var(--border);
  border-radius: 10px;
  position: relative;
  cursor: pointer;
  outline: none;
  transition: background 0.2s ease;
}

.st-switch:checked {
  background: var(--accent);
}

.st-switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  background: #fff;
  border-radius: 50%;
  transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.st-switch:checked::after {
  transform: translateX(16px);
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
