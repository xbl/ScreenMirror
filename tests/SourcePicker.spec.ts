import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import SourcePicker from '../src/components/SourcePicker.vue';

const apiMocks = vi.hoisted(() => ({
  checkScreenRecordingPermission: vi.fn(),
  enumerateCaptureSources: vi.fn(),
  getCaptureSourcePreview: vi.fn(),
  setCaptureTarget: vi.fn(),
}));

vi.mock('../src/utils/api', () => ({ api: apiMocks }));

const sources = [
  { id: 'screen:0', sourceId: 'main-display', name: 'Built-in Retina Display', kind: 'screen' as const, isPrimary: true, preview: null, width: 2560, height: 1600 },
  { id: 'screen:1', sourceId: 'desk-display', name: 'Studio Display', kind: 'screen' as const, isPrimary: false, preview: null, width: 1920, height: 1080 },
  { id: 'window:0', sourceId: 'terminal-window', name: 'Terminal', kind: 'window' as const, isPrimary: false, preview: null, width: 1440, height: 900 },
];

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: { en: { source: {
    label: 'Sharing source', current: 'Currently sharing', change: 'Change', chooseHint: 'Choose only when switching.', chooseType: 'Choose what to share', chooseWindow: 'Choose a window or app', chooseDisplay: 'Choose a display', entireScreenDescription: 'Share the primary display', windowsDescription: 'Share one running window or app', extendedDescription: 'Use an extended display', runningWindows: 'Running windows and apps', availableDisplays: 'Available displays', back: 'Back', close: 'Close', entireScreen: 'Entire screen', windowsAndApps: 'Window or app', extendedDisplays: 'Extended displays', primary: 'Primary display', resolution: '{width} x {height}', refresh: 'Refresh sources', loading: 'Loading...', noSources: 'No sources', previewLoading: 'Loading preview...', noPreview: 'No preview available', quality: 'Quality', qualityBalanced: 'Balanced', qualityHigh: 'High', qualityUltra: 'Ultra', errorPermission: 'Permission required', errorEnumerate: 'Could not load sources', errorSwitch: 'Could not switch', errorSourceGone: 'Source gone',
  } } },
});

function mountPicker() {
  return mount(SourcePicker, { global: { plugins: [i18n] } });
}

async function openPicker(wrapper: ReturnType<typeof mountPicker>) {
  await wrapper.get('.sp-change-head').trigger('click');
  await flushPromises();
}

async function openSourceList(wrapper: ReturnType<typeof mountPicker>, type: 'window' | 'extended') {
  await openPicker(wrapper);
  await wrapper.get(`[data-source-type="${type}"]`).trigger('click');
  await flushPromises();
}

describe('SourcePicker', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    apiMocks.checkScreenRecordingPermission.mockResolvedValue(true);
    apiMocks.enumerateCaptureSources.mockResolvedValue(sources);
    apiMocks.getCaptureSourcePreview.mockResolvedValue('data:image/jpeg;base64,preview');
    apiMocks.setCaptureTarget.mockResolvedValue(undefined);
  });

  it('keeps the main card compact and shows only the current source', async () => {
    const wrapper = mountPicker();
    await flushPromises();

    expect(wrapper.get('.sp-current-name').text()).toBe('Built-in Retina Display');
    expect(wrapper.find('[data-source-id="desk-display"]').exists()).toBe(false);
    expect(wrapper.find('.sp-type-card').exists()).toBe(false);
  });

  it('opens an AirPlay-style source type chooser', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    await openPicker(wrapper);

    expect(wrapper.get('.sp-dialog').text()).toContain('Choose what to share');
    expect(wrapper.findAll('.sp-type-card')).toHaveLength(3);
  });

  it('selects the primary display from the entire-screen action', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.setCaptureTarget.mockClear();
    await openPicker(wrapper);
    await wrapper.get('[data-source-type="screen"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({ kind: 'screen', id: 0, sourceId: 'main-display', quality: 0.75 });
    expect(wrapper.find('.sp-dialog').exists()).toBe(false);
  });

  it('opens the running-window list only after choosing window or app', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    await openSourceList(wrapper, 'window');

    expect(wrapper.get('#sp-dialog-title').text()).toBe('Choose a window or app');
    expect(wrapper.get('[data-source-id="terminal-window"]').text()).toContain('Terminal');
    expect(wrapper.find('[data-source-id="desk-display"]').exists()).toBe(false);
  });

  it('switches to an extended display from the second-level list', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    await openSourceList(wrapper, 'extended');
    await wrapper.get('[data-source-id="desk-display"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenLastCalledWith({ kind: 'screen', id: 1, sourceId: 'desk-display', quality: 0.75 });
    expect(wrapper.get('.sp-current-name').text()).toBe('Studio Display');
    expect(wrapper.find('.sp-dialog').exists()).toBe(false);
  });

  it('sends a stable window source ID without confusing colliding display IDs', async () => {
    const collidingWindow = { ...sources[2], sourceId: 'main-display', name: 'Terminal App' };
    apiMocks.enumerateCaptureSources.mockResolvedValue([...sources, collidingWindow]);
    const wrapper = mountPicker();
    await flushPromises();
    await openSourceList(wrapper, 'window');
    await wrapper.get('[data-source-key="window:main-display"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenLastCalledWith({ kind: 'window', id: 0, sourceId: 'main-display', quality: 0.75 });
  });

  it('does not wait for display previews before selecting the default source', async () => {
    let resolvePreview!: (value: string | null) => void;
    apiMocks.getCaptureSourcePreview.mockImplementation((sourceId: string) => sourceId === 'main-display'
      ? new Promise((resolve) => { resolvePreview = resolve; })
      : Promise.resolve(null));
    const wrapper = mountPicker();
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({ kind: 'screen', id: 0, sourceId: 'main-display', quality: 0.75 });
    expect(wrapper.get('.sp-current').text()).toContain('Loading preview...');
    resolvePreview('data:image/jpeg;base64,done');
    await flushPromises();
    expect(wrapper.find('.sp-current-preview img').attributes('src')).toBe('data:image/jpeg;base64,done');
  });

  it('refreshes the list from inside the second-level chooser', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    await openSourceList(wrapper, 'extended');
    apiMocks.enumerateCaptureSources.mockResolvedValueOnce([sources[0], { ...sources[1], name: 'Refreshed Display' }]);
    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.text()).toContain('Refreshed Display');
  });
});
