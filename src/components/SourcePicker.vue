<template>
  <section class="source-picker" aria-labelledby="source-picker-title">
    <header class="sp-head">
      <span id="source-picker-title" class="sp-eyebrow">{{ t('source.label') }}</span>
      <button
        class="sp-refresh"
        type="button"
        :disabled="loading || previewLoadingIds.size > 0"
        @click="refreshSources(true)"
      >
        {{ t('source.refresh') }}
      </button>
    </header>

    <p v-if="error" class="sp-error" role="alert">{{ error }}</p>

    <div v-if="loading" class="sp-loading" aria-live="polite">{{ t('source.loading') }}</div>
    <div v-else class="sp-content">
      <div class="sp-groups">
        <template v-if="sourceGroups.length">
          <section v-for="group in sourceGroups" :key="group.key" class="sp-group">
            <h2 class="sp-group-title">{{ t(group.label) }}</h2>
            <div class="sp-source-list">
              <button
                v-for="item in group.items"
                :key="sourceKey(item)"
                class="sp-source"
                :class="{ selected: sourceKey(item) === selectedSourceKey }"
                type="button"
                :data-source-id="item.sourceId"
                :data-source-key="sourceKey(item)"
                :aria-pressed="sourceKey(item) === selectedSourceKey"
                @click="selectSource(item)"
              >
                <span v-if="item.preview" class="sp-thumb" aria-hidden="true">
                  <img :src="item.preview" alt="" />
                </span>
                <span v-else-if="isPreviewLoading(item)" class="sp-no-preview">
                  {{ t('source.previewLoading') }}
                </span>
                <span v-else class="sp-no-preview">{{ t('source.noPreview') }}</span>
                <span class="sp-source-copy">
                  <span class="sp-source-name">{{ item.name }}</span>
                  <span class="sp-source-meta">
                    <span v-if="item.isPrimary" class="sp-primary">{{ t('source.primary') }}</span>
                    <span>{{ resolution(item) }}</span>
                  </span>
                </span>
              </button>
            </div>
          </section>
        </template>
        <div v-else class="sp-empty">{{ t('source.noSources') }}</div>
      </div>

      <aside v-if="selectedSource" class="sp-preview" data-testid="source-preview" aria-live="polite">
        <div class="sp-preview-frame">
          <img v-if="selectedSource?.preview" :src="selectedSource.preview" alt="" />
          <span v-else-if="isPreviewLoading(selectedSource)" class="sp-preview-fallback">
            {{ t('source.previewLoading') }}
          </span>
          <span v-else class="sp-preview-fallback">{{ t('source.noPreview') }}</span>
        </div>
        <div v-if="selectedSource" class="sp-preview-copy">
          <span class="sp-preview-name">{{ selectedSource.name }}</span>
          <span class="sp-preview-meta">
            <span v-if="selectedSource.isPrimary">{{ t('source.primary') }}</span>
            <span>{{ resolution(selectedSource) }}</span>
          </span>
        </div>
      </aside>
    </div>

    <label class="sp-quality">
      <span class="sp-quality-label">{{ t('source.quality') }}</span>
      <select :value="quality" :disabled="!selectedSource" @change="changeQuality">
        <option value="balanced">{{ t('source.qualityBalanced') }}</option>
        <option value="high">{{ t('source.qualityHigh') }}</option>
        <option value="ultra">{{ t('source.qualityUltra') }}</option>
      </select>
    </label>
  </section>
</template>

<script setup lang="ts">
import { computed, inject, onMounted, ref } from 'vue';
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
let captureOperation = 0;
let refreshOperation = 0;
let refreshGeneration = 0;

const permissionModal = inject<ProvidedPermissionModal>(
  PermissionModalKey,
  ref(null) as ProvidedPermissionModal,
);

const sourceGroups = computed(() => [
  {
    key: 'primary',
    label: 'source.entireScreen',
    items: sources.value.filter((source) => source.kind === 'screen' && source.isPrimary),
  },
  {
    key: 'window',
    label: 'source.windowsAndApps',
    items: sources.value.filter((source) => source.kind === 'window'),
  },
  {
    key: 'extended',
    label: 'source.extendedDisplays',
    items: sources.value.filter((source) => source.kind === 'screen' && !source.isPrimary),
  },
].filter((group) => group.items.length > 0));

