<template>
  <Teleport to="body">
    <div v-if="open" class="up-backdrop" @click.self="$emit('close')">
      <aside class="up-modal" role="dialog" :aria-label="t('updater.title')">
        <header class="up-head">
          <div class="up-title-wrap">
            <span class="up-icon" aria-hidden="true">
              <template v-if="checking">🔄</template>
              <template v-else-if="status === 'ready'">🎉</template>
              <template v-else-if="status === 'error' || error">⚠️</template>
              <template v-else-if="updateInfo?.hasUpdate">🚀</template>
              <template v-else>✅</template>
            </span>
            <div>
              <h2>{{ t('updater.title') }}</h2>
              <p v-if="updateInfo?.currentVersion" class="up-subtitle">
                {{ t('updater.currentVersion', { version: updateInfo.currentVersion }) }}
              </p>
            </div>
          </div>
          <button class="up-close" type="button" aria-label="Close" @click="$emit('close')">
            ×
          </button>
        </header>

        <div class="up-body">
          <!-- 1. Checking State -->
          <div v-if="checking" class="up-state-box up-loading">
            <div class="up-spinner" aria-hidden="true" />
            <p>{{ t('updater.checking') }}</p>
          </div>

          <!-- 2. Error State -->
          <div v-else-if="error || status === 'error'" class="up-state-box up-error">
            <p class="up-error-text">{{ error || errorMessage || t('updater.failed') }}</p>
            <button class="up-btn up-btn-secondary" type="button" @click="$emit('check')">
              {{ t('settings.checkForUpdates') }}
            </button>
          </div>

          <!-- 3. Up to date State -->
          <div v-else-if="updateInfo && !updateInfo.hasUpdate" class="up-state-box up-success">
            <div class="up-badge-check">✓</div>
            <h3>{{ t('updater.upToDate') }}</h3>
            <p class="up-desc">
              {{ t('updater.upToDateDetail', { version: updateInfo.currentVersion }) }}
            </p>
          </div>

          <!-- 4. Update Available State -->
          <div v-else-if="updateInfo && updateInfo.hasUpdate" class="up-update-content">
            <div class="up-version-banner">
              <span class="up-new-tag">NEW</span>
              <strong>{{
                t('updater.newVersionFound', { version: updateInfo.latestVersion })
              }}</strong>
              <span v-if="updateInfo.publishedAt" class="up-date">
                {{ formatDate(updateInfo.publishedAt) }}
              </span>
            </div>

            <!-- Download Progress Bar -->
            <div
              v-if="status === 'downloading' || status === 'extracting'"
              class="up-progress-card"
            >
              <div class="up-progress-info">
                <span>{{
                  status === 'extracting' ? t('updater.downloadSuccess') : t('updater.downloading')
                }}</span>
                <span>{{ percent.toFixed(0) }}%</span>
              </div>
              <div class="up-progress-track">
                <div class="up-progress-bar" :style="{ width: percent + '%' }" />
              </div>
              <p class="up-progress-detail">
                {{ formatBytes(downloadedBytes) }} /
                {{ totalBytes ? formatBytes(totalBytes) : '—' }}
              </p>
            </div>

            <!-- Ready to restart banner -->
            <div v-else-if="status === 'ready'" class="up-ready-banner">
              <span class="up-ready-icon" aria-hidden="true">✨</span>
              <p>{{ t('updater.readyToRestart') }}</p>
            </div>

            <!-- Release Notes / Changelog -->
            <div v-if="updateInfo.releaseNotes && status !== 'downloading'" class="up-notes-box">
              <h4 class="up-notes-title">{{ t('updater.releaseNotes') }}</h4>
              <div class="up-notes-body">{{ updateInfo.releaseNotes }}</div>
            </div>
          </div>
        </div>

        <footer class="up-foot">
          <!-- Ready State: Restart App -->
          <template v-if="status === 'ready'">
            <button class="up-btn up-btn-primary" type="button" @click="handleRestart">
              <span>{{ t('updater.restartNow') }}</span>
            </button>
            <button class="up-btn up-btn-ghost" type="button" @click="$emit('close')">
              {{ t('updater.later') }}
            </button>
          </template>

          <!-- Update Available State -->
          <template v-else-if="updateInfo?.hasUpdate">
            <button
              v-if="status !== 'downloading' && status !== 'extracting' && updateInfo.downloadUrl"
              class="up-btn up-btn-primary"
              type="button"
              @click="handleStartUpdate"
            >
              <span>{{ t('updater.updateNow') }}</span>
            </button>
            <button
              v-if="status !== 'downloading' && status !== 'extracting'"
              class="up-btn up-btn-secondary"
              type="button"
              @click="handleOpenGithub"
            >
              <span>{{ t('updater.viewOnGithub') }}</span>
              <span aria-hidden="true">↗</span>
            </button>
            <button class="up-btn up-btn-ghost" type="button" @click="$emit('close')">
              {{ t('updater.close') }}
            </button>
          </template>

          <!-- Up to Date / Error -->
          <template v-else>
            <button class="up-btn up-btn-secondary" type="button" @click="handleOpenGithub">
              <span>{{ t('updater.viewOnGithub') }}</span>
              <span aria-hidden="true">↗</span>
            </button>
            <button class="up-btn up-btn-primary" type="button" @click="$emit('close')">
              {{ t('updater.close') }}
            </button>
          </template>
        </footer>
      </aside>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type DownloadProgress, type UpdateInfo } from '../utils/api';

const props = defineProps<{
  open: boolean;
  updateInfo: UpdateInfo | null;
  checking: boolean;
  error: string | null;
}>();

