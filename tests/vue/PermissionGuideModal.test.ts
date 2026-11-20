import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import PermissionGuideModal from '../../src/components/PermissionGuideModal.vue';

const apiMocks = vi.hoisted(() => ({
  requestAndOpenScreenRecordingSettings: vi.fn(),
  checkScreenRecordingPermission: vi.fn(),
}));

vi.mock('../../src/utils/api', () => ({ api: apiMocks }));

const i18n = createI18n({
  legacy: false,
  locale: 'zh-CN',
  messages: {
    'zh-CN': {
      permission: {
        guideTitle: '屏幕录制权限设置指引',
        step1Title: '1. 确认放入「应用程序」目录',
        step1Desc: '请先将 ScreenMirror 拖移至「访达 → 应用程序」中运行',
        step2Title: '2. 点击「前往授权」唤起登记',
        step2Desc: '系统将主动申请权限',
        step3Title: '3. 开启权限开关',
        step3Desc: '在列表中找到 ScreenMirror 并开启开关',
        fallbackTitle: '在列表中未看到 ScreenMirror？',
        fallbackDesc: '点击「+」加号按钮手动添加',
        openSettings: '前往系统设置授权',
        recheck: '已完成设置，重新检测',
        recheckSuccess: '权限已成功授予！',
        recheckFail: '尚未检测到权限，请在系统设置中开启后重试',
      },
      source: {
        loading: '正在载入...',
      },
    },
  },
});

describe('PermissionGuideModal', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('renders modal content when open', () => {
    const wrapper = mount(PermissionGuideModal, {
      props: { open: true },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    expect(wrapper.text()).toContain('屏幕录制权限设置指引');
    expect(wrapper.text()).toContain('1. 确认放入「应用程序」目录');
    expect(wrapper.text()).toContain('在列表中未看到 ScreenMirror？');
    expect(wrapper.text()).toContain('前往系统设置授权');
  });

  it('calls requestAndOpenScreenRecordingSettings when clicking open settings', async () => {
    apiMocks.requestAndOpenScreenRecordingSettings.mockResolvedValue(undefined);
    const wrapper = mount(PermissionGuideModal, {
      props: { open: true },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    const openBtn = wrapper.find('.pg-btn-primary');
    await openBtn.trigger('click');
    await flushPromises();

    expect(apiMocks.requestAndOpenScreenRecordingSettings).toHaveBeenCalledTimes(1);
  });

  it('handles recheck success', async () => {
    apiMocks.checkScreenRecordingPermission.mockResolvedValue(true);
    const wrapper = mount(PermissionGuideModal, {
      props: { open: true },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    const recheckBtn = wrapper.find('.pg-btn-secondary');
    await recheckBtn.trigger('click');
    await flushPromises();

    expect(apiMocks.checkScreenRecordingPermission).toHaveBeenCalledTimes(1);
    expect(wrapper.emitted('granted')).toBeTruthy();
    expect(wrapper.text()).toContain('权限已成功授予！');
  });

  it('handles recheck failure', async () => {
    apiMocks.checkScreenRecordingPermission.mockResolvedValue(false);
    const wrapper = mount(PermissionGuideModal, {
      props: { open: true },
      global: { plugins: [i18n], stubs: { Teleport: true } },
    });

    const recheckBtn = wrapper.find('.pg-btn-secondary');
    await recheckBtn.trigger('click');
    await flushPromises();

    expect(apiMocks.checkScreenRecordingPermission).toHaveBeenCalledTimes(1);
    expect(wrapper.emitted('granted')).toBeFalsy();
    expect(wrapper.text()).toContain('尚未检测到权限');
  });
});