function qualityValue(value: Quality) {
  return { balanced: 0.5, high: 0.75, ultra: 1.0 }[value];
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

    const defaultSource = available.find((source) => source.kind === 'screen' && source.isPrimary)
      ?? available.find((source) => source.kind === 'screen')
      ?? available[0];
    if (defaultSource) await selectSource(defaultSource);
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
});
</script>

<style scoped>
.source-picker {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  padding: var(--sp-5);
  background: var(--surface);
  border: var(--line);
  border-radius: var(--radius-lg);
}

.sp-head,
.sp-quality,
.sp-preview-copy,
.sp-source-meta,
.sp-preview-meta {
  display: flex;
  align-items: center;
}

.sp-head,
.sp-quality {
  justify-content: space-between;
  gap: var(--sp-3);
}

.sp-eyebrow,
.sp-group-title {
  font-size: var(--fs-12);
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: var(--muted);
}

.sp-group-title {
  margin: 0 0 var(--sp-2);
  font-family: var(--font-body);
  font-weight: 600;
}

.sp-refresh {
  color: var(--accent);
  font-size: var(--fs-13);
}

.sp-refresh:disabled {
  color: var(--muted);
  cursor: wait;
}

.sp-content {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(180px, 0.65fr);
  gap: var(--sp-5);
}

.sp-groups,
.sp-group,
.sp-source-list,
.sp-preview-copy {
  display: flex;
  flex-direction: column;
}

.sp-groups {
  gap: var(--sp-4);
  min-width: 0;
}

.sp-source-list {
  gap: var(--sp-2);
}

.sp-source {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  align-items: center;
  gap: var(--sp-3);
  width: 100%;
  min-height: 62px;
  padding: var(--sp-2);
  text-align: left;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  transition: border-color var(--motion) ease, background var(--motion) ease;
}

.sp-source:hover {
  border-color: var(--border-strong);
  background: var(--surface-3);
}

.sp-source.selected {
  border-color: var(--accent-line);
  background: var(--accent-dim);
}

.sp-thumb {
  display: grid;
  width: 48px;
  height: 36px;
  overflow: hidden;
  place-items: center;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg);
}

.sp-thumb img,
.sp-preview-frame img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.sp-no-preview {
  display: grid;
  min-height: 36px;
  padding: var(--sp-1);
  place-items: center;
  color: var(--muted);
  font-size: 0.625rem;
  line-height: 1.2;
  text-align: center;
}

.sp-source-copy,
.sp-source-meta {
  min-width: 0;
}

.sp-source-copy {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sp-source-name,
.sp-preview-name {
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--fs-14);
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sp-source-meta,
.sp-preview-meta {
  flex-wrap: wrap;
  gap: var(--sp-2);
  color: var(--muted);
  font-size: var(--fs-12);
}

.sp-primary {
  color: var(--accent);
}

.sp-preview {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: var(--sp-3);
  min-width: 0;
  padding-left: var(--sp-5);
  border-left: var(--line);
}

.sp-preview-frame {
  display: grid;
  aspect-ratio: 16 / 10;
  overflow: hidden;
  place-items: center;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.sp-preview-fallback,
.sp-loading,
.sp-empty {
  color: var(--muted);
  font-size: var(--fs-13);
}

.sp-preview-copy {
  gap: var(--sp-1);
}

.sp-quality {
  padding-top: var(--sp-3);
  border-top: var(--line);
  color: var(--muted);
  font-size: var(--fs-12);
}

.sp-quality select {
  min-width: 150px;
  padding: 8px 10px;
  color: var(--text);
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.sp-quality select:disabled {
  color: var(--muted);
  cursor: not-allowed;
}

.sp-error {
  margin: 0;
  padding: var(--sp-2) var(--sp-3);
  color: var(--danger);
  font-size: var(--fs-13);
  background: var(--danger-dim);
  border-left: 2px solid var(--danger);
}

@media (max-width: 560px) {
  .sp-content {
    grid-template-columns: 1fr;
  }

  .sp-preview {
    padding-top: var(--sp-4);
    padding-left: 0;
    border-top: var(--line);
    border-left: 0;
  }

  .sp-preview-frame {
    max-height: 180px;
  }
}
</style>
