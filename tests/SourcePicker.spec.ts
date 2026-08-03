import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import SourcePicker from '../src/components/SourcePicker.vue';

const apiMocks = vi.hoisted(() => ({
  enumerateCaptureSources: vi.fn(),
  getCaptureSourcePreview: vi.fn(),
  getCaptureTarget: vi.fn(),
  setCaptureTarget: vi.fn(),
}));
const windowMocks = vi.hoisted(() => ({ close: vi.fn() }));
const eventMocks = vi.hoisted(() => ({ listen: vi.fn() }));

vi.mock('../src/utils/api', () => ({ api: apiMocks }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => windowMocks }));
vi.mock('@tauri-apps/api/event', () => eventMocks);

const sources = [
  { id: 'screen:0', sourceId: 'main-display', name: 'Built-in Retina Display', kind: 'screen' as const, isPrimary: true, preview: null, width: 2560, height: 1600 },
  { id: 'screen:1', sourceId: 'desk-display', name: 'Studio Display', kind: 'screen' as const, isPrimary: false, preview: null, width: 1920, height: 1080 },
  { id: 'window:0', sourceId: 'terminal-window', name: 'Terminal', kind: 'window' as const, isPrimary: false, preview: null, width: 1440, height: 900 },
];

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: { en: { source: {
    label: 'Sharing source', current: 'Currently sharing', change: 'Change', chooseType: 'Choose what to share', chooseWindow: 'Choose a window or app', entireScreenDescription: 'Share the primary display', windowsDescription: 'Share one running window or app', extendedDescription: 'Use an extended display', runningWindows: 'Running windows and apps', back: 'Back', close: 'Close', cancel: 'Cancel', entireScreen: 'Entire screen', windowsAndApps: 'Window or app', extendedDisplays: 'Extended displays', primary: 'Primary display', resolution: '{width} x {height}', refresh: 'Refresh sources', loading: 'Loading...', noSources: 'No sources', previewLoading: 'Loading preview...', noPreview: 'No preview available', quality: 'Quality', qualityBalanced: 'Balanced', qualityHigh: 'High', qualityUltra: 'Ultra', errorEnumerate: 'Could not load sources', errorSwitch: 'Could not switch',
  } } },
});

function mountPicker(standalone = false) {
  return mount(SourcePicker, { props: { standalone }, global: { plugins: [i18n] } });
}

async function openWindowChooser(wrapper: ReturnType<typeof mountPicker>) {
  await wrapper.get('[data-source-type="window"]').trigger('click');
  await flushPromises();
}

