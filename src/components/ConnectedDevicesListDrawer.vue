<template>
  <Teleport to="body">
    <div v-if="open" class="drw-backdrop" @click.self="$emit('close')">
      <aside class="drw-panel" role="dialog" :aria-label="t('devices.title')">
        <header class="drw-head">
          <span class="drw-eyebrow">{{ t('devices.title') }}</span>
          <button class="drw-close" @click="$emit('close')" aria-label="Close">×</button>
        </header>
        <div v-if="devices.length === 0" class="drw-empty">
          {{ t('devices.none') }}
        </div>
        <ul v-else class="drw-list">
          <li v-for="d in devices" :key="d.id" class="drw-item">
            <div class="drw-meta">
              <span class="drw-id">{{ d.ip }}</span>
              <span class="drw-sub">{{ d.os }} · {{ d.browser }}</span>
            </div>
          </li>
        </ul>
      </aside>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { api, type Device } from '../utils/api';

const props = defineProps<{ open: boolean }>();
defineEmits<{ (e: 'close'): void }>();
const { t } = useI18n();

const devices = ref<Device[]>([]);
let poll: number | undefined;

async function refresh() {
  try {
    devices.value = await api.getConnectedDevices();
  } catch {
    devices.value = [];
  }
}

onMounted(() => {
  void refresh();
  poll = window.setInterval(refresh, 3000);
});

onBeforeUnmount(() => {
  if (poll) clearInterval(poll);
});

watch(
  () => props.open,
  (v) => {
    if (v) void refresh();
  },
);
</script>

<style scoped>
.drw-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(8, 10, 14, 0.6);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: stretch;
  justify-content: flex-end;
  z-index: 90;
}

.drw-panel {
  width: 360px;
  max-width: 100%;
  height: 100%;
  background: var(--surface);
  border-left: var(--line);
  display: flex;
  flex-direction: column;
  padding: var(--sp-6);
  gap: var(--sp-4);
  animation: slidein var(--motion) ease-out;
}

@keyframes slidein {
  from {
    transform: translateX(20px);
    opacity: 0;
  }
}

.drw-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.drw-eyebrow {
  font-size: var(--fs-12);
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--accent);
}

.drw-close {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-md);
  color: var(--muted);
  font-size: var(--fs-18);
  line-height: 1;
}
.drw-close:hover {
  color: var(--text);
  background: var(--surface-2);
}

.drw-empty {
  color: var(--muted);
  font-size: var(--fs-14);
  padding: var(--sp-4) 0;
}

.drw-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.drw-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--sp-3) var(--sp-4);
  border: var(--line);
  border-radius: var(--radius-md);
  background: var(--bg);
}

.drw-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.drw-id {
  font-family: var(--font-mono);
  font-size: var(--fs-13);
  color: var(--text);
}

.drw-sub {
  font-size: var(--fs-12);
  color: var(--muted);
}

@media (prefers-reduced-motion: reduce) {
  .drw-panel {
    animation: none;
  }
}
</style>
