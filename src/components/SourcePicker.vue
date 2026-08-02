<template>
  <section class="source-picker" :class="{ 'source-picker-standalone': standalone }" aria-labelledby="source-picker-title">
    <template v-if="standalone">
      <header class="sp-standalone-head" data-tauri-drag-region>
        <button v-if="pickerStep === 'windows'" class="sp-back" type="button" :aria-label="t('source.back')" @click="pickerStep = 'types'">
          <span aria-hidden="true">&#8249;</span>
          {{ t('source.back') }}
        </button>
        <span v-else class="sp-head-spacer" aria-hidden="true"></span>
        <h1 id="source-picker-title">{{ pickerStep === 'types' ? t('source.chooseType') : t('source.chooseWindow') }}</h1>
      </header>

      <p v-if="error" class="sp-error" role="alert">{{ error }}</p>
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

          <button
            v-for="screen in extendedScreens"
            :key="sourceKey(screen)"
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
              <strong>{{ t('source.extendedDisplays') }}</strong>
              <small>{{ screen.name }} · {{ t('source.extendedDescription') }}</small>
            </span>
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
            <span class="sp-window-thumb" aria-hidden="true"><span class="sp-window-icon"></span></span>
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
      <header class="sp-head"><span id="source-picker-title" class="sp-eyebrow">{{ t('source.label') }}</span></header>
      <p v-if="error" class="sp-error" role="alert">{{ error }}</p>
      <div v-if="loading && !selectedSource" class="sp-loading" aria-live="polite">{{ t('source.loading') }}</div>
      <div v-else class="sp-current" data-testid="source-preview" aria-live="polite">
        <div class="sp-current-preview"><img v-if="selectedSource?.preview" :src="selectedSource.preview" alt="" /><span v-else>{{ t('source.noPreview') }}</span></div>
        <div class="sp-current-copy"><span class="sp-current-label">{{ t('source.current') }}</span><strong>{{ selectedSource?.name || t('source.noSources') }}</strong><small v-if="selectedSource">{{ resolution(selectedSource) }}</small></div>
        <button class="sp-change" type="button" @click="openPicker">{{ t('source.change') }}</button>
      </div>
      <label class="sp-quality"><span>{{ t('source.quality') }}</span><select :value="quality" :disabled="!selectedSource" @change="changeQuality"><option value="balanced">{{ t('source.qualityBalanced') }}</option><option value="high">{{ t('source.qualityHigh') }}</option><option value="ultra">{{ t('source.qualityUltra') }}</option></select></label>
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useI18n } from 'vue-i18n';
import { api, type CaptureSourceInfo, type CaptureTarget } from '../utils/api';

type Quality = 'balanced' | 'high' | 'ultra';
type PickerStep = 'types' | 'windows';

const props = defineProps<{ externalChooser?: boolean; standalone?: boolean }>();
const { t } = useI18n();
const sources = ref<CaptureSourceInfo[]>([]);
const selectedSource = ref<CaptureSourceInfo | null>(null);
const quality = ref<Quality>('high');
const error = ref('');
const loading = ref(false);
const pickerStep = ref<PickerStep>('types');
let captureOperation = 0;
let refreshGeneration = 0;

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
const selectedSourceKey = computed(() => selectedSource.value ? sourceKey(selectedSource.value) : null);

function sourceKey(source: Pick<CaptureSourceInfo, 'kind' | 'sourceId'>) { return `${source.kind}:${source.sourceId}`; }
function resolution(source: CaptureSourceInfo) { return t('source.resolution', { width: source.width, height: source.height }); }
function qualityValue(value: Quality) { return { balanced: 0.5, high: 0.75, ultra: 1.0 }[value]; }
function qualityName(value: number): Quality { return value >= 0.9 ? 'ultra' : value >= 0.65 ? 'high' : 'balanced'; }
function openPicker() { if (props.externalChooser) void api.openSourcePickerWindow(); }
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
    return true;
  } catch {
    if (operation === captureOperation) error.value = t('source.errorSwitch');
    return false;
  }
}
async function selectAndClose(source: CaptureSourceInfo) {
  const closing = closePicker();
  await selectSource(source);
  await closing;
}

