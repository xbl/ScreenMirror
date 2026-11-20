<template>
  <section class="source-picker" :class="{ 'source-picker-standalone': standalone }" aria-labelledby="source-picker-title" @mousedown="handleWindowMouseDown">
    <template v-if="standalone">
      <header class="sp-standalone-head">
        <button v-if="pickerStep === 'windows'" class="sp-back" type="button" :aria-label="t('source.back')" @mousedown.stop @click="pickerStep = 'types'">
          <span aria-hidden="true">&#8249;</span>
          {{ t('source.back') }}
        </button>
        <span v-else class="sp-head-spacer" aria-hidden="true"></span>
        <h1 id="source-picker-title">{{ pickerStep === 'types' ? t('source.chooseType') : t('source.chooseWindow') }}</h1>
      </header>

      <div v-if="error" class="sp-error" role="alert">
        <span>{{ error }}</span>
        <button v-if="error === t('source.errorPermission')" class="sp-perm-btn" type="button" @click="handleOpenPermission">
          {{ t('permission.grantAction') }}
        </button>
      </div>
      <div v-if="loading && !sources.length" class="sp-loading" aria-live="polite">{{ t('source.loading') }}</div>

      <template v-else-if="pickerStep === 'types'">
        <div class="sp-type-grid">
          <button
            v-if="primaryScreen"
            class="sp-type-card"
            :class="{ selected: sourceKey(primaryScreen) === selectedSourceKey }"
            type="button"
            data-source-type="screen"
            @click="selectAndClose(primaryScreen)"
          >
            <span class="sp-type-visual sp-type-screen">
              <img v-if="primaryScreen.preview" :src="primaryScreen.preview" alt="" />
              <span v-else class="sp-preview-fallback">{{ t('source.previewLoading') }}</span>
            </span>
            <span class="sp-type-copy">
              <strong>{{ t('source.entireScreen') }}</strong>
              <small>{{ t('source.entireScreenDescription') }}</small>
            </span>
          </button>

          <button class="sp-type-card" type="button" data-source-type="window" @click="pickerStep = 'windows'">
            <span class="sp-type-visual sp-type-window" aria-hidden="true">
              <span class="sp-window-backdrop"></span>
              <span class="sp-window-front"></span>
            </span>
            <span class="sp-type-copy">
              <strong>{{ t('source.windowsAndApps') }}</strong>
              <small>{{ t('source.windowsDescription') }}</small>
            </span>
          </button>

          <div
            v-for="screen in extendedScreens"
            :key="sourceKey(screen)"
            class="sp-extended-card-wrap"
          >
            <button
              class="sp-type-card"
              :class="{ selected: sourceKey(screen) === selectedSourceKey }"
              type="button"
              data-source-type="extended"
              @click="selectAndClose(screen)"
            >
              <span class="sp-type-visual sp-type-extended">
                <img v-if="screen.preview" :src="screen.preview" alt="" />
                <span v-else class="sp-preview-fallback">{{ t('source.previewLoading') }}</span>
              </span>
              <span class="sp-type-copy">
                <strong>
                  {{ screen.isVirtual ? t('source.virtualDisplayBadge') : t('source.extendedDisplays') }}
                  <span v-if="screen.isVirtual" class="sp-virtual-badge">{{ t('source.virtualDisplayBadge') }}</span>
                </strong>
                <small>{{ screen.name }} · {{ t('source.extendedDescription') }}</small>
              </span>
            </button>
            <button
              v-if="screen.isVirtual"
              class="sp-virtual-card-delete"
              type="button"
              :title="t('source.destroyVirtualDisplay')"
              @click.stop="handleDestroyVirtual(screen)"
            >
              &#10005;
            </button>
          </div>
        </div>

        <div class="sp-virtual-section">
          <button
            class="sp-create-virtual-btn"
            type="button"
            data-source-type="virtual"
            :disabled="creatingVirtual"
            @click="handleCreateVirtualDisplay"
          >
            <span class="sp-virtual-plus" aria-hidden="true">+</span>
            <span>{{ creatingVirtual ? t('source.loading') : t('source.createVirtualDisplay') }}</span>
          </button>
          <button
            v-if="hasVirtualDisplay"
            class="sp-destroy-all-virtual-btn"
            type="button"
            @click="handleDestroyVirtualDisplays"
          >
            {{ t('source.destroyAllVirtualDisplays') }}
          </button>
        </div>
        <p v-if="!primaryScreen && !extendedScreens.length && !orderedWindows.length" class="sp-empty">{{ t('source.noSources') }}</p>
      </template>

      <template v-else>
        <div class="sp-list-toolbar">
          <span>{{ t('source.runningWindows') }}</span>
          <button class="sp-refresh" type="button" :disabled="loading" @click="refreshSources(true)">{{ t('source.refresh') }}</button>
        </div>
        <div v-if="orderedWindows.length" class="sp-window-list">
          <button
            v-for="item in orderedWindows"
            :key="sourceKey(item)"
            class="sp-window-card"
            :class="{ selected: sourceKey(item) === selectedSourceKey }"
            type="button"
            :data-source-id="item.sourceId"
            :data-source-key="sourceKey(item)"
            :aria-pressed="sourceKey(item) === selectedSourceKey"
            @click="selectAndClose(item)"
          >
            <span class="sp-window-thumb" aria-hidden="true">
              <img v-if="item.preview" :src="item.preview" alt="" class="sp-window-thumb-img" />
              <span v-else class="sp-window-icon"></span>
            </span>
            <span class="sp-window-copy"><strong>{{ item.name }}</strong><small>{{ resolution(item) }}</small></span>
            <span v-if="sourceKey(item) === selectedSourceKey" class="sp-check" aria-hidden="true">&#10003;</span>
          </button>
        </div>
        <p v-else class="sp-empty">{{ t('source.noSources') }}</p>
      </template>

      <footer class="sp-footer">
        <button class="sp-cancel" type="button" @click="closePicker">{{ t('source.cancel') }}</button>
      </footer>
    </template>

    <template v-else>
      <header class="sp-head">
        <span id="source-picker-title" class="sp-eyebrow">{{ t('source.label') }}</span>
        <button
          v-if="hasVirtualDisplay"
          class="sp-destroy-virtual-tray-btn"
          type="button"
          @click="handleDestroyVirtualDisplays"
        >
          {{ t('source.destroyVirtualDisplay') }}
        </button>
        <button
          v-else
          class="sp-create-virtual-tray-btn"
          type="button"
          :disabled="creatingVirtual"
          @click="handleCreateVirtualDisplay"
        >
          <span class="sp-plus-icon">+</span>
          {{ creatingVirtual ? t('source.loading') : t('source.createVirtualDisplay') }}
        </button>
      </header>
      <div v-if="error" class="sp-error" role="alert">
        <span>{{ error }}</span>
        <button v-if="error === t('source.errorPermission')" class="sp-perm-btn" type="button" @click="handleOpenPermission">
          {{ t('permission.grantAction') }}
        </button>
      </div>
      <div v-if="loading && !selectedSource" class="sp-loading" aria-live="polite">{{ t('source.loading') }}</div>
      <div v-else class="sp-current" data-testid="source-preview" aria-live="polite">
        <div class="sp-current-preview">
          <img
            v-if="selectedSource?.preview"
            :src="selectedSource.preview"
            alt=""
            @error="onPreviewImageError"
          />
          <span v-else-if="previewLoading" class="sp-preview-status">{{ t('source.previewLoading') }}</span>
          <span v-else-if="selectedSource?.kind === 'window'" class="sp-preview-window-fallback" aria-hidden="true">
            <span class="sp-window-icon"></span>
          </span>
          <span v-else class="sp-preview-status">{{ t('source.noPreview') }}</span>
        </div>
        <div class="sp-current-copy">
          <span class="sp-current-label">
            {{ t('source.current') }}
            <span v-if="selectedSource?.isVirtual" class="sp-virtual-badge">{{ t('source.virtualDisplayBadge') }}</span>
          </span>
          <strong>{{ selectedSource?.name || t('source.noSources') }}</strong>
          <small v-if="selectedSource">{{ resolution(selectedSource) }}</small>
        </div>
        <div class="sp-current-actions">
          <button v-if="selectedSource?.isVirtual" class="sp-destroy-virtual" type="button" :title="t('source.destroyVirtualDisplay')" @click="handleDestroySelectedVirtual">&#10005;</button>
          <button class="sp-change" type="button" @click="openPicker">{{ t('source.change') }}</button>
        </div>
      </div>
      <label class="sp-quality"><span>{{ t('source.quality') }}</span><select :value="quality" :disabled="!selectedSource" @change="changeQuality"><option value="balanced">{{ t('source.qualityBalanced') }}</option><option value="high">{{ t('source.qualityHigh') }}</option><option value="ultra">{{ t('source.qualityUltra') }}</option></select></label>
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useI18n } from 'vue-i18n';
import { api, type CaptureSourceInfo, type CaptureTarget, type CaptureTargetState } from '../utils/api';

