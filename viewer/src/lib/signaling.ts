export type WireMessage = {
  type: string;
  payload?: Record<string, unknown>;
};

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
      ws.send(
        JSON.stringify({ type: 'USER_ENTER', payload: { username: 'viewer', ts: Date.now() } }),
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