defineEmits<{
  (e: 'close'): void;
  (e: 'check'): void;
}>();

const { t } = useI18n();

const status = ref<'idle' | 'downloading' | 'extracting' | 'ready' | 'error'>('idle');
const percent = ref(0);
const downloadedBytes = ref(0);
const totalBytes = ref<number | null>(null);
const errorMessage = ref<string | null>(null);

let unlistenProgress: UnlistenFn | undefined;

onMounted(async () => {
  try {
    unlistenProgress = await listen<DownloadProgress>('update-download-progress', (event) => {
      const p = event.payload;
      downloadedBytes.value = p.downloadedBytes;
      totalBytes.value = p.totalBytes;
      percent.value = p.percent;
      status.value = p.status;
      if (p.status === 'error') {
        errorMessage.value = p.message || 'Download failed';
      }
    });
  } catch (err) {
    console.warn('Failed to listen for update progress:', err);
  }
});

onBeforeUnmount(() => {
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = undefined;
  }
});

async function handleStartUpdate() {
  if (!props.updateInfo?.downloadUrl) return;
  status.value = 'downloading';
  percent.value = 0;
  errorMessage.value = null;

  try {
    await api.downloadAndInstallUpdate(props.updateInfo.downloadUrl);
  } catch (err) {
    status.value = 'error';
    errorMessage.value = err instanceof Error ? err.message : String(err);
  }
}

async function handleRestart() {
  try {
    await api.relaunchApp();
  } catch (err) {
    console.error('Failed to restart app:', err);
  }
}

async function handleOpenGithub() {
  if (props.updateInfo?.releaseUrl) {
    await api.openExternalLink(props.updateInfo.releaseUrl);
  } else {
    await api.openExternalLink('https://github.com/xbl/ScreenMirror/releases');
  }
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
  } catch {
    return iso;
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}
</script>

<style scoped>
.up-backdrop {
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, var(--canvas) 70%, transparent);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: 16px;
  animation: fadeIn 0.16s ease-out;
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.up-modal {
  width: 440px;
  max-width: 100%;
  max-height: 85vh;
  background: var(--surface);
  border: var(--line);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: slideUp 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes slideUp {
  from {
    transform: translateY(16px);
    opacity: 0.8;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

.up-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: var(--line);
  background: color-mix(in srgb, var(--surface) 90%, transparent);
}

.up-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.up-icon {
  font-size: 24px;
}

.up-title-wrap h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.up-subtitle {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--muted);
}

.up-close {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    background 0.15s,
    color 0.15s;
}

.up-close:hover {
  background: color-mix(in srgb, var(--border) 60%, transparent);
  color: var(--text);
}

.up-body {
  padding: 20px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.up-state-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 24px 16px;
  gap: 12px;
}

.up-spinner {
  width: 28px;
  height: 28px;
  border: 3px solid color-mix(in srgb, var(--accent) 25%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.up-badge-check {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  background: color-mix(in srgb, #22c55e 18%, transparent);
  color: #22c55e;
  font-size: 22px;
  font-weight: bold;
  display: flex;
  align-items: center;
  justify-content: center;
}

.up-desc {
  margin: 0;
  font-size: 13px;
  color: var(--muted);
  line-height: 1.5;
}

.up-error-text {
  color: #ef4444;
  font-size: 13px;
  margin: 0;
}

.up-version-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
  border-radius: var(--radius-md);
  font-size: 13px;
}

.up-new-tag {
  background: var(--accent);
  color: #fff;
  font-size: 10px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 4px;
  letter-spacing: 0.5px;
}

.up-date {
  margin-left: auto;
  font-size: 12px;
  color: var(--muted);
}

.up-progress-card {
  padding: 14px;
  background: color-mix(in srgb, var(--border) 40%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.up-progress-info {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  font-weight: 500;
}

.up-progress-track {
  width: 100%;
  height: 6px;
  background: color-mix(in srgb, var(--border) 80%, transparent);
  border-radius: 3px;
  overflow: hidden;
}

.up-progress-bar {
  height: 100%;
  background: var(--accent);
  border-radius: 3px;
  transition: width 0.15s ease-out;
}

.up-progress-detail {
  margin: 0;
  font-size: 11px;
  color: var(--muted);
  text-align: right;
}

.up-ready-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  background: color-mix(in srgb, #22c55e 12%, var(--surface));
  border: 1px solid color-mix(in srgb, #22c55e 30%, transparent);
  border-radius: var(--radius-md);
  color: var(--text);
  font-size: 13px;
}

.up-ready-icon {
  font-size: 20px;
}

.up-notes-box {
  background: color-mix(in srgb, var(--canvas) 60%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 12px;
  max-height: 180px;
  overflow-y: auto;
}

.up-notes-title {
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.up-notes-body {
  font-size: 12px;
  line-height: 1.6;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
}

.up-foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 20px;
  border-top: var(--line);
  background: color-mix(in srgb, var(--surface) 95%, transparent);
}

.up-btn {
  height: 34px;
  padding: 0 14px;
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition: all 0.15s ease-out;
  border: 1px solid transparent;
}

.up-btn-primary {
  background: var(--accent);
  color: #fff;
}

.up-btn-primary:hover {
  background: var(--accent-strong);
}

.up-btn-secondary {
  background: color-mix(in srgb, var(--surface) 70%, transparent);
  border-color: var(--border);
  color: var(--text);
}

.up-btn-secondary:hover {
  background: var(--surface);
  border-color: var(--text-muted);
}

.up-btn-ghost {
  background: transparent;
  color: var(--muted);
}

.up-btn-ghost:hover {
  color: var(--text);
}
</style>
