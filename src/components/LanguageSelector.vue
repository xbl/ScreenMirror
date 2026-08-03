<template>
  <select
    class="ls"
    :value="i18n.locale.value"
    @change="onChange"
    :aria-label="t('settings.language')"
  >
    <option value="en">English</option>
    <option value="zh-CN">简体中文</option>
  </select>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { api } from '../utils/api';
import { setLocale } from '../i18n';

const i18n = useI18n();
async function onChange(e: Event) {
  const lang = (e.target as HTMLSelectElement).value;
  await setLocale(lang);
  try {
    await api.setAppLanguage(lang);
  } catch {
    /* ignore */
  }
}

// Bring t into scope so the template binding compiles even if unused here.
const _t = useI18n().t;
const t = _t;
</script>

<style scoped>
.ls {
  appearance: none;
  background: var(--surface-2);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  padding: 8px 32px 8px 12px;
  font-size: var(--fs-14);
  min-height: 32px;
}

.ls:hover {
  border-color: var(--muted);
}
</style>
