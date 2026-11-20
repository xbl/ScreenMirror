import { describe, it, expect, beforeEach } from 'vitest';
import { useViewerStream } from '../viewer/src/lib/viewerStream';
import { useViewerStatus } from '../viewer/src/lib/viewerStatus';

describe('useViewerStream', () => {
  const {
    activeStream,
    peerConnection,
    hasVideoTrack,
    hasAudioTrack,
    setActiveStream,
    setPeerConnection,
    clearStream,
    reset,
  } = useViewerStream();

  beforeEach(() => {
    reset();
  });

  it('initializes with null stream and no video or audio track', () => {
    expect(activeStream.value).toBeNull();
    expect(peerConnection.value).toBeNull();
    expect(hasVideoTrack.value).toBe(false);
    expect(hasAudioTrack.value).toBe(false);
  });

  it('updates state, sets global __smVideoTrack, and dispatches viewer-stream on setActiveStream', () => {
    let dispatchedStream: unknown = null;
    const listener = (e: Event) => {
      dispatchedStream = (e as CustomEvent).detail;
    };
    window.addEventListener('viewer-stream', listener);

    const mockStream = { id: 'stream-1' } as unknown as MediaStream;
    setActiveStream(mockStream);

    expect(activeStream.value).toBe(mockStream);
    expect(hasVideoTrack.value).toBe(true);
    expect((window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack).toBe(true);
    expect(dispatchedStream).toBe(mockStream);

    window.removeEventListener('viewer-stream', listener);
  });

  it('updates peerConnection and sets global __smPc on setPeerConnection', () => {
    const mockPc = { id: 'pc-1' } as unknown as RTCPeerConnection;
    setPeerConnection(mockPc);

    expect(peerConnection.value).toBe(mockPc);
    expect((window as unknown as { __smPc?: RTCPeerConnection }).__smPc).toBe(mockPc);
  });

  it('clears stream without affecting peerConnection on clearStream', () => {
    const mockPc = { id: 'pc-1' } as unknown as RTCPeerConnection;
    const mockStream = { id: 'stream-1' } as unknown as MediaStream;
    setPeerConnection(mockPc);
    setActiveStream(mockStream);

    clearStream();

    expect(activeStream.value).toBeNull();
    expect(hasVideoTrack.value).toBe(false);
    expect(peerConnection.value).toBe(mockPc);
  });

  it('resets all state and deletes global variables on reset', () => {
    const mockPc = { id: 'pc-1' } as unknown as RTCPeerConnection;
    const mockStream = { id: 'stream-1' } as unknown as MediaStream;
    setPeerConnection(mockPc);
    setActiveStream(mockStream);

    reset();

    expect(activeStream.value).toBeNull();
    expect(peerConnection.value).toBeNull();
    expect(hasVideoTrack.value).toBe(false);
    expect(hasAudioTrack.value).toBe(false);
    expect((window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack).toBeUndefined();
    expect((window as unknown as { __smAudioTrack?: boolean }).__smAudioTrack).toBeUndefined();
    expect((window as unknown as { __smPc?: RTCPeerConnection }).__smPc).toBeUndefined();
  });

  it('correctly detects audio tracks when present on the stream', () => {
    const mockAudioTrack = { id: 'audio-1', kind: 'audio' };
    const mockVideoTrack = { id: 'video-1', kind: 'video' };
    const mockStream = {
      id: 'stream-audio-1',
      getTracks: () => [mockVideoTrack, mockAudioTrack],
      getVideoTracks: () => [mockVideoTrack],
      getAudioTracks: () => [mockAudioTrack],
    } as unknown as MediaStream;

    setActiveStream(mockStream);
    expect(hasVideoTrack.value).toBe(true);
    expect(hasAudioTrack.value).toBe(true);
    expect((window as unknown as { __smAudioTrack?: boolean }).__smAudioTrack).toBe(true);
  });

  it('synchronizes activeStream when viewer-stream event is dispatched externally', () => {
    // In jsdom MediaStream might need to be mocked as an instance of MediaStream
    class MockMediaStream {
      id = 'external-stream';
    }
    // Temporarily register MockMediaStream if MediaStream is not defined in jsdom
    const originalMediaStream = window.MediaStream;
    (window as unknown as { MediaStream: unknown }).MediaStream = MockMediaStream;

    try {
      const extStream = new MockMediaStream() as unknown as MediaStream;
      window.dispatchEvent(new CustomEvent('viewer-stream', { detail: extStream }));

      expect(activeStream.value).toBe(extStream);
      expect(hasVideoTrack.value).toBe(true);
      expect((window as unknown as { __smVideoTrack?: boolean }).__smVideoTrack).toBe(true);
    } finally {
      (window as unknown as { MediaStream: unknown }).MediaStream = originalMediaStream;
    }
  });
});

describe('useViewerStatus', () => {
  it('transitions through status state machine correctly', () => {
    const { status, markStreaming, markDisconnected, reset } = useViewerStatus();

    reset();
    expect(status.value).toBe('connecting');

    markStreaming();
    expect(status.value).toBe('streaming');

    markDisconnected();
    expect(status.value).toBe('disconnected');

    reset();
    expect(status.value).toBe('connecting');
  });
});
