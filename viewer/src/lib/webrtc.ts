export interface WebRTCPeer {
  pc: RTCPeerConnection;
  setRemote: (sdp: string) => Promise<void>;
  close: () => void;
}

export function createPeer(): WebRTCPeer {
  const pc = new RTCPeerConnection({ iceServers: [] });
  pc.addTransceiver('video', { direction: 'recvonly' });
  return {
    pc,
    async setRemote(sdp) {
      await pc.setRemoteDescription({ type: 'answer', sdp });
    },
    close() {
      pc.close();
    },
  };
}
