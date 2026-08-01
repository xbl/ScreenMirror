<template>
  <section class="source-picker" aria-labelledby="source-picker-title">
    <header class="sp-head">
      <div>
        <span id="source-picker-title" class="sp-eyebrow">{{ t('source.label') }}</span>
        <p class="sp-head-note">{{ t('source.chooseHint') }}</p>
      </div>
      <button class="sp-change sp-change-head" type="button" @click="openPicker">
        {{ t('source.change') }}
      </button>
    </header>

    <p v-if="error" class="sp-error" role="alert">{{ error }}</p>
    <div v-if="loading && !selectedSource" class="sp-loading" aria-live="polite">
      {{ t('source.loading') }}
    </div>
    <div v-else class="sp-current" data-testid="source-preview" aria-live="polite">
      <div class="sp-current-preview">
        <img v-if="selectedSource?.preview" :src="selectedSource.preview" alt="" />
        <span v-else-if="selectedSource && isPreviewLoading(selectedSource)" class="sp-preview-fallback">
          {{ t('source.previewLoading') }}
        </span>
        <span v-else class="sp-preview-fallback">{{ t('source.noPreview') }}</span>
      </div>
      <div class="sp-current-copy">
        <span class="sp-current-label">{{ t('source.current') }}</span>
        <span class="sp-current-name">{{ selectedSource?.name || t('source.noSources') }}</span>
        <span v-if="selectedSource" class="sp-current-meta">
          <span v-if="selectedSource.isPrimary" class="sp-primary">{{ t('source.primary') }}</span>
          <span>{{ resolution(selectedSource) }}</span>
        </span>
      </div>
      <button class="sp-change sp-change-current" type="button" @click="openPicker">
        {{ t('source.change') }}
      </button>
    </div>

    <label class="sp-quality">
      <span class="sp-quality-label">{{ t('source.quality') }}</span>
      <select :value="quality" :disabled="!selectedSource" @change="changeQuality">
        <option value="balanced">{{ t('source.qualityBalanced') }}</option>
        <option value="high">{{ t('source.qualityHigh') }}</option>
        <option value="ultra">{{ t('source.qualityUltra') }}</option>
      </select>
    </label>

    <div v-if="pickerOpen" class="sp-modal" @click.self="closePicker">
      <section class="sp-dialog" role="dialog" aria-modal="true" aria-labelledby="sp-dialog-title">
        <header class="sp-dialog-head">
          <button v-if="pickerStep !== 'types'" class="sp-icon-button" type="button" :aria-label="t('source.back')" @click="pickerStep = 'types'">
            <span aria-hidden="true">‹</span>
          </button>
          <div>
            <span class="sp-dialog-kicker">{{ t('source.label') }}</span>
            <h2 id="sp-dialog-title">{{ pickerStep === 'types' ? t('source.chooseType') : pickerStep === 'windows' ? t('source.chooseWindow') : t('source.chooseDisplay') }}</h2>
          </div>
          <button class="sp-icon-button" type="button" :aria-label="t('source.close')" @click="closePicker">×</button>
        </header>

        <template v-if="pickerStep === 'types'">
          <div class="sp-type-grid">
            <button class="sp-type-card" type="button" data-source-type="screen" @click="chooseType('screen')">
              <span class="sp-type-icon sp-icon-screen" aria-hidden="true"></span>
              <span class="sp-type-copy"><strong>{{ t('source.entireScreen') }}</strong><small>{{ t('source.entireScreenDescription') }}</small></span>
              <span class="sp-chevron" aria-hidden="true">›</span>
            </button>
            <button class="sp-type-card" type="button" data-source-type="window" @click="chooseType('windows')">
              <span class="sp-type-icon sp-icon-window" aria-hidden="true"></span>
              <span class="sp-type-copy"><strong>{{ t('source.windowsAndApps') }}</strong><small>{{ t('source.windowsDescription') }}</small></span>
              <span class="sp-chevron" aria-hidden="true">›</span>
            </button>
            <button class="sp-type-card" type="button" data-source-type="extended" @click="chooseType('extended')">
              <span class="sp-type-icon sp-icon-display" aria-hidden="true"></span>
              <span class="sp-type-copy"><strong>{{ t('source.extendedDisplays') }}</strong><small>{{ t('source.extendedDescription') }}</small></span>
              <span class="sp-chevron" aria-hidden="true">›</span>
            </button>
          </div>
        </template>

        <template v-else>
          <div class="sp-list-toolbar">
            <p>{{ pickerStep === 'windows' ? t('source.runningWindows') : t('source.availableDisplays') }}</p>
            <button class="sp-refresh" type="button" :disabled="loading || previewLoadingIds.size > 0" @click="refreshSources(true)">
              {{ t('source.refresh') }}
            </button>
          </div>
          <div v-if="loading" class="sp-loading" aria-live="polite">{{ t('source.loading') }}</div>
          <div v-else-if="pickerSources.length" class="sp-source-list">
            <button
              v-for="item in pickerSources"
              :key="sourceKey(item)"
              class="sp-source"
              :class="{ selected: sourceKey(item) === selectedSourceKey }"
              type="button"
              :data-source-id="item.sourceId"
              :data-source-key="sourceKey(item)"
              :aria-pressed="sourceKey(item) === selectedSourceKey"
              @click="selectAndClose(item)"
            >
              <span v-if="item.preview" class="sp-thumb" aria-hidden="true"><img :src="item.preview" alt="" /></span>
              <span v-else class="sp-thumb sp-thumb-fallback" aria-hidden="true"><span :class="item.kind === 'window' ? 'sp-icon-window' : 'sp-icon-display'"></span></span>
              <span class="sp-source-copy">
                <span class="sp-source-name">{{ item.name }}</span>
                <span class="sp-source-meta">
                  <span v-if="item.isPrimary" class="sp-primary">{{ t('source.primary') }}</span>
                  <span>{{ resolution(item) }}</span>
                </span>
              </span>
              <span class="sp-source-check" aria-hidden="true">{{ sourceKey(item) === selectedSourceKey ? '✓' : '›' }}</span>
            </button>
          </div>
          <div v-else class="sp-empty">{{ t('source.noSources') }}</div>
        </template>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, inject, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { api, type CaptureSourceInfo, type CaptureTarget } from '../utils/api';
