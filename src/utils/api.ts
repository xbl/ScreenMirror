import { invoke } from '@tauri-apps/api/core';

export type Device = {
  id: string;
  name: string;
  ip: string;
  os: string;
  browser: string;
  roomId: string;
  sharingSessionId: string;
  screenWidth?: number;
  screenHeight?: number;
  devicePixelRatio?: number;
};

export type CaptureSourceKind = 'screen' | 'window';

export type CaptureTarget = {
  kind: CaptureSourceKind;
  id: number;
  sourceId: string;
  quality: number;
};

export const api = {
  getLanIp: () => invoke<string | null>('get_lan_ip'),
  checkWifi: () => invoke<boolean>('check_wifi_connection'),
  checkScreenRecordingPermission: () => invoke<boolean>('check_screen_recording_permission'),
  requestScreenRecordingPermission: () => invoke<boolean>('request_screen_recording_permission'),
  openScreenRecordingSettings: () => invoke<void>('open_screen_recording_settings'),
  requestAndOpenScreenRecordingSettings: () => invoke<void>('request_and_open_screen_recording_settings'),
  checkAccessibilityPermission: () => invoke<boolean>('check_accessibility_permission'),
  requestAccessibilityPermission: () => invoke<boolean>('request_accessibility_permission'),
  openAccessibilitySettings: () => invoke<void>('open_accessibility_settings'),
  requestAndOpenAccessibilitySettings: () => invoke<void>('request_and_open_accessibility_settings'),
  getPort: () => invoke<number>('get_port'),
  getAppLanguage: () => invoke<string>('get_app_language'),
  setAppLanguage: (lang: string) => invoke<void>('set_app_language', { lang }),
  getIsFirstTimeStart: () => invoke<boolean>('get_is_first_time_start'),
  setAppStartedOnce: () => invoke<void>('set_app_started_once'),
  getCurrentVersion: () => invoke<string>('get_current_version'),
  openExternalLink: (url: string) => invoke<void>('open_external_link', { url }),
  writeTextToClipboard: (text: string) => invoke<void>('write_text_to_clipboard', { text }),
  relaunchApp: () => invoke<void>('relaunch_app'),
  exitApp: () => invoke<void>('exit_app'),
  getConnectedDevices: () => invoke<Device[]>('get_connected_devices'),
  isViewerSlotAvailable: () => invoke<boolean>('is_viewer_slot_available'),
  createWaitingSession: (roomId?: string) => invoke<string>('create_waiting_session', { roomId }),
  resetWaitingSession: () => invoke<void>('reset_waiting_session'),
  setDesktopCapturerSourceId: (id: string) => invoke<void>('set_desktop_capturer_source_id', { id }),
  getWaitingSourceId: () => invoke<string | null>('get_waiting_source_id'),
  startSharing: () => invoke<void>('start_sharing'),
  getPendingDevice: () => invoke<Device | null>('get_pending_device'),
  setDeviceConnectedStatus: () => invoke<void>('set_device_connected_status'),
  enumerateCaptureSources: () => invoke<CaptureSourceInfo[]>('enumerate_capture_sources'),
  getCaptureSourcePreview: (sourceId: string, forceRefresh: boolean, sourceKind: CaptureSourceKind) =>
    invoke<string | null>('get_capture_source_preview', { sourceId, forceRefresh, sourceKind }),
  getCaptureTarget: () => invoke<CaptureTargetState | null>('get_capture_target'),
  openSourcePickerWindow: () => invoke<void>('open_source_picker_window'),
  closeSourcePickerWindow: () => invoke<void>('close_source_picker_window'),
  startSourcePickerDrag: () => invoke<void>('start_source_picker_drag'),
  closeTrayPanel: () => invoke<void>('close_tray_panel'),
  setCaptureTarget: (args: CaptureTarget) =>
    invoke<void>('set_capture_target', { args }),
  isVirtualDisplaySupported: () => invoke<boolean>('is_virtual_display_supported'),
  createVirtualDisplay: (config: VirtualDisplayConfig) =>
    invoke<number>('create_virtual_display', { config }),
  destroyVirtualDisplay: (displayId: number) =>
    invoke<void>('destroy_virtual_display', { displayId }),
  destroyAllVirtualDisplays: () => invoke<void>('destroy_all_virtual_displays'),
  getActiveVirtualDisplays: () => invoke<number[]>('get_active_virtual_displays'),
};

export type VirtualDisplayConfig = {
  name?: string;
  width: number;
  height: number;
  refreshRate?: number;
  hiDpi?: boolean;
};

export type CaptureSourceInfo = {
  id: string;
  sourceId: string;
  name: string;
  kind: CaptureSourceKind;
  isPrimary: boolean;
  preview: string | null;
  width: number;
  height: number;
  isVirtual?: boolean;
};

export type CaptureTargetState = {
  kind: CaptureSourceKind;
  id: number;
  sourceId: string | null;
  quality: number;
};
