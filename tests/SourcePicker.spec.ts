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

vi.mock('../src/utils/api', () => ({
  api: apiMocks,
}));

const sources = [
  {
    id: 'screen:0',
    sourceId: 'main-display',
    name: 'Built-in Retina Display',
    kind: 'screen' as const,
    isPrimary: true,
    preview: null,
    width: 2560,
    height: 1600,
  },
  {
    id: 'screen:1',
    sourceId: 'desk-display',
    name: 'Studio Display',
    kind: 'screen' as const,
    isPrimary: false,
    preview: null,
    width: 1920,
    height: 1080,
  },
  {
    id: 'window:0',
    sourceId: 'terminal-window',
    name: 'Terminal',
    kind: 'window' as const,
    isPrimary: false,
    preview: null,
    width: 1440,
    height: 900,
  },
  {
    id: 'window:1',
    sourceId: 'main-display',
    name: 'Main Display App',
    kind: 'window' as const,
    isPrimary: false,
    preview: null,
    width: 1280,
    height: 720,
  },
];

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      source: {
        label: 'Sharing source',
        entireScreen: 'Entire screen',
        windowsAndApps: 'Window or app',
        extendedDisplays: 'Extended displays',
        primary: 'Primary display',
        resolution: '{width} x {height}',
        refresh: 'Refresh sources',
        loading: 'Loading available sources...',
        noSources: 'No shareable sources are available.',
        previewLoading: 'Loading preview...',
        noPreview: 'No preview available',
        quality: 'Quality',
        qualityBalanced: 'Balanced',
        qualityHigh: 'High',
        qualityUltra: 'Ultra',
        errorPermission: 'Screen Recording permission is required.',
        errorEnumerate: 'Could not load available sources.',
        errorSwitch: 'Could not switch to this source.',
        errorSourceGone: 'The current source is no longer available. Your share is unchanged.',
      },
    },
  },
});