import { PermissionModalKey, type ProvidedPermissionModal } from './PermissionModalHost';

type Quality = 'balanced' | 'high' | 'ultra';
type Operation = { kind: 'capture' | 'refresh'; sequence: number };

const { t } = useI18n();
const sources = ref<CaptureSourceInfo[]>([]);
const selectedSource = ref<CaptureSourceInfo | null>(null);
const quality = ref<Quality>('high');
const error = ref('');
const loading = ref(false);
const previewLoadingIds = ref(new Set<string>());
const pickerOpen = ref(false);
const pickerStep = ref<'types' | 'windows' | 'extended'>('types');
let captureOperation = 0;
let refreshOperation = 0;
let refreshGeneration = 0;

const permissionModal = inject<ProvidedPermissionModal>(
  PermissionModalKey,
  ref(null) as ProvidedPermissionModal,
);

const pickerSources = computed(() => pickerStep.value === 'windows'
  ? sources.value.filter((source) => source.kind === 'window')
  : sources.value.filter((source) => source.kind === 'screen' && !source.isPrimary));

function qualityValue(value: Quality) {
  return { balanced: 0.5, high: 0.75, ultra: 1.0 }[value];
}

function qualityName(value: number): Quality {
  if (value >= 0.9) return 'ultra';
  if (value >= 0.65) return 'high';
  return 'balanced';
}

function resolution(source: CaptureSourceInfo) {
  return t('source.resolution', { width: source.width, height: source.height });
}

function sourceKey(source: Pick<CaptureSourceInfo, 'kind' | 'sourceId'>) {
  return `${source.kind}:${source.sourceId}`;
}

const selectedSourceKey = computed(() =>
  selectedSource.value ? sourceKey(selectedSource.value) : null,
);

function isPreviewLoading(source: CaptureSourceInfo | null) {
  return source !== null && previewLoadingIds.value.has(sourceKey(source));
}

function openPicker() {
  pickerStep.value = 'types';
  pickerOpen.value = true;
}

function closePicker() {
  pickerOpen.value = false;
  pickerStep.value = 'types';
}

async function chooseType(type: 'screen' | 'windows' | 'extended') {
  if (type === 'screen') {
    const primary = sources.value.find((source) => source.kind === 'screen' && source.isPrimary);
    if (primary) {
      await selectAndClose(primary);
      return;
    }
  }
  pickerStep.value = type === 'windows' ? 'windows' : 'extended';
}