describe('SourcePicker', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    apiMocks.enumerateCaptureSources.mockResolvedValue(sources);
    apiMocks.getCaptureSourcePreview.mockResolvedValue('data:image/jpeg;base64,preview');
    apiMocks.getCaptureTarget.mockResolvedValue({ kind: 'screen', id: 0, sourceId: 'main-display', quality: 0.75 });
    apiMocks.setCaptureTarget.mockResolvedValue(undefined);
    windowMocks.close.mockResolvedValue(undefined);
    eventMocks.listen.mockResolvedValue(vi.fn());
  });

  it('keeps the embedded control compact and shows only the current source', async () => {
    const wrapper = mountPicker();
    await flushPromises();

    expect(wrapper.get('.sp-current-copy strong').text()).toBe('Built-in Retina Display');
    expect(wrapper.find('[data-source-id="desk-display"]').exists()).toBe(false);
    expect(wrapper.find('.sp-type-card').exists()).toBe(false);
  });

  it('shows the AirPlay-style three-choice layout in the standalone window', async () => {
    const wrapper = mountPicker(true);
    await flushPromises();

    expect(wrapper.get('h1').text()).toBe('Choose what to share');
    expect(wrapper.findAll('.sp-type-card')).toHaveLength(3);
    expect(wrapper.get('[data-source-type="screen"] strong').text()).toBe('Entire screen');
    expect(wrapper.get('[data-source-type="window"] strong').text()).toBe('Window or app');
    expect(wrapper.get('[data-source-type="extended"] strong').text()).toBe('Extended displays');
  });

  it('selects the primary display and closes immediately', async () => {
    const wrapper = mountPicker(true);
    await flushPromises();
    apiMocks.setCaptureTarget.mockClear();

    await wrapper.get('[data-source-type="screen"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({ kind: 'screen', id: 0, sourceId: 'main-display', quality: 0.75 });
    expect(windowMocks.close).toHaveBeenCalledOnce();
    expect(wrapper.get('h1').text()).toBe('Choose what to share');
  });

  it('opens the running-window list only after choosing window or app', async () => {
    const wrapper = mountPicker(true);
    await flushPromises();
    await openWindowChooser(wrapper);

    expect(wrapper.get('h1').text()).toBe('Choose a window or app');
    expect(wrapper.get('[data-source-id="terminal-window"]').text()).toContain('Terminal');
    expect(wrapper.find('[data-source-id="desk-display"]').exists()).toBe(false);
    expect(apiMocks.setCaptureTarget).not.toHaveBeenCalled();
  });

  it('selects an extended display directly from the first-level choices', async () => {
    const wrapper = mountPicker(true);
    await flushPromises();
    await wrapper.get('[data-source-type="extended"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenLastCalledWith({ kind: 'screen', id: 1, sourceId: 'desk-display', quality: 0.75 });
    expect(windowMocks.close).toHaveBeenCalledOnce();
  });

  it('sends a stable window source ID without confusing colliding display IDs', async () => {
    const collidingWindow = { ...sources[2], sourceId: 'main-display', name: 'Terminal App' };
    apiMocks.enumerateCaptureSources.mockResolvedValue([...sources, collidingWindow]);
    const wrapper = mountPicker(true);
    await flushPromises();
    await openWindowChooser(wrapper);
    await wrapper.get('[data-source-key="window:main-display"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenLastCalledWith({ kind: 'window', id: 0, sourceId: 'main-display', quality: 0.75 });
    expect(windowMocks.close).toHaveBeenCalledOnce();
  });

  it('does not wait for display previews before selecting the default embedded source', async () => {
    let resolvePreview!: (value: string | null) => void;
    apiMocks.getCaptureSourcePreview.mockImplementation((sourceId: string) => sourceId === 'main-display'
      ? new Promise((resolve) => { resolvePreview = resolve; })
      : Promise.resolve(null));
    apiMocks.getCaptureTarget.mockResolvedValue(null);
    const wrapper = mountPicker();
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({ kind: 'screen', id: 0, sourceId: 'main-display', quality: 0.75 });
    expect(wrapper.get('.sp-current').text()).toContain('Built-in Retina Display');
    resolvePreview('data:image/jpeg;base64,done');
    await flushPromises();
    expect(wrapper.find('.sp-current-preview img').attributes('src')).toBe('data:image/jpeg;base64,done');
  });

  it('refreshes the window list from inside the second-level chooser', async () => {
    const wrapper = mountPicker(true);
    await flushPromises();
    await openWindowChooser(wrapper);
    apiMocks.enumerateCaptureSources.mockResolvedValueOnce([sources[0], { ...sources[2], name: 'Refreshed App' }]);
    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.text()).toContain('Refreshed App');
  });

  it('refreshes sources when the reused standalone window is opened', async () => {
    const wrapper = mountPicker(true);
    await flushPromises();
    const openHandler = eventMocks.listen.mock.calls.find(([event]) => event === 'source-picker-opened')?.[1];
    expect(openHandler).toBeTypeOf('function');

    apiMocks.enumerateCaptureSources.mockResolvedValueOnce([{ ...sources[0], name: 'Updated Display' }]);
    apiMocks.getCaptureSourcePreview.mockResolvedValueOnce('data:image/jpeg;base64,updated');
    await openHandler?.();
    await flushPromises();

    expect(wrapper.get('[data-source-type="screen"] img').attributes('src')).toBe('data:image/jpeg;base64,updated');
    expect(apiMocks.getCaptureSourcePreview).toHaveBeenLastCalledWith('main-display', true);
  });
});
