<template>
  <!-- This callout is kept for backward compatibility with the drawer list,
       but the drawer now renders its own inline metadata. -->
  <div class="dic">
    <div class="dic-row">
      <span class="dic-key">IP</span>
      <span class="dic-val dic-mono">{{ device.ip }}</span>
    </div>
    <div class="dic-row">
      <span class="dic-key">OS</span>
      <span class="dic-val">{{ device.os }}</span>
    </div>
    <div class="dic-row">
      <span class="dic-key">Browser</span>
      <span class="dic-val">{{ device.browser }}</span>
    </div>
    <div class="dic-row">
      <span class="dic-key">Room</span>
      <span class="dic-val dic-mono">{{ device.roomId }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Device } from '../utils/api';
defineProps<{ device: Device }>();
</script>

<style scoped>
.dic {
  display: grid;
  grid-template-columns: auto 1fr;
  column-gap: var(--sp-3);
  row-gap: 2px;
  font-size: var(--fs-13);
}
.dic-row {
  display: contents;
}
.dic-key {
  color: var(--muted);
}
.dic-val {
  color: var(--text);
}
.dic-mono {
  font-family: var(--font-mono);
}
</style>