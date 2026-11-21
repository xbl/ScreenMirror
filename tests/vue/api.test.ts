import { describe, it, expect } from 'vitest';
import { api } from '../../src/utils/api';

describe('api', () => {
  it('has all expected methods', () => {
    expect(typeof api.getLanIp).toBe('function');
    expect(typeof api.getPort).toBe('function');
    expect(typeof api.createWaitingSession).toBe('function');
    expect(typeof api.resetWaitingSession).toBe('function');
    expect(typeof api.startSharing).toBe('function');
    expect(typeof api.checkScreenRecordingPermission).toBe('function');
    expect(typeof api.requestAndOpenScreenRecordingSettings).toBe('function');
    expect(typeof api.checkForUpdates).toBe('function');
    expect(typeof api.downloadAndInstallUpdate).toBe('function');
    expect(typeof api.getAutoCheckUpdate).toBe('function');
    expect(typeof api.setAutoCheckUpdate).toBe('function');
  });
});