async function selectAndClose(source: CaptureSourceInfo) {
  if (await selectSource(source)) closePicker();
}

function captureTarget(source: CaptureSourceInfo, targetQuality = quality.value): CaptureTarget {
  const id = Number.parseInt(source.id.split(':').at(-1) ?? '', 10);
  return {
    kind: source.kind,
    id: Number.isNaN(id) ? 0 : id,
    sourceId: source.sourceId,
    quality: qualityValue(targetQuality),
  };
}

function isCurrentOperation(operation: Operation) {
  return operation.kind === 'capture'
    ? operation.sequence === captureOperation
    : operation.sequence === refreshOperation;
}

async function ensurePermission(operation: Operation) {
  try {
    const granted = await api.checkScreenRecordingPermission();
    if (granted) return true;
  } catch {
    if (isCurrentOperation(operation)) error.value = t('source.errorPermission');
    return false;
  }

  if (!isCurrentOperation(operation)) return false;
  error.value = t('source.errorPermission');
  await permissionModal.value?.checkAndShow();
  return false;
}

async function selectSource(source: CaptureSourceInfo, nextQuality = quality.value) {
  const operation: Operation = { kind: 'capture', sequence: ++captureOperation };
  error.value = '';
  if (!(await ensurePermission(operation))) return false;
  if (!isCurrentOperation(operation)) return false;

  try {
    await api.setCaptureTarget(captureTarget(source, nextQuality));
    if (!isCurrentOperation(operation)) return false;
    selectedSource.value = sources.value.find((item) => sourceKey(item) === sourceKey(source)) ?? source;
    quality.value = nextQuality;
    return true;
  } catch {
    if (!isCurrentOperation(operation)) return false;
    error.value = t('source.errorSwitch');
    return false;
  }
}

function refreshPreviews(available: CaptureSourceInfo[], generation: number, forceRefresh: boolean) {
  const displaySources = available.filter((source) => source.kind === 'screen');
  previewLoadingIds.value = new Set(displaySources.map((source) => sourceKey(source)));
  void Promise.all(displaySources
    .map(async (source) => {
      try {
        const preview = await api.getCaptureSourcePreview(source.sourceId, forceRefresh);
        if (!preview || generation !== refreshGeneration) return;

        let refreshedSource: CaptureSourceInfo | null = null;
        sources.value = sources.value.map((item) => {
          if (sourceKey(item) !== sourceKey(source)) return item;
          refreshedSource = { ...item, preview };
          return refreshedSource;
        });
        if (refreshedSource && selectedSource.value && sourceKey(selectedSource.value) === sourceKey(source)) {
          selectedSource.value = refreshedSource;
        }
      } catch {
        // Preview capture is best-effort; preserve the existing null fallback.
      } finally {
        if (generation === refreshGeneration) {
          const next = new Set(previewLoadingIds.value);
          next.delete(sourceKey(source));
          previewLoadingIds.value = next;
        }
      }
    }));
}

async function refreshSources(forceRefresh = false) {
  const operation: Operation = { kind: 'refresh', sequence: ++refreshOperation };
  const generation = ++refreshGeneration;
  error.value = '';
  loading.value = true;
  previewLoadingIds.value = new Set();
  if (!(await ensurePermission(operation))) {
    if (generation === refreshGeneration) loading.value = false;
    return;
  }
  if (!isCurrentOperation(operation) || generation !== refreshGeneration) return;

  try {
    const available = await api.enumerateCaptureSources();
    if (generation !== refreshGeneration) return;
    sources.value = available;
    refreshPreviews(available, generation, forceRefresh);

    if (selectedSource.value) {
      const refreshedSelection = available.find(
        (source) => selectedSource.value && sourceKey(source) === sourceKey(selectedSource.value),
      );
      if (refreshedSelection) selectedSource.value = refreshedSelection;
      else error.value = t('source.errorSourceGone');
      return;
    }

    let restoredSource: CaptureSourceInfo | undefined;
    try {
      const currentTarget = await api.getCaptureTarget();
      if (currentTarget) {
        restoredSource = available.find((source) =>
          source.kind === currentTarget.kind
          && (currentTarget.sourceId
            ? source.sourceId === currentTarget.sourceId
            : source.id === `${currentTarget.kind}:${currentTarget.id}`),
        );
        if (restoredSource) {
          selectedSource.value = restoredSource;
          quality.value = qualityName(currentTarget.quality);
        }
      }
    } catch {
      // Older test harnesses and non-Tauri previews may not expose this IPC.
    }
    const defaultSource = restoredSource
      ?? available.find((source) => source.kind === 'screen' && source.isPrimary)
      ?? available.find((source) => source.kind === 'screen')
      ?? available[0];
    if (defaultSource && !restoredSource) await selectSource(defaultSource);
  } catch {
    if (generation === refreshGeneration) error.value = t('source.errorEnumerate');
  } finally {
    if (generation === refreshGeneration) loading.value = false;
  }
}

