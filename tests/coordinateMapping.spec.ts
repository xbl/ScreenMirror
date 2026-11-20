import { describe, it, expect } from 'vitest';
import {
  computeRenderBox,
  clientToNormalized,
  normalizedToClient,
} from '../viewer/src/lib/coordinateMapping';

describe('coordinateMapping', () => {
  describe('computeRenderBox', () => {
    it('returns zero render box when dimensions are zero or negative', () => {
      expect(computeRenderBox(0, 0, 1920, 1080)).toEqual({
        renderedWidth: 0,
        renderedHeight: 0,
        offsetX: 0,
        offsetY: 0,
      });
      expect(computeRenderBox(1920, 1080, 0, 0)).toEqual({
        renderedWidth: 0,
        renderedHeight: 0,
        offsetX: 0,
        offsetY: 0,
      });
    });

    it('matches container exactly when aspect ratios match', () => {
      const box = computeRenderBox(1920, 1080, 1920, 1080, 'contain');
      expect(box).toEqual({
        renderedWidth: 1920,
        renderedHeight: 1080,
        offsetX: 0,
        offsetY: 0,
      });
    });

    it('calculates pillarbox (bars on left/right) when container is wider than content', () => {
      // Container: 1920x1080 (16:9 ~ 1.777)
      // Content: 1080x1080 (1:1 = 1.0)
      const box = computeRenderBox(1920, 1080, 1080, 1080, 'contain');
      expect(box.renderedHeight).toBe(1080);
      expect(box.renderedWidth).toBe(1080);
      expect(box.offsetX).toBe((1920 - 1080) / 2); // 420
      expect(box.offsetY).toBe(0);
    });

    it('calculates letterbox (bars on top/bottom) when container is taller than content', () => {
      // Container: 1000x1000 (1:1)
      // Content: 1920x1080 (16:9)
      const box = computeRenderBox(1000, 1000, 1920, 1080, 'contain');
      expect(box.renderedWidth).toBe(1000);
      expect(box.renderedHeight).toBeCloseTo(1000 / (1920 / 1080), 2);
      expect(box.offsetX).toBe(0);
      expect(box.offsetY).toBeCloseTo((1000 - box.renderedHeight) / 2, 2);
    });
  });

  describe('clientToNormalized', () => {
    const rect = { left: 100, top: 50, width: 1920, height: 1080 };

    it('normalizes center point accurately', () => {
      // Center of element: left + 1920/2 = 1060, top + 1080/2 = 590
      const point = clientToNormalized(1060, 590, rect, 1920, 1080, 'contain');
      expect(point).not.toBeNull();
      expect(point?.normX).toBeCloseTo(0.5, 4);
      expect(point?.normY).toBeCloseTo(0.5, 4);
      expect(point?.isInside).toBe(true);
    });

    it('normalizes corners accurately', () => {
      const topLeft = clientToNormalized(100, 50, rect, 1920, 1080, 'contain');
      expect(topLeft?.normX).toBe(0);
      expect(topLeft?.normY).toBe(0);
      expect(topLeft?.isInside).toBe(true);

      const bottomRight = clientToNormalized(2020, 1130, rect, 1920, 1080, 'contain');
      expect(bottomRight?.normX).toBe(1);
      expect(bottomRight?.normY).toBe(1);
      expect(bottomRight?.isInside).toBe(true);
    });

    it('flags points in letterbox/pillarbox bars as not inside and clamps them', () => {
      // Square content (1080x1080) inside 1920x1080 container -> offsetX = 420
      // Click at clientX = 150 (in the left bar, offset is 50px into rect, but offsetX is 420px)
      const pointInBar = clientToNormalized(150, 590, rect, 1080, 1080, 'contain');
      expect(pointInBar).not.toBeNull();
      expect(pointInBar?.isInside).toBe(false);
      expect(pointInBar?.normX).toBe(0); // clamped to 0
      expect(pointInBar?.normY).toBeCloseTo(0.5, 4);
    });
  });

  describe('normalizedToClient', () => {
    const rect = { left: 100, top: 50, width: 1920, height: 1080 };

    it('translates normalized center back to client coordinates', () => {
      const client = normalizedToClient(0.5, 0.5, rect, 1920, 1080, 'contain');
      expect(client?.clientX).toBeCloseTo(1060, 2);
      expect(client?.clientY).toBeCloseTo(590, 2);
    });

    it('translates corners back to client coordinates with pillarboxing', () => {
      // 1080x1080 content in 1920x1080 container -> offsetX = 420
      const clientTL = normalizedToClient(0, 0, rect, 1080, 1080, 'contain');
      expect(clientTL?.clientX).toBe(100 + 420);
      expect(clientTL?.clientY).toBe(50);

      const clientBR = normalizedToClient(1, 1, rect, 1080, 1080, 'contain');
      expect(clientBR?.clientX).toBe(100 + 420 + 1080);
      expect(clientBR?.clientY).toBe(50 + 1080);
    });
  });
});
