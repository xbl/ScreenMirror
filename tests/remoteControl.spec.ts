// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useRemoteControl, type RemoteInputEvent } from '../viewer/src/lib/remoteControl';

describe('useRemoteControl', () => {
  const mockStorage: Record<string, string> = {};

  beforeEach(() => {
    for (const key of Object.keys(mockStorage)) {
      delete mockStorage[key];
    }
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => mockStorage[key] ?? null,
      setItem: (key: string, value: string) => {
        mockStorage[key] = value;
      },
      removeItem: (key: string) => {
        delete mockStorage[key];
      },
      clear: () => {
        for (const key of Object.keys(mockStorage)) {
          delete mockStorage[key];
        }
      },
    });

    const { reset, setTouchControlEnabled } = useRemoteControl();
    reset();
    setTouchControlEnabled(true);
  });

  it('initializes with touch control enabled by default', () => {
    const { touchControlEnabled } = useRemoteControl();
    expect(touchControlEnabled.value).toBe(true);
  });

  it('toggles touch control and persists state to localStorage', () => {
    const { touchControlEnabled, toggleTouchControl } = useRemoteControl();
    expect(touchControlEnabled.value).toBe(true);

    const newState = toggleTouchControl();
    expect(newState).toBe(false);
    expect(touchControlEnabled.value).toBe(false);
    expect(localStorage.getItem('screenmirror_touch_control')).toBe('false');

    const stateAfterSecondToggle = toggleTouchControl();
    expect(stateAfterSecondToggle).toBe(true);
    expect(touchControlEnabled.value).toBe(true);
    expect(localStorage.getItem('screenmirror_touch_control')).toBe('true');
  });

  it('prioritizes WebRTC DataChannel when open', () => {
    const { setControlChannel, sendRemoteInput } = useRemoteControl();

    const mockDc = {
      readyState: 'open',
      send: vi.fn(),
    } as unknown as RTCDataChannel;

    setControlChannel(mockDc);

    const event: RemoteInputEvent = { action: 'mouse_move', x: 0.25, y: 0.75 };
    const sent = sendRemoteInput(event);

    expect(sent).toBe(true);
    expect(mockDc.send).toHaveBeenCalledTimes(1);
    expect(mockDc.send).toHaveBeenCalledWith(JSON.stringify(event));
  });

  it('falls back to WebSocket when DataChannel is not open', () => {
    const { setControlChannel, setFallbackWs, sendRemoteInput } = useRemoteControl();

    const mockDc = {
      readyState: 'connecting',
      send: vi.fn(),
    } as unknown as RTCDataChannel;

    const mockWs = {
      readyState: WebSocket.OPEN,
      send: vi.fn(),
    } as unknown as WebSocket;

    setControlChannel(mockDc);
    setFallbackWs(mockWs);

    const event: RemoteInputEvent = {
      action: 'mouse_click',
      x: 0.5,
      y: 0.5,
      button: 'left',
      click_count: 1,
    };
    const sent = sendRemoteInput(event);

    expect(sent).toBe(true);
    expect(mockDc.send).not.toHaveBeenCalled();
    expect(mockWs.send).toHaveBeenCalledTimes(1);
    expect(mockWs.send).toHaveBeenCalledWith(
      JSON.stringify({
        type: 'REMOTE_INPUT',
        payload: event,
      }),
    );
  });

  it('does not send input when touch control is toggled off', () => {
    const { setControlChannel, setTouchControlEnabled, sendRemoteInput } = useRemoteControl();

    const mockDc = {
      readyState: 'open',
      send: vi.fn(),
    } as unknown as RTCDataChannel;

    setControlChannel(mockDc);
    setTouchControlEnabled(false);

    const event: RemoteInputEvent = { action: 'mouse_move', x: 0.5, y: 0.5 };
    const sent = sendRemoteInput(event);

    expect(sent).toBe(false);
    expect(mockDc.send).not.toHaveBeenCalled();
  });
});
