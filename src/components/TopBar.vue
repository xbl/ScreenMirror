<template>
  <header class="top-bar">
    <div class="top-brand">
      <span class="top-mark" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="20" height="20">
          <rect
            x="2.5"
            y="4"
            width="19"
            height="13"
            rx="2"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
          />
          <path
            d="M9 20h6M12 17v3"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
          />
          <path
            d="M14 10.5c0-.8.6-1.5 1.4-1.5"
            stroke="currentColor"
            stroke-width="1.2"
            stroke-linecap="round"
            fill="none"
          />
        </svg>
      </span>
      <span class="top-name">Screenmirror</span>
    </div>
    <nav class="top-actions">
      <button
        class="top-btn"
        type="button"
        :title="t('topbar.devices')"
        :aria-label="t('topbar.devices')"
        @click="emit('open-devices')"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <rect
            x="3"
            y="5"
            width="13"
            height="9"
            rx="1.4"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
          />
          <path
            d="M16 9l5-2v6l-5-2v-2z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linejoin="round"
          />
        </svg>
        <span v-if="viewerCount > 0" class="top-badge">{{ viewerCount }}</span>
      </button>
      <button
        class="top-btn"
        type="button"
        :title="t('topbar.settings')"
        :aria-label="t('topbar.settings')"
        @click="emit('open-settings')"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <circle
            cx="12"
            cy="12"
            r="3"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
          />
          <path
            d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1A2 2 0 1 1 4.4 17l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1A2 2 0 1 1 7 4.4l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1A2 2 0 1 1 19.6 7l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linejoin="round"
          />
        </svg>
      </button>
    </nav>
  </header>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{ viewerCount: number }>();

const emit = defineEmits<{
  (e: 'open-devices'): void;
  (e: 'open-settings'): void;
}>();

const { t } = useI18n();
</script>

<style scoped>
.top-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 52px;
  padding: 0 var(--sp-6);
  background: var(--bg);
  border-bottom: var(--line);
  position: sticky;
  top: 0;
  z-index: 5;
}

.top-brand {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--text);
}

.top-mark {
  display: inline-flex;
  width: 22px;
  height: 22px;
  align-items: center;
  justify-content: center;
  color: var(--accent);
}

.top-name {
  font-family: var(--font-display);
  font-size: var(--fs-16);
  font-weight: 500;
  letter-spacing: -0.01em;
  color: var(--text-strong);
}

.top-actions {
  display: flex;
  gap: var(--sp-2);
}

.top-btn {
  position: relative;
  width: 34px;
  height: 34px;
  border-radius: var(--radius-md);
  color: var(--muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition:
    color var(--motion) ease,
    background var(--motion) ease;
}

.top-btn:hover {
  color: var(--text);
  background: var(--surface-2);
}

.top-badge {
  position: absolute;
  top: 2px;
  right: 2px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--accent);
  color: #0a1413;
  font-size: 10px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
</style>