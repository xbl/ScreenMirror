import { ref, type Ref } from 'vue';

export type ViewerStatus = 'idle' | 'connecting' | 'streaming' | 'disconnected';

/**
 * Status state machine for the viewer side.
 *
 * Transitions:
 *   idle → connecting          (on mount, immediately)
 *   connecting → streaming     (when the WebRTC media track arrives —
 *                               the caller invokes markStreaming() from the
 *                               existing 'viewer-stream' CustomEvent listener)
 *   streaming → disconnected   (when the host stops pushing or the
 *                               signaling WebSocket closes — caller invokes
 *                               markDisconnected())
 *   disconnected → connecting  (on reset() called by Reconnect button)
 */
export function useViewerStatus(): {
  status: Ref<ViewerStatus>;
  markStreaming: () => void;
  markDisconnected: () => void;
  reset: () => void;
} {
  const status = ref<ViewerStatus>('idle');

  function markStreaming() {
    if (status.value !== 'streaming') status.value = 'streaming';
  }
  function markDisconnected() {
    // Don't downgrade a streaming viewer back to disconnected for transient
    // blips; the caller should debounce. We accept whatever the caller says.
    status.value = 'disconnected';
  }
  function reset() {
    status.value = 'connecting';
  }

  return { status, markStreaming, markDisconnected, reset };
}