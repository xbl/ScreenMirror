import { ref, shallowRef, type Ref, type ShallowRef } from 'vue';

export type RemoteMouseButton = 'left' | 'right' | 'middle';

export type RemoteInputEvent =
  | { action: 'mouse_move'; x: number; y: number }
  | { action: 'mouse_down'; x: number; y: number; button?: RemoteMouseButton; click_count?: number }
  | { action: 'mouse_up'; x: number; y: number; button?: RemoteMouseButton; click_count?: number }
  | { action: 'mouse_click'; x: number; y: number; button?: RemoteMouseButton; click_count?: number }
  | { action: 'mouse_drag'; x: number; y: number; button?: RemoteMouseButton }
  | { action: 'scroll'; x: number; y: number; delta_x: number; delta_y: number };

const STORAGE_KEY = 'screenmirror_touch_control';

function getInitialTouchControlState(): boolean {
  if (typeof window === 'undefined' || !window.localStorage) {
    return true;
  }
  const saved = window.localStorage.getItem(STORAGE_KEY);
  return saved !== null ? saved === 'true' : true;
}

const touchControlEnabled = ref<boolean>(getInitialTouchControlState());
const controlChannel = shallowRef<RTCDataChannel | null>(null);
const fallbackWs = shallowRef<WebSocket | null>(null);
const isChannelOpen = ref<boolean>(false);

export function useRemoteControl(): {
  touchControlEnabled: Ref<boolean>;
  controlChannel: ShallowRef<RTCDataChannel | null>;
  fallbackWs: ShallowRef<WebSocket | null>;
  isChannelOpen: Ref<boolean>;
  setControlChannel: (dc: RTCDataChannel | null) => void;
  setFallbackWs: (ws: WebSocket | null) => void;
  toggleTouchControl: () => boolean;
  setTouchControlEnabled: (enabled: boolean) => void;
  sendRemoteInput: (event: RemoteInputEvent) => boolean;
  reset: () => void;
} {
  function setControlChannel(dc: RTCDataChannel | null) {
    controlChannel.value = dc;
    if (dc) {
      isChannelOpen.value = dc.readyState === 'open';
      dc.onopen = () => {
        isChannelOpen.value = true;
        console.log('[remote-control] DataChannel open');
      };
      dc.onclose = () => {
        isChannelOpen.value = false;
        console.log('[remote-control] DataChannel closed');
      };
      dc.onerror = (e) => {
        console.warn('[remote-control] DataChannel error:', e);
      };
    } else {
      isChannelOpen.value = false;
    }
  }

  function setFallbackWs(ws: WebSocket | null) {
    fallbackWs.value = ws;
  }

  function toggleTouchControl(): boolean {
    touchControlEnabled.value = !touchControlEnabled.value;
    if (typeof window !== 'undefined' && window.localStorage) {
      window.localStorage.setItem(STORAGE_KEY, String(touchControlEnabled.value));
    }
    return touchControlEnabled.value;
  }

  function setTouchControlEnabled(enabled: boolean) {
    touchControlEnabled.value = enabled;
    if (typeof window !== 'undefined' && window.localStorage) {
      window.localStorage.setItem(STORAGE_KEY, String(enabled));
    }
  }

  function sendRemoteInput(event: RemoteInputEvent): boolean {
    if (!touchControlEnabled.value) {
      return false;
    }

    const dc = controlChannel.value;
    if (dc && dc.readyState === 'open') {
      try {
        dc.send(JSON.stringify(event));
        return true;
      } catch (err) {
        console.warn('[remote-control] DataChannel send failed, falling back to WS:', err);
      }
    }

    const ws = fallbackWs.value;
    if (ws && ws.readyState === WebSocket.OPEN) {
      try {
        ws.send(JSON.stringify({
          type: 'REMOTE_INPUT',
          payload: event,
        }));
        return true;
      } catch (err) {
        console.warn('[remote-control] WebSocket send failed:', err);
      }
    }

    return false;
  }

  function reset() {
    controlChannel.value = null;
    fallbackWs.value = null;
    isChannelOpen.value = false;
  }

  return {
    touchControlEnabled,
    controlChannel,
    fallbackWs,
    isChannelOpen,
    setControlChannel,
    setFallbackWs,
    toggleTouchControl,
    setTouchControlEnabled,
    sendRemoteInput,
    reset,
  };
}