type Quality = 'balanced' | 'high' | 'ultra';
type PickerStep = 'types' | 'windows';

const props = defineProps<{ externalChooser?: boolean; standalone?: boolean }>();
const { t } = useI18n();
const sources = ref<CaptureSourceInfo[]>([]);
const selectedSource = ref<CaptureSourceInfo | null>(null);
const quality = ref<Quality>('high');
const error = ref('');
const loading = ref(false);
const previewLoading = ref(false);
const pickerStep = ref<PickerStep>('types');
let captureOperation = 0;
let refreshGeneration = 0;
let unlistenTrayOpened: UnlistenFn | undefined;
let unlistenPickerOpened: UnlistenFn | undefined;
let unlistenTargetChanged: UnlistenFn | undefined;
let targetSyncTimer: number | undefined;

const orderedKeys = ref<string[]>([]);
const ordered = computed(() => {
  const byKey = new Map(sources.value.map((source) => [sourceKey(source), source]));
  const keys = [...orderedKeys.value, ...sources.value.map(sourceKey).filter((key) => !orderedKeys.value.includes(key))];
  return keys.map((key) => byKey.get(key)).filter((source): source is CaptureSourceInfo => Boolean(source));
});
const orderedScreens = computed(() => ordered.value.filter((source) => source.kind === 'screen'));
const primaryScreen = computed(() => orderedScreens.value.find((source) => source.isPrimary) ?? orderedScreens.value[0] ?? null);
const extendedScreens = computed(() => orderedScreens.value.filter((source) => sourceKey(source) !== (primaryScreen.value ? sourceKey(primaryScreen.value) : '')));
const orderedWindows = computed(() => ordered.value.filter((source) => source.kind === 'window'));
const virtualScreens = computed(() => sources.value.filter((source) => source.isVirtual));
const hasVirtualDisplay = computed(() => virtualScreens.value.length > 0);
const selectedSourceKey = computed(() => selectedSource.value ? sourceKey(selectedSource.value) : null);

