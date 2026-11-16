import { shallowRef, ref, type ShallowRef, type Ref } from 'vue';

// Module-level singletons ensuring reactive stream state survives component lifecycle changes
const activeStream = shallowRef<MediaStream | null>(null);
const peerConnection = shallowRef<RTCPeerConnection | null>(null);
const hasVideoTrack = ref(false);

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
      hasVideoTrack.value = true;
      (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
    }
  });
}

export function useViewerStream(): {
  activeStream: ShallowRef<MediaStream | null>;
  peerConnection: ShallowRef<RTCPeerConnection | null>;
  hasVideoTrack: Ref<boolean>;
  setActiveStream: (stream: MediaStream | null) => void;
  setPeerConnection: (pc: RTCPeerConnection | null) => void;
  clearStream: () => void;
  reset: () => void;
} {
  function setActiveStream(stream: MediaStream | null) {
    activeStream.value = stream;
    if (stream) {
      hasVideoTrack.value = true;
      (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack = true;
      // Dispatch viewer-stream CustomEvent for backward compatibility
      window.dispatchEvent(new CustomEvent('viewer-stream', { detail: stream }));
    } else {
      hasVideoTrack.value = false;
    }
  }

  function setPeerConnection(pc: RTCPeerConnection | null) {
    peerConnection.value = pc;
    (window as unknown as { __smPc?: RTCPeerConnection | null }).__smPc = pc;
  }

  function clearStream() {
    activeStream.value = null;
    hasVideoTrack.value = false;
  }

  function reset() {
    activeStream.value = null;
    hasVideoTrack.value = false;
    peerConnection.value = null;
    if (typeof window !== 'undefined') {
      delete (window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack;
      delete (window as unknown as { __smPc?: RTCPeerConnection | null }).__smPc;
    }
  }

  return {
    activeStream,
    peerConnection,
    hasVideoTrack,
    setActiveStream,
    setPeerConnection,
    clearStream,
    reset,
  };
}
