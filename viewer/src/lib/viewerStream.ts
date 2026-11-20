import { shallowRef, ref, type ShallowRef, type Ref } from 'vue';

// Module-level singletons ensuring reactive stream state survives component lifecycle changes
const activeStream = shallowRef<MediaStream | null>(null);
const peerConnection = shallowRef<RTCPeerConnection | null>(null);
const hasVideoTrack = ref(false);
const hasAudioTrack = ref(false);

// Global window event listener to catch any externally dispatched 'viewer-stream' events
if (typeof window !== 'undefined') {
  window.addEventListener('viewer-stream', (e: Event) => {
    const stream = (e as CustomEvent<MediaStream>).detail;
    const isStream =
      typeof MediaStream !== 'undefined'
        ? stream instanceof MediaStream
        : stream !== null && typeof stream === 'object';
    if (isStream && activeStream.value !== stream) {
      activeStream.value = stream;
      const vTracks = typeof stream.getVideoTracks === 'function' ? stream.getVideoTracks() : [];
      const aTracks = typeof stream.getAudioTracks === 'function' ? stream.getAudioTracks() : [];
      hasVideoTrack.value = typeof stream.getVideoTracks === 'function' ? vTracks.length > 0 : true;
      hasAudioTrack.value = typeof stream.getAudioTracks === 'function' ? aTracks.length > 0 : false;
      if (hasVideoTrack.value) {
        (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
      }
      if (hasAudioTrack.value) {
        (window as unknown as { __smAudioTrack?: boolean }).__smAudioTrack = true;
      }
    }
  });
}

export function useViewerStream(): {
  activeStream: ShallowRef<MediaStream | null>;
  peerConnection: ShallowRef<RTCPeerConnection | null>;
  hasVideoTrack: Ref<boolean>;
  hasAudioTrack: Ref<boolean>;
  setActiveStream: (stream: MediaStream | null) => void;
  setPeerConnection: (pc: RTCPeerConnection | null) => void;
  clearStream: () => void;
  reset: () => void;
} {
  function setActiveStream(stream: MediaStream | null) {
    if (stream && activeStream.value && activeStream.value !== stream) {
      if (typeof stream.getTracks === 'function' && typeof activeStream.value.getTracks === 'function') {
        for (const track of stream.getTracks()) {
          if (!activeStream.value.getTracks().some((t) => t.id === track.id)) {
            activeStream.value.addTrack(track);
          }
        }
      }
      const vTracks = typeof activeStream.value.getVideoTracks === 'function' ? activeStream.value.getVideoTracks() : [];
      const aTracks = typeof activeStream.value.getAudioTracks === 'function' ? activeStream.value.getAudioTracks() : [];
      hasVideoTrack.value = typeof activeStream.value.getVideoTracks === 'function' ? vTracks.length > 0 : true;
      hasAudioTrack.value = typeof activeStream.value.getAudioTracks === 'function' ? aTracks.length > 0 : false;
      if (hasVideoTrack.value) {
        (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
      }
      if (hasAudioTrack.value) {
        (window as unknown as { __smAudioTrack?: boolean }).__smAudioTrack = true;
      }
      window.dispatchEvent(new CustomEvent('viewer-stream', { detail: activeStream.value }));
      return;
    }

    activeStream.value = stream;
    if (stream) {
      const vTracks = typeof stream.getVideoTracks === 'function' ? stream.getVideoTracks() : [];
      const aTracks = typeof stream.getAudioTracks === 'function' ? stream.getAudioTracks() : [];
      hasVideoTrack.value = typeof stream.getVideoTracks === 'function' ? vTracks.length > 0 : true;
      hasAudioTrack.value = typeof stream.getAudioTracks === 'function' ? aTracks.length > 0 : false;
      if (hasVideoTrack.value) {
        (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
      }
      if (hasAudioTrack.value) {
        (window as unknown as { __smAudioTrack?: boolean }).__smAudioTrack = true;
      }
      // Dispatch viewer-stream CustomEvent for backward compatibility
      window.dispatchEvent(new CustomEvent('viewer-stream', { detail: stream }));
    } else {
      hasVideoTrack.value = false;
      hasAudioTrack.value = false;
    }
  }

  function setPeerConnection(pc: RTCPeerConnection | null) {
    peerConnection.value = pc;
    (window as unknown as { __smPc?: RTCPeerConnection | null }).__smPc = pc;
  }

  function clearStream() {
    activeStream.value = null;
    hasVideoTrack.value = false;
    hasAudioTrack.value = false;
  }

  function reset() {
    activeStream.value = null;
    hasVideoTrack.value = false;
    hasAudioTrack.value = false;
    peerConnection.value = null;
    if (typeof window !== 'undefined') {
      delete (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack;
      delete (window as unknown as { __smAudioTrack?: boolean }).__smAudioTrack;
      delete (window as unknown as { __smPc?: RTCPeerConnection | null }).__smPc;
    }
  }

  return {
    activeStream,
    peerConnection,
    hasVideoTrack,
    hasAudioTrack,
    setActiveStream,
    setPeerConnection,
    clearStream,
    reset,
  };
}
