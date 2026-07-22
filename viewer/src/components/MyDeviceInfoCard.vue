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
  background: white;
  padding: 16px;
  border-radius: 8px;
  border: 1px solid #eee;
}
.ip {
  background: #00f99273;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 16px;
  display: inline-block;
}
.hint {
  color: #5c7080;
  font-size: 12px;
  margin-top: 8px;
}
</style>