async function changeQuality(event: Event) {
  const nextQuality = (event.target as HTMLSelectElement).value as Quality;
  if (!selectedSource.value || !(await selectSource(selectedSource.value, nextQuality))) {
    (event.target as HTMLSelectElement).value = quality.value;
  }
}

onMounted(() => {
  void refreshSources();
  window.addEventListener('keydown', onKeydown);
});

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && pickerOpen.value) closePicker();
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown);
});
</script>

<style scoped>
.source-picker { position: relative; display: flex; flex-direction: column; gap: var(--sp-3); padding: var(--sp-4); background: var(--surface); border: var(--line); border-radius: var(--radius-lg); }
.sp-head, .sp-quality, .sp-current, .sp-current-meta, .sp-source-meta, .sp-dialog-head, .sp-list-toolbar { display: flex; align-items: center; }
.sp-head, .sp-quality { justify-content: space-between; gap: var(--sp-3); }
.sp-eyebrow, .sp-current-label, .sp-dialog-kicker { color: var(--muted); font-size: var(--fs-12); letter-spacing: .12em; text-transform: uppercase; }
.sp-head-note { margin: 3px 0 0; color: var(--muted); font-size: var(--fs-12); }
.sp-change, .sp-refresh { border: 0; cursor: pointer; font-size: var(--fs-13); }
.sp-change { color: var(--accent); font-weight: 600; }
.sp-change:hover, .sp-refresh:hover { color: var(--text-strong); }
.sp-change-current { margin-left: auto; padding: var(--sp-2) var(--sp-3); border: 1px solid var(--accent-line); border-radius: var(--radius-sm); background: var(--accent-dim); }
.sp-refresh { padding: 0; color: var(--accent); background: transparent; }
.sp-refresh:disabled { color: var(--muted); cursor: wait; }
.sp-current { gap: var(--sp-3); min-height: 88px; padding: var(--sp-2); border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-2); }
.sp-current-preview { display: grid; flex: 0 0 112px; aspect-ratio: 16 / 10; overflow: hidden; place-items: center; border: 1px solid var(--border); border-radius: 4px; background: var(--bg); }
.sp-current-preview img, .sp-thumb img { display: block; width: 100%; height: 100%; object-fit: cover; }
.sp-current-copy, .sp-source-copy, .sp-type-copy { display: flex; flex-direction: column; }
.sp-current-copy { min-width: 0; gap: 3px; }
.sp-current-name { overflow: hidden; color: var(--text-strong); font-size: var(--fs-15); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.sp-current-meta, .sp-source-meta { flex-wrap: wrap; gap: var(--sp-2); color: var(--muted); font-size: var(--fs-12); }
.sp-preview-fallback, .sp-loading, .sp-empty { color: var(--muted); font-size: var(--fs-12); text-align: center; }
.sp-quality { padding-top: var(--sp-3); border-top: var(--line); color: var(--muted); font-size: var(--fs-12); }
.sp-quality select { min-width: 150px; padding: 8px 10px; color: var(--text); background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--radius-sm); }
.sp-quality select:disabled { color: var(--muted); cursor: not-allowed; }
.sp-error { margin: 0; padding: var(--sp-2) var(--sp-3); color: var(--danger); font-size: var(--fs-13); background: var(--danger-dim); border-left: 2px solid var(--danger); }
.sp-modal { position: fixed; z-index: 20; inset: 0; display: grid; padding: var(--sp-6); place-items: center; background: rgb(10 14 20 / 42%); backdrop-filter: blur(7px); }
.sp-dialog { width: min(560px, 100%); max-height: min(720px, 90vh); overflow: auto; padding: var(--sp-6); border: 1px solid rgb(255 255 255 / 48%); border-radius: 22px; background: color-mix(in srgb, var(--surface) 94%, white); box-shadow: 0 28px 80px rgb(0 0 0 / 24%), 0 4px 18px rgb(0 0 0 / 12%); animation: sp-dialog-in 180ms ease-out both; }
@keyframes sp-dialog-in { from { opacity: 0; transform: translateY(10px) scale(.98); } to { opacity: 1; transform: translateY(0) scale(1); } }
.sp-dialog-head { justify-content: space-between; gap: var(--sp-3); padding-bottom: var(--sp-5); border-bottom: var(--line); }
.sp-dialog-head > div { flex: 1; }
.sp-dialog h2 { margin: 3px 0 0; color: var(--text-strong); font-family: var(--font-display); font-size: var(--fs-24); font-weight: 500; }
.sp-icon-button { display: grid; width: 34px; height: 34px; flex: 0 0 34px; place-items: center; color: var(--text-strong); font-size: 26px; line-height: 1; border: 0; border-radius: 50%; background: var(--surface-3); cursor: pointer; }
.sp-icon-button:hover { background: var(--border); }
.sp-type-grid { display: grid; gap: var(--sp-3); padding-top: var(--sp-5); }
.sp-type-card { display: grid; grid-template-columns: 46px minmax(0, 1fr) 20px; align-items: center; gap: var(--sp-3); width: 100%; min-height: 78px; padding: var(--sp-3) var(--sp-4); text-align: left; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-2); cursor: pointer; transition: border-color var(--motion) ease, background var(--motion) ease, transform var(--motion) ease; }
.sp-type-card:hover { border-color: var(--accent-line); background: var(--accent-dim); transform: translateY(-1px); }
.sp-type-icon { position: relative; display: block; width: 42px; height: 42px; border-radius: 13px; background: var(--accent-dim); }
.sp-icon-screen::before, .sp-icon-display::before, .sp-icon-window::before { position: absolute; content: ''; border: 2px solid var(--accent); border-radius: 3px; }
.sp-icon-screen::before, .sp-icon-display::before { top: 10px; left: 8px; width: 26px; height: 18px; }
.sp-icon-screen::after, .sp-icon-display::after { position: absolute; bottom: 9px; left: 15px; width: 12px; height: 2px; content: ''; background: var(--accent); }
.sp-icon-window::before { top: 10px; left: 9px; width: 24px; height: 22px; border-radius: 4px; box-shadow: -5px 5px 0 -1px var(--accent-dim), -5px 5px 0 1px var(--accent); }
.sp-type-copy { gap: 4px; }
.sp-type-copy strong { color: var(--text-strong); font-size: var(--fs-15); }
.sp-type-copy small { color: var(--muted); font-size: var(--fs-12); }
.sp-chevron, .sp-source-check { color: var(--muted); font-size: 24px; text-align: right; }
.sp-list-toolbar { justify-content: space-between; gap: var(--sp-3); padding: var(--sp-4) 0 var(--sp-3); }
.sp-list-toolbar p { margin: 0; color: var(--muted); font-size: var(--fs-13); }
.sp-source-list { display: flex; flex-direction: column; gap: var(--sp-2); }
.sp-source { display: grid; grid-template-columns: 58px minmax(0, 1fr) 20px; align-items: center; gap: var(--sp-3); width: 100%; min-height: 68px; padding: var(--sp-2); text-align: left; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface-2); cursor: pointer; transition: border-color var(--motion) ease, background var(--motion) ease; }
.sp-source:hover, .sp-source.selected { border-color: var(--accent-line); background: var(--accent-dim); }
.sp-thumb { display: grid; width: 58px; height: 42px; overflow: hidden; place-items: center; border: 1px solid var(--border); border-radius: 4px; background: var(--bg); }
.sp-thumb-fallback { color: var(--accent); }
.sp-source-copy { min-width: 0; gap: 2px; }
.sp-source-name { overflow: hidden; color: var(--text-strong); font-size: var(--fs-14); font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.sp-primary { color: var(--accent); }
@media (max-width: 560px) { .sp-modal { padding: var(--sp-3); } .sp-dialog { padding: var(--sp-4); border-radius: 18px; } .sp-current-preview { flex-basis: 88px; } .sp-change-current { padding-inline: var(--sp-2); } }
</style>