function refreshPreviews(available: CaptureSourceInfo[], generation: number, forceRefresh: boolean) {
  for (const source of available.filter((item) => item.kind === 'screen')) {
    void api.getCaptureSourcePreview(source.sourceId, forceRefresh).then((preview) => {
      if (!preview || generation !== refreshGeneration) return;
      sources.value = sources.value.map((item) => sourceKey(item) === sourceKey(source) ? { ...item, preview } : item);
      if (selectedSource.value && sourceKey(selectedSource.value) === sourceKey(source)) selectedSource.value = { ...source, preview };
    });
  }
}
async function refreshSources(forceRefresh = false) {
  const generation = ++refreshGeneration;
  loading.value = true;
  error.value = '';
  try {
    const available = await api.enumerateCaptureSources();
    if (generation !== refreshGeneration) return;
    sources.value = available;
    orderedKeys.value = orderedKeys.value.filter((key) => available.some((source) => sourceKey(source) === key));
    refreshPreviews(available, generation, forceRefresh);
    const current = await api.getCaptureTarget().catch(() => null);
    const restored = current && available.find((source) => source.kind === current.kind && (current.sourceId ? source.sourceId === current.sourceId : source.id === `${current.kind}:${current.id}`));
    if (restored) { selectedSource.value = restored; quality.value = qualityName(current.quality); }
    else if (!selectedSource.value && !props.standalone) {
      const primary = available.find((source) => source.kind === 'screen' && source.isPrimary) ?? available.find((source) => source.kind === 'screen') ?? available[0];
      if (primary) await selectSource(primary);
    }
  } catch {
    if (generation === refreshGeneration) error.value = t('source.errorEnumerate');
  } finally {
    if (generation === refreshGeneration) loading.value = false;
  }
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

onMounted(() => { void refreshSources(); void resizePickerWindow('types'); window.addEventListener('keydown', onKeydown); });
watch([pickerStep, orderedWindows], ([step]) => { void resizePickerWindow(step); }, { flush: 'post' });
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown));
</script>

