export type WireMessage = {
  type: string;
  payload?: Record<string, unknown>;
};

export type ViewerMetadata = {
  os: string;
  browser: string;
};

export function getViewerMetadata(): ViewerMetadata {
  const userAgent = navigator.userAgent;
  const platform = navigator.platform;
  const os = /iPhone|iPad|iPod/.test(userAgent)
    ? 'iOS'
    : /Mac/.test(platform) || /Mac OS/.test(userAgent)
      ? 'macOS'
      : /Win/.test(platform) || /Windows/.test(userAgent)
        ? 'Windows'
        : /Android/.test(userAgent)
          ? 'Android'
          : /Linux/.test(platform) || /Linux/.test(userAgent)
            ? 'Linux'
            : 'Unknown OS';
  const browser = /Edg\//.test(userAgent)
    ? 'Edge'
    : /Firefox\//.test(userAgent)
      ? 'Firefox'
      : /CriOS\//.test(userAgent)
        ? 'Chrome (iOS)'
        : /Chrome\//.test(userAgent)
          ? 'Chrome'
          : /Safari\//.test(userAgent)
            ? 'Safari'
            : 'Unknown browser';
  return { os, browser };
}

export function connectSignaling(
  roomId: string,
  onMessage: (m: WireMessage) => void,
  onOpen?: () => void,
  onClose?: () => void,
): WebSocket {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${window.location.host}/api/ws?roomId=${encodeURIComponent(roomId)}`;
  const ws = new WebSocket(url);
  ws.onopen = () => {
    onOpen?.();
    try {
      const metadata = getViewerMetadata();
      ws.send(
        JSON.stringify({ type: 'USER_ENTER', payload: { username: 'viewer', ts: Date.now(), ...metadata } }),
      );
    } catch {}
  };
  ws.onmessage = (e) => {
    try {
      const m = JSON.parse(e.data) as WireMessage;
      onMessage(m);
    } catch {
      // ignore non-JSON
    }
  };
  ws.onclose = () => onClose?.();
  ws.onerror = () => onClose?.();
  return ws;
}