function sourceKey(source: Pick<CaptureSourceInfo, 'kind' | 'sourceId'>) { return `${source.kind}:${source.sourceId}`; }
function resolution(source: CaptureSourceInfo) { return t('source.resolution', { width: source.width, height: source.height }); }
function qualityValue(value: Quality) { return { balanced: 0.5, high: 0.75, ultra: 1.0 }[value]; }
function qualityName(value: number): Quality { return value >= 0.9 ? 'ultra' : value >= 0.65 ? 'high' : 'balanced'; }
function openPicker() { if (props.externalChooser) void api.openSourcePickerWindow(); }
function handleWindowMouseDown(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target;
  if (target instanceof Element && target.closest('button, input, select, textarea, a')) return;
  event.preventDefault();
  void api.startSourcePickerDrag();
}
async function closePicker() {
  if (!props.standalone) return;
  try {
    await api.closeSourcePickerWindow();
  } catch {
    try {
      await getCurrentWindow().close();
    } catch {
      window.close();
    }
  }
}

function captureTarget(source: CaptureSourceInfo, targetQuality = quality.value): CaptureTarget {
  const id = Number.parseInt(source.id.split(':').at(-1) ?? '', 10);
  return { kind: source.kind, id: Number.isNaN(id) ? 0 : id, sourceId: source.sourceId, quality: qualityValue(targetQuality) };
}
async function selectSource(source: CaptureSourceInfo, nextQuality = quality.value) {
  const operation = ++captureOperation;
  error.value = '';
  try {
    await api.setCaptureTarget(captureTarget(source, nextQuality));
    if (operation !== captureOperation) return false;
    selectedSource.value = sources.value.find((item) => sourceKey(item) === sourceKey(source)) ?? source;
    quality.value = nextQuality;
    previewLoading.value = !selectedSource.value.preview;
    return true;
  } catch {
    if (operation === captureOperation) error.value = t('source.errorSwitch');
    return false;
  }
}
async function selectAndClose(source: CaptureSourceInfo) {
  const selected = await selectSource(source);
  if (selected) await closePicker();
}

