<template>
  <div class="device-card">
    <h3>{{ t('viewer.myDevice') }}</h3>
    <div>{{ t('viewer.deviceType') }}: {{ device.deviceType || '—' }}</div>
    <div class="ip">{{ t('viewer.deviceIp') }}: <strong>{{ device.ip || '—' }}</strong></div>
    <div>{{ t('viewer.deviceBrowser') }}: {{ device.browser || '—' }}</div>
    <div>{{ t('viewer.deviceOs') }}: {{ device.os || '—' }}</div>
    <div>{{ t('viewer.connectionId') }}: {{ device.roomId || '—' }}</div>
    <p class="hint">{{ t('viewer.ipHelp') }}</p>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
defineProps<{
  device: { ip: string; os: string; browser: string; deviceType: string; roomId: string };
}>();
const { t } = useI18n();
</script>

<style scoped>
.device-card {
  background: var(--group);
  padding: 16px;
  border-radius: var(--radius-lg);
  border: var(--line);
}
.ip {
  background: var(--accent-dim);
  font-weight: 700;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  display: inline-block;
}
.hint {
  color: var(--muted);
  font-size: 12px;
  margin-top: 8px;
}
</style>
