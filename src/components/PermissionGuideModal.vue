<template>
  <Teleport to="body">
    <div v-if="open" class="pg-backdrop" @click.self="$emit('close')">
      <aside class="pg-modal" role="dialog" :aria-label="t('permission.guideTitle')">
        <header class="pg-head">
          <div class="pg-title-wrap">
            <span class="pg-shield-icon" aria-hidden="true">🛡️</span>
            <h2>{{ t('permission.guideTitle') }}</h2>
          </div>
          <button class="pg-close" type="button" aria-label="Close" @click="$emit('close')">×</button>
        </header>

        <div class="pg-body">
          <ol class="pg-steps">
            <li class="pg-step">
              <span class="pg-step-num">1</span>
              <div class="pg-step-content">
                <strong>{{ t('permission.step1Title') }}</strong>
                <p>{{ t('permission.step1Desc') }}</p>
              </div>
            </li>
            <li class="pg-step">
              <span class="pg-step-num">2</span>
              <div class="pg-step-content">
                <strong>{{ t('permission.step2Title') }}</strong>
                <p>{{ t('permission.step2Desc') }}</p>
              </div>
            </li>
            <li class="pg-step">
              <span class="pg-step-num">3</span>
              <div class="pg-step-content">
                <strong>{{ t('permission.step3Title') }}</strong>
                <p>{{ t('permission.step3Desc') }}</p>
              </div>
            </li>
          </ol>

          <div class="pg-fallback-box">
            <div class="pg-fallback-head">
              <span class="pg-light-icon" aria-hidden="true">💡</span>
              <strong>{{ t('permission.fallbackTitle') }}</strong>
            </div>
            <p>{{ t('permission.fallbackDesc') }}</p>
          </div>

          <div v-if="recheckMessage" class="pg-feedback" :class="{ success: isGranted }">
            {{ recheckMessage }}
          </div>
        </div>

        <footer class="pg-foot">
          <button class="pg-btn pg-btn-primary" type="button" @click="handleOpenSettings">
            <span>{{ t('permission.openSettings') }}</span>
            <span aria-hidden="true">↗</span>
          </button>
          <button class="pg-btn pg-btn-secondary" type="button" :disabled="rechecking" @click="handleRecheck">
            <span>{{ rechecking ? t('source.loading') : t('permission.recheck') }}</span>
          </button>
        </footer>
      </aside>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '../utils/api';

defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: 'close'): void; (e: 'granted'): void }>();

const { t } = useI18n();
const rechecking = ref(false);
const recheckMessage = ref('');
const isGranted = ref(false);

async function handleOpenSettings() {
  recheckMessage.value = '';
  try {
    await api.requestAndOpenScreenRecordingSettings();
  } catch (e) {
    console.error('Failed to open screen recording settings:', e);
  }
}

async function handleRecheck() {
  if (rechecking.value) return;
  rechecking.value = true;
  recheckMessage.value = '';
  try {
    const granted = await api.checkScreenRecordingPermission();
    if (granted) {
      isGranted.value = true;
      recheckMessage.value = t('permission.recheckSuccess');
      emit('granted');
      setTimeout(() => {
        emit('close');
      }, 1000);
    } else {
      isGranted.value = false;
      recheckMessage.value = t('permission.recheckFail');
    }
  } finally {
    rechecking.value = false;
  }
}
</script>

<style scoped>
.pg-backdrop {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(2px);
  padding: 16px;
}

.pg-modal {
  width: 100%;
  max-width: 420px;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--surface);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.2);
  color: var(--text);
  font-family: var(--font-body);
}

.pg-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: var(--line);
  background: var(--group);
}

.pg-title-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}

.pg-shield-icon {
  font-size: 18px;
  line-height: 1;
}

.pg-head h2 {
  font-size: 15px;
  font-weight: 650;
  color: var(--text-strong);
  margin: 0;
}

.pg-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  color: var(--muted);
  font-size: 20px;
  line-height: 1;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
}

.pg-close:hover {
  background: var(--control);
  color: var(--text-strong);
}

.pg-body {
  padding: 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.pg-steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.pg-step {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.pg-step-num {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  border-radius: 50%;
  background: var(--accent-dim);
  color: var(--accent);
  font-weight: 700;
  font-size: 12px;
  margin-top: 2px;
}

.pg-step-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.pg-step-content strong {
  font-size: 13px;
  color: var(--text-strong);
}

.pg-step-content p {
  margin: 0;
  font-size: 12px;
  color: var(--muted);
  line-height: 1.45;
}

.pg-fallback-box {
  padding: 10px 12px;
  background: var(--canvas);
  border: 1px dashed var(--accent-line);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.pg-fallback-head {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--accent-strong);
  font-size: 12px;
}

.pg-light-icon {
  font-size: 14px;
}

.pg-fallback-box p {
  margin: 0;
  font-size: 11px;
  color: var(--muted);
  line-height: 1.45;
}

.pg-feedback {
  padding: 8px 12px;
  font-size: 12px;
  text-align: center;
  border-radius: var(--radius-sm);
  background: var(--danger-dim);
  color: var(--danger);
}

.pg-feedback.success {
  background: rgba(53, 165, 91, 0.12);
  color: var(--success);
}

.pg-foot {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 16px;
  border-top: var(--line);
  background: var(--group);
}

.pg-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 9px 16px;
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast) ease-out;
}

.pg-btn-primary {
  background: var(--accent);
  color: #fff;
  border: none;
}

.pg-btn-primary:hover {
  background: var(--accent-strong);
}

.pg-btn-secondary {
  background: var(--control);
  color: var(--text);
  border: 1px solid var(--border);
}

.pg-btn-secondary:hover {
  background: var(--surface-3, #e2dbd1);
}

.pg-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