const creatingVirtual = ref(false);

async function handleCreateVirtualDisplay() {
  if (creatingVirtual.value) return;
  creatingVirtual.value = true;
  error.value = '';
  try {
    const displayId = await api.createVirtualDisplay({
      name: 'ScreenMirror Virtual Display',
      width: 1920,
      height: 1080,
      refreshRate: 60,
      hiDpi: false,
    });
    await refreshSources(true);
    const virtualSource = sources.value.find((s) => s.sourceId === String(displayId));
    if (virtualSource) {
      await selectAndClose(virtualSource);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    creatingVirtual.value = false;
  }
}

async function handleDestroyVirtual(screen: CaptureSourceInfo) {
  const idNum = Number(screen.sourceId);
  // Optimistically remove from local state immediately for snappy user feedback
  sources.value = sources.value.filter((s) => sourceKey(s) !== sourceKey(screen));
  if (selectedSource.value && sourceKey(selectedSource.value) === sourceKey(screen)) {
    selectedSource.value = primaryScreen.value;
  }
  if (!Number.isNaN(idNum)) {
    try {
      await api.destroyVirtualDisplay(idNum);
      await refreshSources(true);
    } catch (e) {
      error.value = String(e);
      await refreshSources(true);
    }
  }
}

async function handleDestroyVirtualDisplays() {
  // Optimistically remove all virtual displays immediately
  sources.value = sources.value.filter((s) => !s.isVirtual);
  if (selectedSource.value?.isVirtual) {
    selectedSource.value = primaryScreen.value;
  }
  try {
    await api.destroyAllVirtualDisplays();
    await refreshSources(true);
  } catch (e) {
    error.value = String(e);
    await refreshSources(true);
  }
}

async function handleDestroySelectedVirtual() {
  if (!selectedSource.value?.isVirtual) return;
  await handleDestroyVirtual(selectedSource.value);
}

function onPreviewImageError() {
  if (selectedSource.value) {
    selectedSource.value = { ...selectedSource.value, preview: null };
    previewLoading.value = false;
  }
}

function refreshPreview(source: CaptureSourceInfo, generation: number, forceRefresh: boolean, attempt = 0) {
  void api.getCaptureSourcePreview(source.sourceId, forceRefresh, source.kind).then((preview) => {
    if (generation !== refreshGeneration) return;
    if (!preview) {
      if (attempt < 3) {
        window.setTimeout(() => refreshPreview(source, generation, true, attempt + 1), 150);
      } else if (selectedSource.value && sourceKey(selectedSource.value) === sourceKey(source)) {
        previewLoading.value = false;
      }
      return;
    }
    sources.value = sources.value.map((item) => sourceKey(item) === sourceKey(source) ? { ...item, preview } : item);
    if (selectedSource.value && sourceKey(selectedSource.value) === sourceKey(source)) {
      selectedSource.value = { ...source, preview };
      previewLoading.value = false;
    }
  }).catch(() => {
    if (generation === refreshGeneration && selectedSource.value && sourceKey(selectedSource.value) === sourceKey(source)) previewLoading.value = false;
  });
}
function refreshPreviews(available: CaptureSourceInfo[], selected: CaptureSourceInfo | null, generation: number, forceRefresh: boolean) {
  const previewSources = available.filter((item) => item.kind === 'screen');
  if (selected?.kind === 'window') previewSources.push(selected);
  if (props.standalone && pickerStep.value === 'windows') {
    for (const w of available.filter((item) => item.kind === 'window')) {
      if (!previewSources.some((s) => sourceKey(s) === sourceKey(w))) {
        previewSources.push(w);
      }
    }
  }
  for (const source of previewSources) {
    refreshPreview(source, generation, forceRefresh);
  }
}
function updateCurrentSource(target: CaptureTargetState) {
  const source = sources.value.find((item) => item.kind === target.kind && item.sourceId === target.sourceId);
  if (!source) {
    void refreshSources(true);
    return;
  }
  selectedSource.value = source;
  quality.value = qualityName(target.quality);
  previewLoading.value = !source.preview;
  const generation = ++refreshGeneration;
  refreshPreview(source, generation, true);
}
async function syncCurrentSource() {
  const target = await api.getCaptureTarget().catch(() => null);
  if (!target || (selectedSource.value?.kind === target.kind && selectedSource.value.sourceId === target.sourceId && quality.value === qualityName(target.quality))) return;
  updateCurrentSource(target);
}
async function refreshSources(forceRefresh = false) {
  const generation = ++refreshGeneration;
  loading.value = true;
  if (!props.standalone) previewLoading.value = true;
  error.value = '';
  try {
    const available = await api.enumerateCaptureSources();
    if (generation !== refreshGeneration) return;
    const existingPreviews = new Map(
      sources.value.filter((s) => s.preview).map((s) => [sourceKey(s), s.preview!])
    );
    if (selectedSource.value?.preview) {
      existingPreviews.set(sourceKey(selectedSource.value), selectedSource.value.preview);
    }
    sources.value = available.map((s) => {
      const prev = existingPreviews.get(sourceKey(s));
      return prev ? { ...s, preview: prev } : s;
    });
    orderedKeys.value = orderedKeys.value.filter((key) => available.some((source) => sourceKey(source) === key));
    const current = await api.getCaptureTarget().catch(() => null);
    const restored = current && sources.value.find((source) => source.kind === current.kind && (current.sourceId ? source.sourceId === current.sourceId : source.id === `${current.kind}:${current.id}`));
    if (restored) {
      selectedSource.value = restored;
      quality.value = qualityName(current.quality);
      if (selectedSource.value.preview) previewLoading.value = false;
    }
    else if (!selectedSource.value && !props.standalone) {
      const primary = available.find((source) => source.kind === 'screen' && source.isPrimary) ?? available.find((source) => source.kind === 'screen') ?? available[0];
      if (primary) await selectSource(primary);
    }
    refreshPreviews(available, selectedSource.value, generation, forceRefresh);
  } catch {
    if (generation === refreshGeneration) {
      const hasPerm = await api.checkScreenRecordingPermission().catch(() => true);
      error.value = hasPerm ? t('source.errorEnumerate') : t('source.errorPermission');
    }
  } finally {
    if (generation === refreshGeneration) loading.value = false;
  }
}

async function handleOpenPermission() {
  await api.requestAndOpenScreenRecordingSettings();
}
async function changeQuality(event: Event) {
  const next = (event.target as HTMLSelectElement).value as Quality;
  if (selectedSource.value) await selectSource(selectedSource.value, next);
}
function onKeydown(event: KeyboardEvent) { if (event.key === 'Escape' && props.standalone) closePicker(); }

async function resizePickerWindow(step: PickerStep) {
  if (!props.standalone) return;
  const pickerWindow = getCurrentWindow();
  if (typeof pickerWindow.setSize !== 'function') return;
  await nextTick();
  const height = step === 'types'
    ? 430
    : Math.min(760, Math.max(620, 200 + orderedWindows.value.length * 82));
  await pickerWindow.setSize(new LogicalSize(720, height));
}

onMounted(() => {
  void refreshSources(props.externalChooser || props.standalone);
  void resizePickerWindow('types');
  window.addEventListener('keydown', onKeydown);
  if (props.externalChooser) {
    void listen('tray-panel-opened', () => refreshSources(true)).then((stop) => {
      unlistenTrayOpened = stop;
    });
    void listen<CaptureTargetState>('capture-target-changed', ({ payload }) => updateCurrentSource(payload)).then((stop) => {
      unlistenTargetChanged = stop;
    });
    targetSyncTimer = window.setInterval(() => { void syncCurrentSource(); }, 1000);
  }
  if (props.standalone) {
    void listen('source-picker-opened', () => refreshSources(true)).then((stop) => {
      unlistenPickerOpened = stop;
    });
  }
});
watch([pickerStep, orderedWindows], ([step, windows]) => {
  void resizePickerWindow(step);
  if (step === 'windows') {
    for (const item of windows) {
      if (!item.preview) {
        refreshPreview(item, refreshGeneration, false);
      }
    }
  }
}, { flush: 'post' });
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown);
  unlistenTrayOpened?.();
  unlistenPickerOpened?.();
  unlistenTargetChanged?.();
  if (targetSyncTimer) window.clearInterval(targetSyncTimer);
});
</script>

