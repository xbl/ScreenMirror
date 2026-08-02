import { invoke } from '@tauri-apps/api/core';

export type Device = {
  id: string;
  name: string;
  ip: string;
  os: string;
  browser: string;
  roomId: string;
  sharingSessionId: string;
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
  disconnectDevice: (id: string) => invoke<boolean>('disconnect_device', { id }),
  disconnectAllDevices: () => invoke<void>('disconnect_all_devices'),
  isViewerSlotAvailable: () => invoke<boolean>('is_viewer_slot_available'),
  createWaitingSession: (roomId?: string) => invoke<string>('create_waiting_session', { roomId }),
  resetWaitingSession: () => invoke<void>('reset_waiting_session'),
  setDesktopCapturerSourceId: (id: string) => invoke<void>('set_desktop_capturer_source_id', { id }),
  getWaitingSourceId: () => invoke<string | null>('get_waiting_source_id'),
  startSharing: () => invoke<void>('start_sharing'),
  getPendingDevice: () => invoke<Device | null>('get_pending_device'),
  setDeviceConnectedStatus: () => invoke<void>('set_device_connected_status'),
  enumerateCaptureSources: () => invoke<CaptureSourceInfo[]>('enumerate_capture_sources'),
  getCaptureSourcePreview: (sourceId: string, forceRefresh: boolean) =>
    invoke<string | null>('get_capture_source_preview', { sourceId, forceRefresh }),
  getCaptureTarget: () => invoke<CaptureTargetState | null>('get_capture_target'),
  openSourcePickerWindow: () => invoke<void>('open_source_picker_window'),
  closeSourcePickerWindow: () => invoke<void>('close_source_picker_window'),
  closeTrayPanel: () => invoke<void>('close_tray_panel'),
  setCaptureTarget: (args: CaptureTarget) =>
    invoke<void>('set_capture_target', { args }),
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
};

export type CaptureTargetState = {
  kind: CaptureSourceKind;
  id: number;
  sourceId: string | null;
  quality: number;
};
