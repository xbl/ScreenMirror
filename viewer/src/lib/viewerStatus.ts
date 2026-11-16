import { ref, type Ref } from 'vue';

export type ViewerStatus = 'idle' | 'connecting' | 'streaming' | 'disconnected';

const sharedStatus = ref<ViewerStatus>('idle');

/**
 * Status state machine for the viewer side.
 *
 * Transitions:
 *   idle → connecting          (on mount, immediately)
 *   connecting → streaming     (when the WebRTC media track arrives —
 *                               the caller invokes markStreaming() from the
 *                               reactive activeStream or 'viewer-stream' event)
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
  function markStreaming() {
    if (sharedStatus.value !== 'streaming') sharedStatus.value = 'streaming';
  }
  function markDisconnected() {
    // Don't downgrade a streaming viewer back to disconnected for transient
    // blips; the caller should debounce. We accept whatever the caller says.
    sharedStatus.value = 'disconnected';
  }
  function reset() {
    sharedStatus.value = 'connecting';
  }

  return { status: sharedStatus, markStreaming, markDisconnected, reset };
}