<style scoped>
.source-picker { display: flex; flex-direction: column; gap: 12px; padding: 12px; color: var(--text); background: var(--surface); }
.source-picker-standalone { min-width: 0; min-height: 100vh; padding: 20px 28px 16px; overflow-x: hidden; background: var(--surface); }
.sp-standalone-head { display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; min-height: 34px; cursor: grab; user-select: none; }
.sp-standalone-head:active { cursor: grabbing; }
.sp-standalone-head h1 { color: var(--text-strong); font-family: var(--font-body); font-size: clamp(20px, 3vw, 28px); font-weight: 650; text-align: center; }
.sp-head-spacer { min-width: 1px; }
.sp-back { display: inline-flex; align-items: center; gap: 5px; justify-self: start; padding: 5px 8px; color: var(--muted); border-radius: var(--radius-sm); }
.sp-back:hover { color: var(--text-strong); background: var(--surface-2); }
.sp-back span { font-size: 23px; line-height: .7; }
.sp-type-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; margin: 8px 0 auto; }
.sp-type-card { display: flex; flex-direction: column; gap: 12px; min-width: 0; padding: 10px; text-align: center; border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--group); transition: transform var(--motion-fast) ease-out, border-color var(--motion-fast) ease-out, background var(--motion-fast) ease-out; }
.sp-type-card:hover { border-color: var(--accent); background: var(--surface-3); transform: translateY(-2px); }
.sp-type-card.selected { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-dim); }
.sp-type-card:active { transform: translateY(0); }
.sp-type-visual { position: relative; display: grid; aspect-ratio: 1.46; overflow: hidden; place-items: center; border-radius: 10px; background: #2b323c; }
.sp-type-visual img { width: 100%; height: 100%; object-fit: cover; }
.sp-preview-fallback { color: var(--muted); font-size: 11px; }
.sp-type-copy { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.sp-type-copy strong { color: var(--text-strong); font-size: 15px; }
.sp-type-copy small { min-height: 36px; color: var(--muted); font-size: 12px; line-height: 1.45; }
.sp-type-window { background: var(--control); }
.sp-window-backdrop, .sp-window-front { position: absolute; border: 2px solid var(--muted-2); border-radius: 4px; background: var(--muted); }
.sp-window-backdrop { width: 69%; height: 54%; transform: translate(-10%, -10%); opacity: .65; }
.sp-window-front { width: 62%; height: 68%; transform: translate(13%, 11%); background: var(--selected); box-shadow: 0 5px 14px rgb(0 0 0 / 18%); }
.sp-type-extended { background: var(--control); }
.sp-list-toolbar { display: flex; align-items: center; justify-content: space-between; margin-top: 20px; color: var(--muted); font-size: 13px; }
.sp-refresh, .sp-change { padding: 6px 8px; color: var(--accent); font-weight: 600; border-radius: var(--radius-sm); }
.sp-refresh:hover, .sp-change:hover { background: var(--accent-dim); }
.sp-window-list { display: grid; gap: 10px; min-width: 0; margin-top: 8px; }
.sp-window-card { position: relative; display: flex; align-items: center; gap: 12px; min-width: 0; max-width: 100%; min-height: 64px; padding: 9px 12px; overflow: hidden; text-align: left; border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--group); }
.sp-window-card:hover, .sp-window-card.selected { border-color: var(--accent); background: var(--surface-3); }
.sp-window-thumb { position: relative; display: grid; width: 56px; height: 38px; flex: 0 0 56px; place-items: center; border-radius: 6px; overflow: hidden; background: var(--control); border: 1px solid var(--border); }
.sp-window-thumb-img { width: 100%; height: 100%; object-fit: cover; }
.sp-window-icon { width: 26px; height: 19px; border: 2px solid var(--accent); border-radius: 4px; box-shadow: -5px 5px 0 -1px var(--control), -5px 5px 0 1px var(--accent); }
.sp-window-copy { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; overflow: hidden; }
.sp-window-copy strong { overflow: hidden; color: var(--text-strong); text-overflow: ellipsis; white-space: nowrap; }
.sp-window-copy small { color: var(--muted); font-size: 12px; }
.sp-check { color: var(--accent); font-size: 18px; }
.sp-footer { display: flex; justify-content: flex-end; margin-top: 10px; padding-top: 12px; border-top: var(--line); }
.sp-cancel { min-width: 84px; padding: 8px 16px; color: var(--text-strong); border: var(--line); border-radius: var(--radius-md); background: var(--control); }
.sp-cancel:hover { background: var(--accent-dim); }
.sp-head, .sp-current, .sp-quality { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.sp-eyebrow { color: var(--muted); font-size: 11px; letter-spacing: .14em; text-transform: uppercase; }
.sp-create-virtual-tray-btn { display: inline-flex; align-items: center; gap: 4px; padding: 3px 8px; color: var(--accent); font-size: 11px; font-weight: 600; border: 1px dashed var(--accent); border-radius: var(--radius-sm); background: var(--surface-2); cursor: pointer; transition: all var(--motion-fast) ease-out; }
.sp-create-virtual-tray-btn:hover { background: var(--accent-dim); }
.sp-create-virtual-tray-btn:disabled { opacity: 0.6; cursor: not-allowed; }
.sp-destroy-virtual-tray-btn { display: inline-flex; align-items: center; padding: 3px 8px; color: var(--danger); font-size: 11px; font-weight: 600; border: 1px solid var(--danger-dim); border-radius: var(--radius-sm); background: var(--surface-2); cursor: pointer; }
.sp-destroy-virtual-tray-btn:hover { background: var(--danger-dim); }
.sp-plus-icon { font-size: 13px; font-weight: 700; line-height: 1; }
.sp-loading, .sp-empty { padding: 32px 0; color: var(--muted); text-align: center; }
.sp-error { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 10px 12px; color: var(--danger); background: var(--danger-dim); border-radius: var(--radius-sm); font-size: 12px; }
.sp-perm-btn { padding: 4px 8px; font-size: 11px; font-weight: 600; color: #fff; background: var(--warn, #e08b38); border: none; border-radius: var(--radius-sm); cursor: pointer; white-space: nowrap; }
.sp-perm-btn:hover { opacity: .9; }
.sp-current { min-height: 88px; padding: 8px; border: var(--line); border-radius: var(--radius-sm); background: var(--surface-2); }
.sp-current-preview { display: grid; place-items: center; width: 112px; aspect-ratio: 16 / 10; overflow: hidden; border-radius: 6px; background: var(--control); border: 1px solid var(--border); }
.sp-current-preview img { width: 100%; height: 100%; object-fit: cover; }
.sp-preview-status { padding: 4px; color: var(--muted); font-size: 11px; text-align: center; }
.sp-preview-window-fallback { display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; }
.sp-current-copy { display: flex; flex: 1; flex-direction: column; gap: 4px; min-width: 0; }
.sp-current-label { color: var(--muted); font-size: 11px; text-transform: uppercase; }
.sp-current-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sp-current-copy small { color: var(--muted); }
.sp-quality { padding-top: 12px; border-top: var(--line); color: var(--muted); font-size: 13px; }
.sp-quality select { min-width: 150px; padding: 8px 10px; color: var(--text); border: var(--line-strong); border-radius: var(--radius-sm); background: var(--surface-2); }
.sp-extended-card-wrap { position: relative; display: flex; flex-direction: column; }
.sp-extended-card-wrap .sp-type-card { width: 100%; height: 100%; flex: 1; }
.sp-virtual-card-delete { position: absolute; top: 6px; right: 6px; width: 24px; height: 24px; padding: 0; display: flex; align-items: center; justify-content: center; background: rgba(0, 0, 0, 0.65); color: #fff; border: 1px solid rgba(255, 255, 255, 0.25); border-radius: 50%; font-size: 11px; line-height: 1; cursor: pointer; z-index: 2; transition: background var(--motion-fast) ease-out, transform var(--motion-fast) ease-out; }
.sp-virtual-card-delete:hover { background: var(--danger); transform: scale(1.1); }
.sp-virtual-section { display: flex; align-items: center; justify-content: center; gap: 12px; margin: 12px 0 4px; }
.sp-destroy-all-virtual-btn { display: inline-flex; align-items: center; gap: 6px; padding: 8px 16px; color: var(--danger); font-size: 13px; font-weight: 600; border: 1px solid var(--danger-dim); border-radius: var(--radius-md); background: var(--control); cursor: pointer; transition: background var(--motion-fast) ease-out, border-color var(--motion-fast) ease-out; }
.sp-destroy-all-virtual-btn:hover { background: var(--danger-dim); }
.sp-create-virtual-btn { display: inline-flex; align-items: center; gap: 8px; padding: 8px 16px; color: var(--accent); font-size: 13px; font-weight: 600; border: 1px dashed var(--accent); border-radius: var(--radius-md); background: var(--control); cursor: pointer; transition: background var(--motion-fast) ease-out, border-color var(--motion-fast) ease-out; }
.sp-create-virtual-btn:hover { background: var(--accent-dim); }
.sp-create-virtual-btn:disabled { opacity: 0.6; cursor: not-allowed; }
.sp-virtual-plus { font-size: 16px; font-weight: 700; line-height: 1; }
.sp-virtual-badge { display: inline-block; margin-left: 6px; padding: 2px 6px; font-size: 10px; font-weight: 600; color: #fff; background: var(--accent); border-radius: 4px; text-transform: none; vertical-align: middle; }
.sp-current-actions { display: flex; align-items: center; gap: 6px; }
.sp-destroy-virtual { display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; padding: 0; color: var(--danger); font-size: 14px; font-weight: 600; border: 1px solid var(--danger-dim); border-radius: var(--radius-sm); background: transparent; cursor: pointer; }
.sp-destroy-virtual:hover { background: var(--danger-dim); }
@media (max-width: 680px) { .source-picker-standalone { padding: 28px 20px 20px; } .sp-type-grid { grid-template-columns: 1fr; } .sp-type-card { flex-direction: row; align-items: center; text-align: left; } .sp-type-visual { width: 132px; flex: 0 0 132px; } .sp-type-copy small { min-height: 0; } }
</style>
