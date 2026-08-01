import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import SourcePicker from '../src/components/SourcePicker.vue';

const apiMocks = vi.hoisted(() => ({
  checkScreenRecordingPermission: vi.fn(),
  enumerateCaptureSources: vi.fn(),
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
    preview: 'data:image/png;base64,main',
    width: 2560,
    height: 1600,
  },
  {
    id: 'screen:1',
    sourceId: 'desk-display',
    name: 'Studio Display',
    kind: 'screen' as const,
    isPrimary: false,
    preview: 'data:image/png;base64,desk',
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
        noPreview: 'No preview available',
        quality: 'Quality',
        qualityBalanced: 'Balanced',
        qualityHigh: 'High',
        qualityUltra: 'Ultra',
        errorPermission: 'Screen Recording permission is required.',
        errorEnumerate: 'Could not load available sources.',
        errorSwitch: 'Could not switch to this source.',
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

describe('SourcePicker', () => {
  beforeEach(() => {
    apiMocks.checkScreenRecordingPermission.mockResolvedValue(true);
    apiMocks.enumerateCaptureSources.mockResolvedValue(sources);
    apiMocks.setCaptureTarget.mockResolvedValue(undefined);
    vi.clearAllMocks();
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
      'data:image/png;base64,main',
    );
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
});
