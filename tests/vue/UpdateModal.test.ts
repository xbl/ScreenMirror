import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import UpdateModal from '../../src/components/UpdateModal.vue';
import type { UpdateInfo } from '../../src/utils/api';

const apiMocks = vi.hoisted(() => ({
  downloadAndInstallUpdate: vi.fn().mockResolvedValue(undefined),
  relaunchApp: vi.fn().mockResolvedValue(undefined),
  openExternalLink: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../src/utils/api', () => ({ api: apiMocks }));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const i18n = createI18n({
  legacy: false,
  locale: 'zh-CN',
  messages: {
    'zh-CN': {
      settings: {
        checkForUpdates: '检查更新',
      },
      updater: {
        title: '软件更新',
        checking: '正在检查更新…',
        upToDate: '当前已是最新版本',
        upToDateDetail: '您当前运行的 ScreenMirror v{version} 已是最新版本。',
        newVersionFound: '发现新版本 v{version}',
        currentVersion: '当前版本：v{version}',
        releaseDate: '发布时间：{date}',
        releaseNotes: '更新内容：',
        updateNow: '立即下载更新',
        viewOnGithub: '前往 GitHub 查看',
        downloading: '正在下载更新…',
        readyToRestart: '更新包已替换就绪，立即重启应用以完成更新。',
        restartNow: '立即重启',
        later: '稍后提醒',
        close: '关闭',
        failed: '检查更新失败',
      },
    },
  },
});

describe('UpdateModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders checking state when checking is true', () => {
    const wrapper = mount(UpdateModal, {
      props: {
        open: true,
        updateInfo: null,
        checking: true,
        error: null,
      },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    expect(wrapper.text()).toContain('正在检查更新…');
    expect(wrapper.find('.up-spinner').exists()).toBe(true);
  });

  it('renders up-to-date state when hasUpdate is false', () => {
    const info: UpdateInfo = {
      currentVersion: '0.1.1',
      latestVersion: '0.1.1',
      hasUpdate: false,
      releaseNotes: null,
      releaseUrl: 'https://github.com/xbl/ScreenMirror/releases',
      publishedAt: '2026-11-20T00:00:00Z',
      downloadUrl: null,
      assetName: null,
      assetSize: null,
    };

    const wrapper = mount(UpdateModal, {
      props: {
        open: true,
        updateInfo: info,
        checking: false,
        error: null,
      },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    expect(wrapper.text()).toContain('当前已是最新版本');
    expect(wrapper.text()).toContain('ScreenMirror v0.1.1');
    expect(wrapper.find('.up-badge-check').exists()).toBe(true);
  });

  it('renders update available state and triggers download on click', async () => {
    const info: UpdateInfo = {
      currentVersion: '0.1.1',
      latestVersion: '0.1.2',
      hasUpdate: true,
      releaseNotes: '### New features\n- Auto update supported',
      releaseUrl: 'https://github.com/xbl/ScreenMirror/releases/tag/v0.1.2',
      publishedAt: '2026-11-21T00:00:00Z',
      downloadUrl:
        'https://github.com/xbl/ScreenMirror/releases/download/v0.1.2/ScreenMirror-v0.1.2-aarch64.app.zip',
      assetName: 'ScreenMirror-v0.1.2-aarch64.app.zip',
      assetSize: 15000000,
    };

    const wrapper = mount(UpdateModal, {
      props: {
        open: true,
        updateInfo: info,
        checking: false,
        error: null,
      },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    expect(wrapper.text()).toContain('发现新版本 v0.1.2');
    expect(wrapper.text()).toContain('Auto update supported');

    const updateBtn = wrapper.find('.up-btn-primary');
    expect(updateBtn.text()).toContain('立即下载更新');
    await updateBtn.trigger('click');
    await flushPromises();

    expect(apiMocks.downloadAndInstallUpdate).toHaveBeenCalledWith(info.downloadUrl);
  });

  it('emits close event when close button is clicked', async () => {
    const wrapper = mount(UpdateModal, {
      props: {
        open: true,
        updateInfo: null,
        checking: false,
        error: null,
      },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    await wrapper.find('.up-close').trigger('click');
    expect(wrapper.emitted('close')).toBeTruthy();
  });
});
