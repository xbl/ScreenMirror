export interface RenderBox {
  renderedWidth: number;
  renderedHeight: number;
  offsetX: number;
  offsetY: number;
}

export interface NormalizedPoint {
  normX: number;
  normY: number;
  isInside: boolean;
}

/**
 * Computes the active render box of content rendered with object-fit inside a container.
 */
export function computeRenderBox(
  containerWidth: number,
  containerHeight: number,
  contentWidth: number,
  contentHeight: number,
  objectFit: 'contain' | 'fill' | 'cover' = 'contain',
): RenderBox {
  if (containerWidth <= 0 || containerHeight <= 0 || contentWidth <= 0 || contentHeight <= 0) {
    return { renderedWidth: 0, renderedHeight: 0, offsetX: 0, offsetY: 0 };
  }

  if (objectFit === 'fill') {
    return {
      renderedWidth: containerWidth,
      renderedHeight: containerHeight,
      offsetX: 0,
      offsetY: 0,
    };
  }

  const containerAspect = containerWidth / containerHeight;
  const contentAspect = contentWidth / contentHeight;

  let renderedWidth = containerWidth;
  let renderedHeight = containerHeight;
  let offsetX = 0;
  let offsetY = 0;

  if (objectFit === 'contain') {
    if (containerAspect > contentAspect) {
      // Pillarboxed (vertical letterbox / bars on left and right)
      renderedHeight = containerHeight;
      renderedWidth = containerHeight * contentAspect;
      offsetX = (containerWidth - renderedWidth) / 2;
    } else {
      // Letterboxed (bars on top and bottom)
      renderedWidth = containerWidth;
      renderedHeight = containerWidth / contentAspect;
      offsetY = (containerHeight - renderedHeight) / 2;
    }
  } else if (objectFit === 'cover') {
    if (containerAspect > contentAspect) {
      renderedWidth = containerWidth;
      renderedHeight = containerWidth / contentAspect;
      offsetY = (containerHeight - renderedHeight) / 2;
    } else {
      renderedHeight = containerHeight;
      renderedWidth = containerHeight * contentAspect;
      offsetX = (containerWidth - renderedWidth) / 2;
    }
  }

  return { renderedWidth, renderedHeight, offsetX, offsetY };
}

/**
 * Translates client pointer coordinates (e.g. from mouse/touch event) into
 * normalized coordinates [0.0..1.0] relative to the actual rendered content box.
 */
export function clientToNormalized(
  clientX: number,
  clientY: number,
  boundingRect: { left: number; top: number; width: number; height: number },
  contentWidth: number,
  contentHeight: number,
  objectFit: 'contain' | 'fill' | 'cover' = 'contain',
): NormalizedPoint | null {
  const box = computeRenderBox(
    boundingRect.width,
    boundingRect.height,
    contentWidth,
    contentHeight,
    objectFit,
  );

  if (box.renderedWidth <= 0 || box.renderedHeight <= 0) {
    return null;
  }

  const pointerX = clientX - boundingRect.left;
  const pointerY = clientY - boundingRect.top;

  const videoX = pointerX - box.offsetX;
  const videoY = pointerY - box.offsetY;

  const rawNormX = videoX / box.renderedWidth;
  const rawNormY = videoY / box.renderedHeight;

  const isInside = rawNormX >= 0 && rawNormX <= 1 && rawNormY >= 0 && rawNormY <= 1;
  const normX = Math.max(0, Math.min(1, rawNormX));
  const normY = Math.max(0, Math.min(1, rawNormY));

  return { normX, normY, isInside };
}

/**
 * Translates normalized coordinates [0.0..1.0] back to client pointer coordinates.
 */
export function normalizedToClient(
  normX: number,
  normY: number,
  boundingRect: { left: number; top: number; width: number; height: number },
  contentWidth: number,
  contentHeight: number,
  objectFit: 'contain' | 'fill' | 'cover' = 'contain',
): { clientX: number; clientY: number } | null {
  const box = computeRenderBox(
    boundingRect.width,
    boundingRect.height,
    contentWidth,
    contentHeight,
    objectFit,
  );

  if (box.renderedWidth <= 0 || box.renderedHeight <= 0) {
    return null;
  }

  const clampedX = Math.max(0, Math.min(1, normX));
  const clampedY = Math.max(0, Math.min(1, normY));

  const clientX = boundingRect.left + box.offsetX + clampedX * box.renderedWidth;
  const clientY = boundingRect.top + box.offsetY + clampedY * box.renderedHeight;

  return { clientX, clientY };
}