function mountPicker() {
  return mount(SourcePicker, {
    global: {
      plugins: [i18n],
    },
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function beginRefresh(wrapper: ReturnType<typeof mountPicker>, forceRefresh = false) {
  const { refreshSources } = (wrapper.vm.$ as unknown as {
    setupState: { refreshSources: (forceRefresh?: boolean) => Promise<void> };
  }).setupState;
  return refreshSources(forceRefresh);
}

describe('SourcePicker', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    apiMocks.checkScreenRecordingPermission.mockResolvedValue(true);
    apiMocks.enumerateCaptureSources.mockResolvedValue(sources);
    apiMocks.getCaptureSourcePreview.mockImplementation((sourceId: string) =>
      Promise.resolve(`data:image/jpeg;base64,${sourceId}`),
    );
    apiMocks.setCaptureTarget.mockResolvedValue(undefined);
  });

  it('groups available sources and renders the selected display preview', async () => {
    const wrapper = mountPicker();
    await flushPromises();

    expect(wrapper.text()).toContain('Entire screen');
    expect(wrapper.text()).toContain('Window or app');
    expect(wrapper.text()).toContain('Extended displays');
    expect(wrapper.text()).toContain('Built-in Retina Display');
    expect(wrapper.text()).toContain('Primary display');
    expect(wrapper.text()).toContain('2560 x 1600');
    expect(wrapper.find('[data-source-id="main-display"]').classes()).toContain('selected');
    expect(wrapper.find('[data-testid="source-preview"] img').attributes('src')).toBe(
      'data:image/jpeg;base64,main-display',
    );
    expect(apiMocks.getCaptureSourcePreview).toHaveBeenCalledWith('main-display', false);
  });

  it('sends a stable source ID when choosing another source', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.setCaptureTarget.mockClear();

    await wrapper.get('[data-source-id="desk-display"]').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({
      kind: 'screen',
      id: 1,
      sourceId: 'desk-display',
      quality: 0.75,
    });
    expect(wrapper.find('[data-source-id="desk-display"]').classes()).toContain('selected');
  });

  it('keeps display and window sources distinct when native IDs collide', async () => {
    const wrapper = mountPicker();
    await flushPromises();

    const display = wrapper.get('[data-source-key="screen:main-display"]');
    const app = wrapper.get('[data-source-key="window:main-display"]');
    expect(display.classes()).toContain('selected');

    await app.trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenLastCalledWith({
      kind: 'window',
      id: 1,
      sourceId: 'main-display',
      quality: 0.75,
    });
    expect(app.classes()).toContain('selected');
    expect(display.classes()).not.toContain('selected');

    apiMocks.enumerateCaptureSources.mockResolvedValueOnce([sources[0], sources[3]]);
    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-testid="source-preview"] .sp-preview-name').text()).toBe(
      'Main Display App',
    );
  });

  it('uses composite keys for preview loading when display and window IDs collide', async () => {
    const pendingPreview = deferred<string | null>();
    apiMocks.getCaptureSourcePreview.mockImplementation(() => pendingPreview.promise);

    const wrapper = mountPicker();
    await flushPromises();

    expect(wrapper.get('[data-source-key="screen:main-display"] .sp-no-preview').text()).toBe(
      'Loading preview...',
    );
    expect(wrapper.get('[data-source-key="window:main-display"] .sp-no-preview').text()).toBe(
      'No preview available',
    );

    pendingPreview.resolve(null);
    await flushPromises();
  });

  it('does not wait for display previews before selecting the default source', async () => {
    const pendingPreview = deferred<string | null>();
    apiMocks.getCaptureSourcePreview.mockImplementation(() => pendingPreview.promise);

    const wrapper = mountPicker();
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({
      kind: 'screen',
      id: 0,
      sourceId: 'main-display',
      quality: 0.75,
    });
    expect(wrapper.find('[data-testid="source-preview"] img').exists()).toBe(false);
    expect(wrapper.get('[data-testid="source-preview"]').text()).toContain('Loading preview...');
    expect(wrapper.get('.sp-refresh').attributes('disabled')).toBeDefined();

    pendingPreview.resolve('data:image/jpeg;base64,main-display');
    await flushPromises();

    expect(wrapper.find('[data-testid="source-preview"] img').attributes('src')).toBe(
      'data:image/jpeg;base64,main-display',
    );
    expect(wrapper.get('.sp-refresh').attributes('disabled')).toBeUndefined();
  });

  it('keeps the null preview fallback when preview capture fails', async () => {
    apiMocks.getCaptureSourcePreview.mockRejectedValue(new Error('capture unavailable'));

    const wrapper = mountPicker();
    await flushPromises();

    expect(wrapper.find('[data-testid="source-preview"] img').exists()).toBe(false);
    expect(wrapper.get('[data-testid="source-preview"]').text()).toContain('No preview available');
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });

  it('keeps the previous source selected when switching fails', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.setCaptureTarget.mockRejectedValueOnce(new Error('capture unavailable'));

    await wrapper.get('[data-source-id="desk-display"]').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-source-id="main-display"]').classes()).toContain('selected');
    expect(wrapper.find('[data-source-id="desk-display"]').classes()).not.toContain('selected');
    expect(wrapper.text()).toContain('Could not switch to this source.');
  });

  it('keeps the selected source when refreshing sources fails', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.enumerateCaptureSources.mockRejectedValueOnce(new Error('unavailable'));

    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-source-id="main-display"]').classes()).toContain('selected');
    expect(wrapper.text()).toContain('Could not load available sources.');
  });

  it('shows a permission error without changing the selected source', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.checkScreenRecordingPermission.mockResolvedValueOnce(false);

    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-source-id="main-display"]').classes()).toContain('selected');
    expect(wrapper.text()).toContain('Screen Recording permission is required.');
  });

  it('keeps the active source when it disappears during a successful refresh', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.setCaptureTarget.mockClear();
    apiMocks.enumerateCaptureSources.mockResolvedValueOnce(sources.slice(1));

    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(apiMocks.setCaptureTarget).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="source-preview"]').text()).toContain('Built-in Retina Display');
    expect(wrapper.text()).toContain('The current source is no longer available.');
  });

  it('only sends the latest rapidly selected source after an out-of-order permission check', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    const firstPermission = deferred<boolean>();
    apiMocks.setCaptureTarget.mockClear();
    apiMocks.checkScreenRecordingPermission
      .mockImplementationOnce(() => firstPermission.promise)
      .mockResolvedValueOnce(true);

    await wrapper.get('[data-source-id="desk-display"]').trigger('click');
    await wrapper.get('[data-source-id="terminal-window"]').trigger('click');
    await flushPromises();
    firstPermission.resolve(true);
    await flushPromises();

    expect(apiMocks.setCaptureTarget).toHaveBeenCalledTimes(1);
    expect(apiMocks.setCaptureTarget).toHaveBeenCalledWith({
      kind: 'window',
      id: 0,
      sourceId: 'terminal-window',
      quality: 0.75,
    });
    expect(wrapper.find('[data-source-id="terminal-window"]').classes()).toContain('selected');
  });

  it('does not show a stale permission error after a newer selection succeeds', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    const firstPermission = deferred<boolean>();
    apiMocks.checkScreenRecordingPermission
      .mockImplementationOnce(() => firstPermission.promise)
      .mockResolvedValueOnce(true);

    await wrapper.get('[data-source-id="desk-display"]').trigger('click');
    await wrapper.get('[data-source-id="terminal-window"]').trigger('click');
    await flushPromises();
    firstPermission.resolve(false);
    await flushPromises();

    expect(wrapper.find('[data-source-id="terminal-window"]').classes()).toContain('selected');
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });

  it('does not show a stale refresh permission error after a newer refresh succeeds', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    const firstPermission = deferred<boolean>();
    apiMocks.checkScreenRecordingPermission
      .mockImplementationOnce(() => firstPermission.promise)
      .mockResolvedValueOnce(true);

    void beginRefresh(wrapper);
    await flushPromises();
    void beginRefresh(wrapper);
    await flushPromises();
    firstPermission.resolve(false);
    await flushPromises();

    expect(wrapper.find('[data-source-id="main-display"]').classes()).toContain('selected');
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  });

  it('keeps the newest refresh result when an older enumeration finishes later', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    const olderResult = deferred<typeof sources>();
    const newerResult = deferred<typeof sources>();
    apiMocks.enumerateCaptureSources
      .mockImplementationOnce(() => olderResult.promise)
      .mockImplementationOnce(() => newerResult.promise);

    void beginRefresh(wrapper);
    await flushPromises();
    void beginRefresh(wrapper);
    await flushPromises();
    newerResult.resolve([{ ...sources[0], name: 'Newest display' }]);
    await flushPromises();
    olderResult.resolve([{ ...sources[0], name: 'Stale display' }]);
    await flushPromises();

    expect(wrapper.text()).toContain('Newest display');
    expect(wrapper.text()).not.toContain('Stale display');
    expect(wrapper.get('.sp-refresh').attributes('disabled')).toBeUndefined();
  });

  it('keeps the selected preview visible when a refresh returns no sources', async () => {
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.enumerateCaptureSources.mockResolvedValueOnce([]);

    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-testid="source-preview"]').text()).toContain('Built-in Retina Display');
    expect(wrapper.text()).toContain('The current source is no longer available.');
    expect(wrapper.text()).toContain('No shareable sources are available.');
  });

  it('replaces the selected display preview after a refresh', async () => {
    apiMocks.getCaptureSourcePreview.mockResolvedValue('data:image/jpeg;base64,refreshed');
    const wrapper = mountPicker();
    await flushPromises();
    apiMocks.enumerateCaptureSources.mockResolvedValueOnce([
      { ...sources[0], preview: null },
    ]);

    await wrapper.get('.sp-refresh').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-testid="source-preview"] img').attributes('src')).toBe(
      'data:image/jpeg;base64,refreshed',
    );
    expect(apiMocks.getCaptureSourcePreview).toHaveBeenLastCalledWith('main-display', true);
  });

  it('does not let an older preview request replace a refreshed preview', async () => {
    const olderPreview = deferred<string | null>();
    apiMocks.getCaptureSourcePreview.mockImplementation((sourceId: string, forceRefresh: boolean) => {
      if (sourceId === 'main-display' && !forceRefresh) return olderPreview.promise;
      if (sourceId === 'main-display') return Promise.resolve('data:image/jpeg;base64,newest');
      return Promise.resolve(null);
    });
    const wrapper = mountPicker();
    await flushPromises();

    await beginRefresh(wrapper, true);
    await flushPromises();
    expect(wrapper.find('[data-testid="source-preview"] img').attributes('src')).toBe(
      'data:image/jpeg;base64,newest',
    );

    olderPreview.resolve('data:image/jpeg;base64,stale');
    await flushPromises();

    expect(wrapper.find('[data-testid="source-preview"] img').attributes('src')).toBe(
      'data:image/jpeg;base64,newest',
    );
  });

  it('labels null previews as unavailable instead of rendering a thumbnail', async () => {
    const wrapper = mountPicker();
    await flushPromises();

    await wrapper.get('[data-source-id="terminal-window"]').trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-source-id="terminal-window"] .sp-no-preview').text()).toBe(
      'No preview available',
    );
    expect(wrapper.get('[data-testid="source-preview"]').text()).toContain('No preview available');
    expect(wrapper.find('[data-testid="source-preview"] img').exists()).toBe(false);
  });
});
