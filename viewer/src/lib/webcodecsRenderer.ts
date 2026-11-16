/**
 * WebCodecs + Canvas hardware direct rendering engine for low-latency video streaming.
 * Provides direct frame-by-frame rendering from MediaStreamTrackProcessor to an HTML5 Canvas.
 */

export interface MediaStreamTrackProcessorOptions {
  track: MediaStreamTrack;
  maxBufferSize?: number;
}

export interface MediaStreamTrackProcessorType<T = VideoFrame> {
  readonly readable: ReadableStream<T>;
}

export interface WindowWithWebCodecs extends Window {
  MediaStreamTrackProcessor?: {
    new (options: MediaStreamTrackProcessorOptions): MediaStreamTrackProcessorType<VideoFrame>;
  };
  VideoFrame?: typeof VideoFrame;
}

/**
 * Checks whether WebCodecs and MediaStreamTrackProcessor are supported in the current environment.
 */
export function isWebCodecsSupported(): boolean {
  if (typeof window === 'undefined') return false;
  const win = window as unknown as WindowWithWebCodecs;
  return (
    typeof win.MediaStreamTrackProcessor !== 'undefined' &&
    typeof win.VideoFrame !== 'undefined'
  );
}

export class WebCodecsRenderer {
  private _isRunning = false;
  private _frameCount = 0;
  private _reader: ReadableStreamDefaultReader<VideoFrame> | null = null;
  private _canvas: HTMLCanvasElement | null = null;
  private _ctx: CanvasRenderingContext2D | null = null;
  private _loopPromise: Promise<void> | null = null;
  private _track: MediaStreamTrack | null = null;
  private _onTrackEnded: (() => void) | null = null;

  public onError?: (error: unknown) => void;
  public onFrame?: (count: number, frame: VideoFrame) => void;

  /**
   * Whether WebCodecs is supported in the current browser/environment.
   */
  public get isSupported(): boolean {
    return isWebCodecsSupported();
  }

  /**
   * Whether the renderer is currently processing and rendering frames.
   */
  public get isRunning(): boolean {
    return this._isRunning;
  }

  /**
   * Number of frames successfully rendered to canvas.
   */
  public get frameCount(): number {
    return this._frameCount;
  }

  /**
   * Starts reading VideoFrames from the provided MediaStreamTrack and rendering to canvas.
   */
  public async start(track: MediaStreamTrack, canvas: HTMLCanvasElement): Promise<void> {
    if (!track) {
      throw new Error('WebCodecsRenderer: a valid MediaStreamTrack is required');
    }
    if (!canvas) {
      throw new Error('WebCodecsRenderer: a valid HTMLCanvasElement is required');
    }
    if (track.readyState === 'ended') {
      throw new Error('WebCodecsRenderer: cannot start with an ended track');
    }
    if (!isWebCodecsSupported()) {
      throw new Error('WebCodecs is not supported in this environment');
    }

    // Stop any existing rendering loop before starting a new one
    if (this._isRunning) {
      await this.stop();
    }

    const ctx =
      canvas.getContext('2d', { alpha: false, desynchronized: true }) ||
      canvas.getContext('2d');
    if (!ctx) {
      throw new Error('WebCodecsRenderer: failed to acquire 2D canvas context');
    }

    const win = window as unknown as WindowWithWebCodecs;
    const Processor = win.MediaStreamTrackProcessor;
    if (!Processor) {
      throw new Error('WebCodecsRenderer: MediaStreamTrackProcessor is unavailable');
    }

    const processor = new Processor({ track });
    const reader = processor.readable.getReader();

    this._canvas = canvas;
    this._ctx = ctx;
    this._reader = reader;
    this._frameCount = 0;
    this._isRunning = true;

    // Track ended listener
    const onEnded = () => {
      void this.stop();
    };
    track.addEventListener('ended', onEnded, { once: true });
    this._track = track;
    this._onTrackEnded = onEnded;

    // Start read loop
    this._loopPromise = this.readLoop(reader, canvas, ctx);
  }

  /**
   * Internal loop reading frames from the stream reader and drawing to canvas.
   */
  private async readLoop(
    reader: ReadableStreamDefaultReader<VideoFrame>,
    canvas: HTMLCanvasElement,
    ctx: CanvasRenderingContext2D,
  ): Promise<void> {
    try {
      while (this._isRunning) {
        const { done, value: frame } = await reader.read();
        if (done) {
          break;
        }
        if (!frame) {
          continue;
        }

        if (!this._isRunning) {
          try {
            frame.close();
          } catch {
            // ignore
          }
          break;
        }

        try {
          const width = frame.displayWidth || frame.codedWidth || canvas.width;
          const height = frame.displayHeight || frame.codedHeight || canvas.height;

          if (width > 0 && height > 0 && (canvas.width !== width || canvas.height !== height)) {
            canvas.width = width;
            canvas.height = height;
          }

          ctx.drawImage(frame, 0, 0, width, height);
          this._frameCount++;

          if (this.onFrame) {
            this.onFrame(this._frameCount, frame);
          }
        } finally {
          try {
            frame.close();
          } catch {
            // ignore if frame is already closed
          }
        }
      }
    } catch (err) {
      if (this._isRunning) {
        console.warn('[webcodecs-renderer] frame read loop error:', err);
        if (this.onError) {
          this.onError(err);
        }
      }
    } finally {
      this._isRunning = false;
    }
  }

  /**
   * Stops the renderer, cancels the stream reader, and cleans up resources.
   */
  public async stop(): Promise<void> {
    if (!this._isRunning && !this._reader) {
      return;
    }

    this._isRunning = false;
    const reader = this._reader;
    this._reader = null;

    if (this._track && this._onTrackEnded) {
      this._track.removeEventListener('ended', this._onTrackEnded);
      this._track = null;
      this._onTrackEnded = null;
    }

    if (reader) {
      try {
        await reader.cancel();
      } catch {
        // ignore cancellation error
      }
      try {
        reader.releaseLock();
      } catch {
        // ignore lock release error
      }
    }

    if (this._loopPromise) {
      try {
        await this._loopPromise;
      } catch {
        // ignore loop shutdown error
      }
      this._loopPromise = null;
    }

    this._canvas = null;
    this._ctx = null;
  }
}