<style scoped>
.source-picker { display: flex; flex-direction: column; gap: 16px; padding: 16px; color: var(--text); background: var(--surface); }
.source-picker-standalone { min-height: 100vh; padding: 24px 34px 18px; background: var(--surface); }
.sp-standalone-head { display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; min-height: 34px; }
.sp-standalone-head h1 { color: var(--text-strong); font-family: var(--font-body); font-size: clamp(20px, 3vw, 28px); font-weight: 650; text-align: center; }
.sp-head-spacer { min-width: 1px; }
.sp-back { display: inline-flex; align-items: center; gap: 5px; justify-self: start; padding: 5px 8px; color: var(--muted); border-radius: var(--radius-sm); }
.sp-back:hover { color: var(--text-strong); background: var(--surface-2); }
.sp-back span { font-size: 23px; line-height: .7; }
.sp-type-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; margin: 8px 0 auto; }
.sp-type-card { display: flex; flex-direction: column; gap: 14px; min-width: 0; padding: 12px; text-align: center; border: 1px solid transparent; border-radius: 16px; background: var(--surface-2); transition: transform var(--motion) ease, border-color var(--motion) ease, background var(--motion) ease; }
.sp-type-card:hover { border-color: var(--accent); background: var(--surface-3); transform: translateY(-2px); }
.sp-type-card.selected { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-line); }
.sp-type-card:active { transform: translateY(0); }
.sp-type-visual { position: relative; display: grid; aspect-ratio: 1.46; overflow: hidden; place-items: center; border-radius: 10px; background: #2b323c; }
.sp-type-visual img { width: 100%; height: 100%; object-fit: cover; }
.sp-preview-fallback { color: var(--muted); font-size: 11px; }
.sp-type-copy { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.sp-type-copy strong { color: var(--text-strong); font-size: 15px; }
.sp-type-copy small { min-height: 36px; color: var(--muted); font-size: 12px; line-height: 1.45; }
.sp-type-window { background: linear-gradient(145deg, #313945, #20252d); }
.sp-window-backdrop, .sp-window-front { position: absolute; border: 2px solid #8aa6a1; border-radius: 4px; background: #667980; }
.sp-window-backdrop { width: 69%; height: 54%; transform: translate(-10%, -10%); opacity: .65; }
.sp-window-front { width: 62%; height: 68%; transform: translate(13%, 11%); background: #e7ece9; box-shadow: 0 5px 14px rgb(0 0 0 / 28%); }
.sp-type-extended { background: #202731; }
.sp-type-extended::after { position: absolute; right: 9%; bottom: 8%; width: 34%; height: 9%; border-radius: 50%; background: #7be0d2; content: ''; opacity: .6; }
.sp-list-toolbar { display: flex; align-items: center; justify-content: space-between; margin-top: 20px; color: var(--muted); font-size: 13px; }
.sp-refresh, .sp-change { padding: 6px 8px; color: var(--accent); font-weight: 600; border-radius: var(--radius-sm); }
.sp-refresh:hover, .sp-change:hover { background: var(--accent-dim); }
.sp-window-list { display: grid; gap: 10px; max-height: 500px; margin-top: 8px; overflow: auto; }
.sp-window-card { position: relative; display: flex; align-items: center; gap: 14px; min-height: 72px; padding: 10px 14px; text-align: left; border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--surface-2); }
.sp-window-card:hover, .sp-window-card.selected { border-color: var(--accent); background: var(--surface-3); }
.sp-window-thumb { display: grid; width: 56px; height: 44px; flex: 0 0 56px; place-items: center; border-radius: 7px; background: #2b323c; }
.sp-window-icon { width: 28px; height: 21px; border: 2px solid var(--accent); border-radius: 4px; box-shadow: -5px 5px 0 -1px #2b323c, -5px 5px 0 1px var(--accent); }
.sp-window-copy { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.sp-window-copy strong { overflow: hidden; color: var(--text-strong); text-overflow: ellipsis; white-space: nowrap; }
.sp-window-copy small { color: var(--muted); font-size: 12px; }
.sp-check { color: var(--accent); font-size: 18px; }
.sp-footer { display: flex; justify-content: flex-end; margin-top: 10px; padding-top: 12px; border-top: var(--line); }
.sp-cancel { min-width: 92px; padding: 9px 18px; color: var(--text-strong); border-radius: var(--radius-md); background: var(--surface-3); }
.sp-cancel:hover { background: var(--accent-dim); }
.sp-head, .sp-current, .sp-quality { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.sp-eyebrow { color: var(--muted); font-size: 11px; letter-spacing: .14em; text-transform: uppercase; }
.sp-loading, .sp-empty { padding: 32px 0; color: var(--muted); text-align: center; }
.sp-error { padding: 10px; color: var(--danger); background: var(--danger-dim); }
.sp-current { min-height: 88px; padding: 8px; border: var(--line); border-radius: var(--radius-sm); background: var(--surface-2); }
.sp-current-preview { width: 112px; aspect-ratio: 16 / 10; overflow: hidden; border-radius: 5px; background: var(--bg); }
.sp-current-preview img { width: 100%; height: 100%; object-fit: cover; }
.sp-current-copy { display: flex; flex: 1; flex-direction: column; gap: 4px; min-width: 0; }
.sp-current-label { color: var(--muted); font-size: 11px; text-transform: uppercase; }
.sp-current-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sp-current-copy small { color: var(--muted); }
.sp-quality { padding-top: 12px; border-top: var(--line); color: var(--muted); font-size: 13px; }
.sp-quality select { min-width: 150px; padding: 8px 10px; color: var(--text); border: var(--line-strong); border-radius: var(--radius-sm); background: var(--surface-2); }
@media (max-width: 680px) { .source-picker-standalone { padding: 28px 20px 20px; } .sp-type-grid { grid-template-columns: 1fr; } .sp-type-card { flex-direction: row; align-items: center; text-align: left; } .sp-type-visual { width: 132px; flex: 0 0 132px; } .sp-type-copy small { min-height: 0; } }
</style>
