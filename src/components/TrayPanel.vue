<template>
  <main class="tray-panel">
    <header class="tray-head">
      <div class="tray-brand">
        <span class="tray-mark" aria-hidden="true"></span>
        <div>
          <p class="tray-kicker">Screenmirror</p>
          <h1>{{ t('tray.title') }}</h1>
        </div>
      </div>
      <button class="tray-close" type="button" :aria-label="t('tray.close')" @click="close">
        ×
      </button>
    </header>

    <QRCard class="tray-qr" />
    <SourcePicker class="tray-source" external-chooser />
  </main>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import QRCard from './QRCard.vue';
import SourcePicker from './SourcePicker.vue';
import { api } from '../utils/api';

const { t } = useI18n();

async function close() {
  try {
    await api.closeTrayPanel();
  } catch {
    window.close();
  }
}
</script>

<style scoped>
.tray-panel {
  min-height: 100vh;
  padding: 18px;
  color: var(--text);
  background: var(--bg);
}

.tray-head,
.tray-brand {
  display: flex;
  align-items: center;
}

.tray-head {
  justify-content: space-between;
  margin-bottom: 16px;
}

.tray-brand {
  gap: 10px;
}

.tray-mark {
  width: 30px;
  height: 30px;
  border: 2px solid var(--accent);
  border-radius: 9px;
  box-shadow: 5px 5px 0 -2px var(--bg), 5px 5px 0 0 var(--accent);
}

.tray-kicker {
  margin: 0;
  color: var(--muted);
  font-size: 10px;
  letter-spacing: .14em;
  text-transform: uppercase;
}

.tray-head h1 {
  margin: 2px 0 0;
  color: var(--text-strong);
  font-family: var(--font-display);
  font-size: 22px;
  font-weight: 500;
}

.tray-close {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  color: var(--muted);
  font-size: 23px;
  line-height: 1;
  border: 0;
  border-radius: 50%;
  background: var(--surface-2);
  cursor: pointer;
}

.tray-close:hover {
  color: var(--text-strong);
  background: var(--surface-3);
}

:deep(.qr-card) {
  grid-template-columns: 176px minmax(0, 1fr);
  gap: 14px;
  padding: 14px;
}

:deep(.qr-frame) {
  width: 160px;
  height: 160px;
  padding: 8px;
}

:deep(.qr-meta) {
  gap: 8px;
}

:deep(.qr-url) {
  max-width: 180px;
  font-size: 11px;
}

:deep(.qr-actions .btn) {
  width: 100%;
}

.tray-source {
  margin-top: 14px;
}

:deep(.source-picker) {
  padding: 14px;
}

</style>
