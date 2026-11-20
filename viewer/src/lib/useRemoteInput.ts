import { ref, onBeforeUnmount, watch, type Ref } from 'vue';
import { clientToNormalized } from './coordinateMapping';
import { useRemoteControl } from './remoteControl';

export interface RemoteInputOptions {
  videoEl: Ref<HTMLVideoElement | null>;
  canvasEl?: Ref<HTMLCanvasElement | null>;
  longPressDelayMs?: number;
  tapDistanceThresholdPx?: number;
}

export function useRemoteInput(options: RemoteInputOptions) {
  const {
    videoEl,
    canvasEl,
    longPressDelayMs = 500,
    tapDistanceThresholdPx = 10,
  } = options;

  const { touchControlEnabled, sendRemoteInput } = useRemoteControl();

  // Gesture state tracking
  let pointerDownPos: { clientX: number; clientY: number; time: number } | null = null;
  let lastTapTime = 0;
  let lastTapPos: { clientX: number; clientY: number } | null = null;
  let longPressTimer: number | undefined;
  let longPressTriggered = false;
  let isDragging = false;
  let lastMoveTime = 0;

  // Two-finger touch scroll and pinch-zoom state
  let lastTwoFingerPos: { x: number; y: number } | null = null;
  const zoomScale = ref<number>(1.0);
  const panX = ref<number>(0);
  const panY = ref<number>(0);

  let initialPinchDist = 0;
  let initialPinchScale = 1.0;
  let isPinching = false;

  function resetZoom() {
    zoomScale.value = 1.0;
    panX.value = 0;
    panY.value = 0;
  }

  function getContentDimensions(): { width: number; height: number } | null {
    const video = videoEl.value;
    if (!video) return null;
    const width = video.videoWidth || (canvasEl?.value ? canvasEl.value.width : 0);
    const height = video.videoHeight || (canvasEl?.value ? canvasEl.value.height : 0);
    if (width <= 0 || height <= 0) return null;
    return { width, height };
  }

  function getNormalized(e: { clientX: number; clientY: number }) {
    const video = videoEl.value;
    if (!video) return null;
    const dims = getContentDimensions();
    if (!dims) return null;

    const rect = video.getBoundingClientRect();
    return clientToNormalized(e.clientX, e.clientY, rect, dims.width, dims.height, 'contain');
  }

  function clearLongPress() {
    if (longPressTimer !== undefined) {
      window.clearTimeout(longPressTimer);
      longPressTimer = undefined;
    }
  }

  // --- Pointer Event Handlers (Mouse & Single-Touch) ---

  function onPointerDown(e: PointerEvent) {
    if (!touchControlEnabled.value) return;

    // Ignore multi-touch start here; touch events handle two-finger scroll
    if (e.pointerType === 'touch' && !e.isPrimary) return;

    const norm = getNormalized(e);
    if (!norm || !norm.isInside) return;

    clearLongPress();
    longPressTriggered = false;
    isDragging = false;
    pointerDownPos = { clientX: e.clientX, clientY: e.clientY, time: Date.now() };

    // Physical mouse events: direct pass-through
    if (e.pointerType === 'mouse') {
      const button = e.button === 2 ? 'right' : e.button === 1 ? 'middle' : 'left';
      sendRemoteInput({
        action: 'mouse_down',
        x: norm.normX,
        y: norm.normY,
        button,
      });
      return;
    }

    // Touch / Stylus input: schedule long-press detection
    const downNormX = norm.normX;
    const downNormY = norm.normY;
    longPressTimer = window.setTimeout(() => {
      longPressTimer = undefined;
      longPressTriggered = true;
      if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
        navigator.vibrate?.(40);
      }
      sendRemoteInput({
        action: 'mouse_click',
        x: downNormX,
        y: downNormY,
        button: 'right',
        click_count: 1,
      });
    }, longPressDelayMs);
  }

  function onPointerMove(e: PointerEvent) {
    if (!touchControlEnabled.value) return;
    if (e.pointerType === 'touch' && !e.isPrimary) return;

    const norm = getNormalized(e);
    if (!norm) return;

    const now = Date.now();

    // Physical mouse hover / drag
    if (e.pointerType === 'mouse') {
      // Throttle mouse moves to ~100Hz (10ms) to prevent event storming
      if (now - lastMoveTime < 10) return;
      lastMoveTime = now;

      if (e.buttons === 1) {
        sendRemoteInput({ action: 'mouse_drag', x: norm.normX, y: norm.normY, button: 'left' });
      } else if (e.buttons === 2) {
        sendRemoteInput({ action: 'mouse_drag', x: norm.normX, y: norm.normY, button: 'right' });
      } else {
        sendRemoteInput({ action: 'mouse_move', x: norm.normX, y: norm.normY });
      }
      return;
    }

    // Touch movement
    if (pointerDownPos && !longPressTriggered) {
      const dist = Math.hypot(e.clientX - pointerDownPos.clientX, e.clientY - pointerDownPos.clientY);
      if (dist > tapDistanceThresholdPx) {
        clearLongPress();

        if (!isDragging) {
          isDragging = true;
          // Send mousedown at starting point when transition to drag begins
          const startNorm = getNormalized(pointerDownPos);
          if (startNorm) {
            sendRemoteInput({
              action: 'mouse_down',
              x: startNorm.normX,
              y: startNorm.normY,
              button: 'left',
            });
          }
        }

        // Throttle touch drag events to ~100Hz
        if (now - lastMoveTime >= 10) {
          lastMoveTime = now;
          sendRemoteInput({
            action: 'mouse_drag',
            x: norm.normX,
            y: norm.normY,
            button: 'left',
          });
        }
      }
    }
  }

  function onPointerUp(e: PointerEvent) {
    if (!touchControlEnabled.value) return;
    if (e.pointerType === 'touch' && !e.isPrimary) return;

    const norm = getNormalized(e);
    clearLongPress();

    // Physical mouse: direct mouse_up
    if (e.pointerType === 'mouse') {
      if (norm) {
        const button = e.button === 2 ? 'right' : e.button === 1 ? 'middle' : 'left';
        sendRemoteInput({
          action: 'mouse_up',
          x: norm.normX,
          y: norm.normY,
          button,
        });
      }
      pointerDownPos = null;
      return;
    }

    // Touch release handling
    if (longPressTriggered) {
      // Long press already handled as right click
      longPressTriggered = false;
      pointerDownPos = null;
      return;
    }

    if (isDragging) {
      isDragging = false;
      if (norm) {
        sendRemoteInput({
          action: 'mouse_up',
          x: norm.normX,
          y: norm.normY,
          button: 'left',
        });
      }
      pointerDownPos = null;
      return;
    }

    // Single or Double Tap
    if (pointerDownPos && norm && norm.isInside) {
      const now = Date.now();
      const distFromStart = Math.hypot(e.clientX - pointerDownPos.clientX, e.clientY - pointerDownPos.clientY);

      if (distFromStart <= tapDistanceThresholdPx) {
        const isDoubleTap =
          now - lastTapTime < 300 &&
          lastTapPos !== null &&
          Math.hypot(e.clientX - lastTapPos.clientX, e.clientY - lastTapPos.clientY) < 25;

        if (isDoubleTap) {
          sendRemoteInput({
            action: 'mouse_click',
            x: norm.normX,
            y: norm.normY,
            button: 'left',
            click_count: 2,
          });
          lastTapTime = 0;
          lastTapPos = null;
        } else {
          sendRemoteInput({
            action: 'mouse_click',
            x: norm.normX,
            y: norm.normY,
            button: 'left',
            click_count: 1,
          });
          lastTapTime = now;
          lastTapPos = { clientX: e.clientX, clientY: e.clientY };
        }
      }
    }

    pointerDownPos = null;
  }

  function onPointerCancel() {
    clearLongPress();
    if (isDragging && pointerDownPos) {
      const norm = getNormalized(pointerDownPos);
      if (norm) {
        sendRemoteInput({
          action: 'mouse_up',
          x: norm.normX,
          y: norm.normY,
          button: 'left',
        });
      }
    }
    isDragging = false;
    longPressTriggered = false;
    pointerDownPos = null;
  }

  // --- Touch Event Handlers for Two-Finger Scroll ---

  function onTouchStart(e: TouchEvent) {
    if (!touchControlEnabled.value) return;

    if (e.touches.length === 2) {
      // Cancel single finger gestures
      clearLongPress();
      if (isDragging) {
        isDragging = false;
        if (pointerDownPos) {
          const norm = getNormalized(pointerDownPos);
          if (norm) {
            sendRemoteInput({ action: 'mouse_up', x: norm.normX, y: norm.normY, button: 'left' });
          }
        }
      }
      pointerDownPos = null;

      const t0 = e.touches[0];
      const t1 = e.touches[1];
      const midX = (t0.clientX + t1.clientX) / 2;
      const midY = (t0.clientY + t1.clientY) / 2;
      lastTwoFingerPos = { x: midX, y: midY };
      initialPinchDist = Math.hypot(t0.clientX - t1.clientX, t0.clientY - t1.clientY);
      initialPinchScale = zoomScale.value;
      isPinching = false;
      e.preventDefault();
    }
  }

  function onTouchMove(e: TouchEvent) {
    if (!touchControlEnabled.value) return;

    if (e.touches.length === 2 && lastTwoFingerPos) {
      const t0 = e.touches[0];
      const t1 = e.touches[1];
      const midX = (t0.clientX + t1.clientX) / 2;
      const midY = (t0.clientY + t1.clientY) / 2;
      const dist = Math.hypot(t0.clientX - t1.clientX, t0.clientY - t1.clientY);

      const deltaX = midX - lastTwoFingerPos.x;
      const deltaY = midY - lastTwoFingerPos.y;
      lastTwoFingerPos = { x: midX, y: midY };

      const pinchRatio = initialPinchDist > 0 ? dist / initialPinchDist : 1.0;
      if (!isPinching && Math.abs(pinchRatio - 1.0) > 0.06) {
        isPinching = true;
      }

      if (isPinching) {
        const targetScale = Math.min(4.0, Math.max(1.0, initialPinchScale * pinchRatio));
        if (targetScale !== zoomScale.value) {
          const scaleDiff = targetScale / zoomScale.value;
          zoomScale.value = targetScale;
          panX.value = midX - (midX - panX.value) * scaleDiff;
          panY.value = midY - (midY - panY.value) * scaleDiff;
        }
      } else if (zoomScale.value > 1.05) {
        // Pan the zoomed view
        panX.value += deltaX;
        panY.value += deltaY;
      } else {
        // Scroll macOS
        const norm = getNormalized({ clientX: midX, clientY: midY });
        if (norm) {
          sendRemoteInput({
            action: 'scroll',
            x: norm.normX,
            y: norm.normY,
            delta_x: deltaX * 2.0,
            delta_y: deltaY * 2.0,
          });
        }
      }
      e.preventDefault();
    }
  }

  function onTouchEnd(e: TouchEvent) {
    if (!touchControlEnabled.value) return;
    if (e.touches.length < 2) {
      lastTwoFingerPos = null;
      initialPinchDist = 0;
      isPinching = false;
      if (zoomScale.value < 1.05) {
        resetZoom();
      }
    }
  }

  // --- Wheel Event Handler ---

  function onWheel(e: WheelEvent) {
    if (!touchControlEnabled.value) return;

    if (e.ctrlKey) {
      e.preventDefault();
      const zoomFactor = e.deltaY < 0 ? 1.1 : 0.9;
      const targetScale = Math.min(4.0, Math.max(1.0, zoomScale.value * zoomFactor));
      if (targetScale !== zoomScale.value) {
        const scaleDiff = targetScale / zoomScale.value;
        zoomScale.value = targetScale;
        panX.value = e.clientX - (e.clientX - panX.value) * scaleDiff;
        panY.value = e.clientY - (e.clientY - panY.value) * scaleDiff;
      }
      return;
    }

    const norm = getNormalized(e);
    if (!norm || !norm.isInside) return;

    e.preventDefault();
    sendRemoteInput({
      action: 'scroll',
      x: norm.normX,
      y: norm.normY,
      delta_x: e.deltaX,
      delta_y: e.deltaY,
    });
  }

  // --- Context Menu Prevention ---

  function onContextMenu(e: MouseEvent) {
    if (touchControlEnabled.value) {
      // Prevent browser native context menu so remote right-click works cleanly
      e.preventDefault();
    }
  }

  let cleanupListeners: (() => void) | null = null;

  function attachListeners(el: HTMLElement) {
    detachListeners();

    el.addEventListener('pointerdown', onPointerDown, { passive: true });
    el.addEventListener('pointermove', onPointerMove, { passive: true });
    el.addEventListener('pointerup', onPointerUp, { passive: true });
    el.addEventListener('pointercancel', onPointerCancel, { passive: true });

    el.addEventListener('touchstart', onTouchStart, { passive: false });
    el.addEventListener('touchmove', onTouchMove, { passive: false });
    el.addEventListener('touchend', onTouchEnd, { passive: true });
    el.addEventListener('touchcancel', onTouchEnd, { passive: true });

    el.addEventListener('wheel', onWheel, { passive: false });
    el.addEventListener('contextmenu', onContextMenu);

    cleanupListeners = () => {
      el.removeEventListener('pointerdown', onPointerDown);
      el.removeEventListener('pointermove', onPointerMove);
      el.removeEventListener('pointerup', onPointerUp);
      el.removeEventListener('pointercancel', onPointerCancel);

      el.removeEventListener('touchstart', onTouchStart);
      el.removeEventListener('touchmove', onTouchMove);
      el.removeEventListener('touchend', onTouchEnd);
      el.removeEventListener('touchcancel', onTouchEnd);

      el.removeEventListener('wheel', onWheel);
      el.removeEventListener('contextmenu', onContextMenu);
    };
  }

  function detachListeners() {
    if (cleanupListeners) {
      cleanupListeners();
      cleanupListeners = null;
    }
    clearLongPress();
  }

  watch(
    videoEl,
    (el) => {
      if (el) {
        attachListeners(el);
      } else {
        detachListeners();
      }
    },
    { immediate: true, flush: 'post' },
  );

  onBeforeUnmount(() => {
    detachListeners();
  });

  return {
    touchControlEnabled,
    zoomScale,
    panX,
    panY,
    resetZoom,
    sendRemoteInput,
  };
}
