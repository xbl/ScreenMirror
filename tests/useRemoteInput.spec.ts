// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import { useRemoteInput } from '../viewer/src/lib/useRemoteInput';
import { useRemoteControl } from '../viewer/src/lib/remoteControl';

// JSDOM does not provide PointerEvent by default; polyfill for tests
if (typeof globalThis.PointerEvent === 'undefined') {
  class MockPointerEvent extends MouseEvent {
    pointerType: string;
    isPrimary: boolean;
    constructor(type: string, params: any = {}) {
      super(type, params);
      this.pointerType = params.pointerType || 'mouse';
      this.isPrimary = params.isPrimary ?? true;
    }
  }
  globalThis.PointerEvent = MockPointerEvent as any;
}

describe('useRemoteInput - Touch Isolation and Decoupled Zoom', () => {
  let mockVideo: HTMLVideoElement;
  let sentEvents: any[] = [];

  beforeEach(() => {
    sentEvents = [];

    // Mock RemoteControl
    const { setTouchControlEnabled, setControlChannel } = useRemoteControl();
    setTouchControlEnabled(false); // default OFF

    const mockDc = {
      readyState: 'open',
      send: vi.fn((data: string) => {
        sentEvents.push(JSON.parse(data));
      }),
    } as unknown as RTCDataChannel;
    setControlChannel(mockDc);

    // Mock video element
    mockVideo = document.createElement('video');
    Object.defineProperty(mockVideo, 'videoWidth', { value: 1920, configurable: true });
    Object.defineProperty(mockVideo, 'videoHeight', { value: 1080, configurable: true });
    mockVideo.getBoundingClientRect = () => ({
      left: 0,
      top: 0,
      right: 390,
      bottom: 844,
      width: 390,
      height: 844,
      x: 0,
      y: 0,
      toJSON: () => {},
    });
  });

  it('allows pinch-to-zoom when touch control is OFF', () => {
    const videoRef = ref<HTMLVideoElement | null>(mockVideo);
    const { zoomScale } = useRemoteInput({ videoEl: videoRef });

    expect(zoomScale.value).toBe(1.0);

    // Two-finger touch start (distance = 100px)
    const touchStartEv = new TouchEvent('touchstart', {
      touches: [
        { clientX: 100, clientY: 200 } as Touch,
        { clientX: 200, clientY: 200 } as Touch,
      ],
      cancelable: true,
    });
    mockVideo.dispatchEvent(touchStartEv);

    // Two-finger touch move: pinch out to 200px (2.0x zoom)
    const touchMoveEv = new TouchEvent('touchmove', {
      touches: [
        { clientX: 50, clientY: 200 } as Touch,
        { clientX: 250, clientY: 200 } as Touch,
      ],
      cancelable: true,
    });
    mockVideo.dispatchEvent(touchMoveEv);

    expect(zoomScale.value).toBe(2.0);
    // Remote events must NEVER be sent when touch control is OFF
    expect(sentEvents).toHaveLength(0);

    // Two-finger touch end
    const touchEndEv = new TouchEvent('touchend', {
      touches: [],
    });
    mockVideo.dispatchEvent(touchEndEv);
    expect(zoomScale.value).toBe(2.0);
  });

  it('allows single-finger panning when zoomed in and touch control is OFF', () => {
    const videoRef = ref<HTMLVideoElement | null>(mockVideo);
    const { zoomScale, panX, panY } = useRemoteInput({ videoEl: videoRef });

    zoomScale.value = 2.0;

    // Single finger touch down
    const startEv = new TouchEvent('touchstart', {
      touches: [{ clientX: 100, clientY: 100 } as Touch],
    });
    mockVideo.dispatchEvent(startEv);

    // Single finger drag by +50, +30
    const moveEv = new TouchEvent('touchmove', {
      touches: [{ clientX: 150, clientY: 130 } as Touch],
      cancelable: true,
    });
    mockVideo.dispatchEvent(moveEv);

    expect(panX.value).toBe(50);
    expect(panY.value).toBe(30);
    expect(sentEvents).toHaveLength(0);
  });

  it('guarantees pinch gesture NEVER sends mouse_down or mouse_click even when touch control is ON', () => {
    const { setTouchControlEnabled } = useRemoteControl();
    setTouchControlEnabled(true); // Turn ON touch control

    const videoRef = ref<HTMLVideoElement | null>(mockVideo);
    const { zoomScale } = useRemoteInput({ videoEl: videoRef });

    // Finger 1 touches down first (pointerdown + touchstart)
    const pDown1 = new PointerEvent('pointerdown', {
      clientX: 100,
      clientY: 200,
      pointerType: 'touch',
      isPrimary: true,
    });
    mockVideo.dispatchEvent(pDown1);

    const tStart1 = new TouchEvent('touchstart', {
      touches: [{ clientX: 100, clientY: 200 } as Touch],
    });
    mockVideo.dispatchEvent(tStart1);

    // 20ms later, Finger 2 touches down -> Entering multi-touch pinch
    const tStart2 = new TouchEvent('touchstart', {
      touches: [
        { clientX: 100, clientY: 200 } as Touch,
        { clientX: 200, clientY: 200 } as Touch,
      ],
      cancelable: true,
    });
    mockVideo.dispatchEvent(tStart2);

    // Both fingers move during pinch
    const tMove = new TouchEvent('touchmove', {
      touches: [
        { clientX: 80, clientY: 200 } as Touch,
        { clientX: 220, clientY: 200 } as Touch,
      ],
      cancelable: true,
    });
    mockVideo.dispatchEvent(tMove);

    // Pointer move fires for finger 1
    const pMove1 = new PointerEvent('pointermove', {
      clientX: 80,
      clientY: 200,
      pointerType: 'touch',
      isPrimary: true,
    });
    mockVideo.dispatchEvent(pMove1);

    // Fingers lift
    const tEnd = new TouchEvent('touchend', {
      touches: [],
    });
    mockVideo.dispatchEvent(tEnd);

    // Pointerup fires on finger 1
    const pUp1 = new PointerEvent('pointerup', {
      clientX: 80,
      clientY: 200,
      pointerType: 'touch',
      isPrimary: true,
    });
    mockVideo.dispatchEvent(pUp1);

    // CRITICAL ASSERTION: Pinch gesture must NEVER have sent any mouse_down, mouse_drag, or mouse_click to the computer!
    const mouseActions = sentEvents.filter(
      (e) => e.action === 'mouse_down' || e.action === 'mouse_click' || e.action === 'mouse_drag',
    );
    expect(mouseActions).toHaveLength(0);
    expect(zoomScale.value).toBeGreaterThan(1.0);
  });

  it('sends single-finger tap as mouse_click when touch control is ON and no pinch occurs', () => {
    const { setTouchControlEnabled } = useRemoteControl();
    setTouchControlEnabled(true);

    const videoRef = ref<HTMLVideoElement | null>(mockVideo);
    useRemoteInput({ videoEl: videoRef });

    // Single finger tap: down and up with minimal movement inside the 16:9 letterbox (Y between 313 and 531)
    const pDown = new PointerEvent('pointerdown', {
      clientX: 150,
      clientY: 420,
      pointerType: 'touch',
      isPrimary: true,
    });
    mockVideo.dispatchEvent(pDown);

    const pUp = new PointerEvent('pointerup', {
      clientX: 152,
      clientY: 421,
      pointerType: 'touch',
      isPrimary: true,
    });
    mockVideo.dispatchEvent(pUp);

    expect(sentEvents).toHaveLength(1);
    expect(sentEvents[0].action).toBe('mouse_click');
    expect(sentEvents[0].button).toBe('left');
    expect(sentEvents[0].click_count).toBe(1);
  });
});
