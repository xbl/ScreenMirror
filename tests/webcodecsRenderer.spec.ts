import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  isWebCodecsSupported,
  WebCodecsRenderer,
  type WindowWithWebCodecs,
} from '../viewer/src/lib/webcodecsRenderer';

class MockVideoFrame {
  displayWidth: number;
  displayHeight: number;
  codedWidth: number;
  codedHeight: number;
  closed = false;

  constructor(width = 1920, height = 1080) {
    this.displayWidth = width;
    this.displayHeight = height;
    this.codedWidth = width;
    this.codedHeight = height;
  }

  close() {
    this.closed = true;
  }
}

class MockMediaStreamTrackProcessor {
  public readable: ReadableStream<MockVideoFrame>;
  public track: MediaStreamTrack;

  constructor(
    { track }: { track: MediaStreamTrack },
    readable?: ReadableStream<MockVideoFrame>,
  ) {
    this.track = track;
    this.readable =
      readable ||
      new ReadableStream<MockVideoFrame>({
        start() {},
      });
  }
}

describe('WebCodecsRenderer & isWebCodecsSupported', () => {
  let originalProcessor: unknown;
  let originalVideoFrame: unknown;
  const win = window as unknown as WindowWithWebCodecs;

  beforeEach(() => {
    originalProcessor = win.MediaStreamTrackProcessor;
    originalVideoFrame = win.VideoFrame;
  });

  afterEach(() => {
    win.MediaStreamTrackProcessor = originalProcessor as typeof win.MediaStreamTrackProcessor;
    win.VideoFrame = originalVideoFrame as typeof win.VideoFrame;
  });

  describe('Feature Detection', () => {
    it('returns false when both MediaStreamTrackProcessor and VideoFrame are absent', () => {
      delete win.MediaStreamTrackProcessor;
      delete win.VideoFrame;

      expect(isWebCodecsSupported()).toBe(false);

      const renderer = new WebCodecsRenderer();
      expect(renderer.isSupported).toBe(false);
    });

    it('returns false when only MediaStreamTrackProcessor is present', () => {
      win.MediaStreamTrackProcessor = MockMediaStreamTrackProcessor as unknown as typeof win.MediaStreamTrackProcessor;
      delete win.VideoFrame;

      expect(isWebCodecsSupported()).toBe(false);
    });

    it('returns false when only VideoFrame is present', () => {
      delete win.MediaStreamTrackProcessor;
      win.VideoFrame = MockVideoFrame as unknown as typeof VideoFrame;

      expect(isWebCodecsSupported()).toBe(false);
    });

    it('returns true when both MediaStreamTrackProcessor and VideoFrame are present', () => {
      win.MediaStreamTrackProcessor = MockMediaStreamTrackProcessor as unknown as typeof win.MediaStreamTrackProcessor;
      win.VideoFrame = MockVideoFrame as unknown as typeof VideoFrame;

      expect(isWebCodecsSupported()).toBe(true);

      const renderer = new WebCodecsRenderer();
      expect(renderer.isSupported).toBe(true);
    });
  });

  describe('Validation & Start Constraints', () => {
    it('throws when start() is called in an unsupported environment', async () => {
      delete win.MediaStreamTrackProcessor;
      delete win.VideoFrame;

      const renderer = new WebCodecsRenderer();
      const mockTrack = { readyState: 'live', addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaStreamTrack;
      const mockCanvas = document.createElement('canvas');

      await expect(renderer.start(mockTrack, mockCanvas)).rejects.toThrow(
        'WebCodecs is not supported',
      );
    });

    it('throws when start() is called with an invalid track or canvas', async () => {
      win.MediaStreamTrackProcessor = MockMediaStreamTrackProcessor as unknown as typeof win.MediaStreamTrackProcessor;
      win.VideoFrame = MockVideoFrame as unknown as typeof VideoFrame;

      const renderer = new WebCodecsRenderer();
      const mockCanvas = document.createElement('canvas');
      const mockTrack = { readyState: 'live', addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaStreamTrack;

      await expect(renderer.start(null as unknown as MediaStreamTrack, mockCanvas)).rejects.toThrow(
        'valid MediaStreamTrack is required',
      );
      await expect(renderer.start(mockTrack, null as unknown as HTMLCanvasElement)).rejects.toThrow(
        'valid HTMLCanvasElement is required',
      );
    });

    it('throws when start() is called with an ended track', async () => {
      win.MediaStreamTrackProcessor = MockMediaStreamTrackProcessor as unknown as typeof win.MediaStreamTrackProcessor;
      win.VideoFrame = MockVideoFrame as unknown as typeof VideoFrame;

      const renderer = new WebCodecsRenderer();
      const mockTrack = { readyState: 'ended', addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaStreamTrack;
      const mockCanvas = document.createElement('canvas');

      await expect(renderer.start(mockTrack, mockCanvas)).rejects.toThrow(
        'cannot start with an ended track',
      );
    });

    it('throws when canvas 2D context cannot be acquired', async () => {
      win.MediaStreamTrackProcessor = MockMediaStreamTrackProcessor as unknown as typeof win.MediaStreamTrackProcessor;
      win.VideoFrame = MockVideoFrame as unknown as typeof VideoFrame;

      const renderer = new WebCodecsRenderer();
      const mockTrack = { readyState: 'live', addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaStreamTrack;
      const mockCanvas = {
        getContext: vi.fn().mockReturnValue(null),
      } as unknown as HTMLCanvasElement;

      await expect(renderer.start(mockTrack, mockCanvas)).rejects.toThrow(
        'failed to acquire 2D canvas context',
      );
    });
  });

  describe('Lifecycle & Frame Rendering', () => {
    beforeEach(() => {
      win.MediaStreamTrackProcessor = MockMediaStreamTrackProcessor as unknown as typeof win.MediaStreamTrackProcessor;
      win.VideoFrame = MockVideoFrame as unknown as typeof VideoFrame;
    });

    it('requests desynchronized 2D context and starts reading frames', async () => {
      let streamController!: ReadableStreamDefaultController<MockVideoFrame>;
      const stream = new ReadableStream<MockVideoFrame>({
        start(controller) {
          streamController = controller;
        },
      });

      win.MediaStreamTrackProcessor = class {
        readable = stream;
        track: MediaStreamTrack;
        constructor({ track }: { track: MediaStreamTrack }) {
          this.track = track;
        }
      } as unknown as typeof win.MediaStreamTrackProcessor;

      const mockCtx = {
        drawImage: vi.fn(),
      };
      const mockCanvas = {
        width: 0,
        height: 0,
        getContext: vi.fn().mockReturnValue(mockCtx),
      } as unknown as HTMLCanvasElement;

      const mockTrack = {
        readyState: 'live',
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      } as unknown as MediaStreamTrack;

      const renderer = new WebCodecsRenderer();
      expect(renderer.isRunning).toBe(false);
      expect(renderer.frameCount).toBe(0);

      await renderer.start(mockTrack, mockCanvas);

      expect(mockCanvas.getContext).toHaveBeenCalledWith('2d', {
        alpha: false,
        desynchronized: true,
      });
      expect(renderer.isRunning).toBe(true);

      // Enqueue frame 1: 1920x1080
      const frame1 = new MockVideoFrame(1920, 1080);
      streamController.enqueue(frame1);

      // Allow microtask ticks for read loop
      await new Promise((r) => setTimeout(r, 10));

      expect(mockCanvas.width).toBe(1920);
      expect(mockCanvas.height).toBe(1080);
      expect(mockCtx.drawImage).toHaveBeenCalledWith(frame1, 0, 0, 1920, 1080);
      expect(frame1.closed).toBe(true);
      expect(renderer.frameCount).toBe(1);

      // Enqueue frame 2 with new dimensions: 1280x720
      const frame2 = new MockVideoFrame(1280, 720);
      streamController.enqueue(frame2);

      await new Promise((r) => setTimeout(r, 10));

      expect(mockCanvas.width).toBe(1280);
      expect(mockCanvas.height).toBe(720);
      expect(mockCtx.drawImage).toHaveBeenCalledWith(frame2, 0, 0, 1280, 720);
      expect(frame2.closed).toBe(true);
      expect(renderer.frameCount).toBe(2);

      // Stop renderer
      await renderer.stop();
      expect(renderer.isRunning).toBe(false);
    });

    it('ensures frame.close() is called even if ctx.drawImage throws', async () => {
      let streamController!: ReadableStreamDefaultController<MockVideoFrame>;
      const stream = new ReadableStream<MockVideoFrame>({
        start(controller) {
          streamController = controller;
        },
      });

      win.MediaStreamTrackProcessor = class {
        readable = stream;
        track: MediaStreamTrack;
        constructor({ track }: { track: MediaStreamTrack }) {
          this.track = track;
        }
      } as unknown as typeof win.MediaStreamTrackProcessor;

      const mockCtx = {
        drawImage: vi.fn().mockImplementation(() => {
          throw new Error('Canvas draw failure');
        }),
      };
      const mockCanvas = {
        width: 100,
        height: 100,
        getContext: vi.fn().mockReturnValue(mockCtx),
      } as unknown as HTMLCanvasElement;

      const mockTrack = {
        readyState: 'live',
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      } as unknown as MediaStreamTrack;

      const renderer = new WebCodecsRenderer();
      await renderer.start(mockTrack, mockCanvas);

      const frame = new MockVideoFrame(800, 600);
      streamController.enqueue(frame);

      await new Promise((r) => setTimeout(r, 10));

      expect(frame.closed).toBe(true);
      await renderer.stop();
    });

    it('handles stream error and triggers onError callback', async () => {
      const readError = new Error('Stream read failed');
      const stream = new ReadableStream<MockVideoFrame>({
        start(controller) {
          controller.error(readError);
        },
      });

      win.MediaStreamTrackProcessor = class {
        readable = stream;
        track: MediaStreamTrack;
        constructor({ track }: { track: MediaStreamTrack }) {
          this.track = track;
        }
      } as unknown as typeof win.MediaStreamTrackProcessor;

      const mockCtx = { drawImage: vi.fn() };
      const mockCanvas = {
        width: 0,
        height: 0,
        getContext: vi.fn().mockReturnValue(mockCtx),
      } as unknown as HTMLCanvasElement;
      const mockTrack = {
        readyState: 'live',
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      } as unknown as MediaStreamTrack;

      const renderer = new WebCodecsRenderer();
      const errorHandler = vi.fn();
      renderer.onError = errorHandler;

      await renderer.start(mockTrack, mockCanvas);

      await new Promise((r) => setTimeout(r, 10));

      expect(errorHandler).toHaveBeenCalledWith(readError);
      expect(renderer.isRunning).toBe(false);
    });

    it('stops automatically when track emits "ended" event', async () => {
      let endedListener: (() => void) | null = null;
      const mockTrack = {
        readyState: 'live',
        addEventListener: vi.fn((event, listener) => {
          if (event === 'ended') endedListener = listener as () => void;
        }),
        removeEventListener: vi.fn(),
      } as unknown as MediaStreamTrack;

      const mockCtx = { drawImage: vi.fn() };
      const mockCanvas = {
        width: 0,
        height: 0,
        getContext: vi.fn().mockReturnValue(mockCtx),
      } as unknown as HTMLCanvasElement;

      const renderer = new WebCodecsRenderer();
      await renderer.start(mockTrack, mockCanvas);
      expect(renderer.isRunning).toBe(true);

      // Trigger track ended
      expect(endedListener).not.toBeNull();
      endedListener!();

      expect(renderer.isRunning).toBe(false);
    });

    it('calling stop() when not running is safe and idempotent', async () => {
      const renderer = new WebCodecsRenderer();
      expect(renderer.isRunning).toBe(false);
      await renderer.stop();
      expect(renderer.isRunning).toBe(false);
    });
  });
});
