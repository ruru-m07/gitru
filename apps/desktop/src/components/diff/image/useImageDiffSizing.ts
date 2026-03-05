import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ImageDimensions } from "./types";

type Size = { width: number; height: number };

const PREVIEW_MAX_EDGE = 2200;

const fitWithin = (source: Size, bounds: Size): Size => {
  if (source.width <= 0 || source.height <= 0) {
    return { width: 0, height: 0 };
  }

  const scale = Math.min(
    bounds.width / source.width,
    bounds.height / source.height,
    1,
  );
  return {
    width: Math.floor(source.width * scale),
    height: Math.floor(source.height * scale),
  };
};

const clampByMaxEdge = (size: Size): Size => {
  const maxEdge = Math.max(size.width, size.height);
  if (maxEdge <= PREVIEW_MAX_EDGE) {
    return size;
  }

  const factor = PREVIEW_MAX_EDGE / maxEdge;
  return {
    width: Math.floor(size.width * factor),
    height: Math.floor(size.height * factor),
  };
};

export function useImageDiffSizing() {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [containerSize, setContainerSize] = useState<Size>({
    width: 0,
    height: 0,
  });
  const [beforeDimensions, setBeforeDimensions] =
    useState<ImageDimensions | null>(null);
  const [afterDimensions, setAfterDimensions] =
    useState<ImageDimensions | null>(null);

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;

    const update = () => {
      setContainerSize({
        width: node.clientWidth,
        height: node.clientHeight,
      });
    };

    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const baseDimensions = useMemo(() => {
    const w = Math.max(
      beforeDimensions?.width ?? 0,
      afterDimensions?.width ?? 0,
    );
    const h = Math.max(
      beforeDimensions?.height ?? 0,
      afterDimensions?.height ?? 0,
    );
    if (w === 0 || h === 0) {
      return { width: 0, height: 0 };
    }
    return { width: w, height: h };
  }, [beforeDimensions, afterDimensions]);

  const previewSize = useMemo(() => {
    if (baseDimensions.width === 0 || baseDimensions.height === 0) {
      return { width: 0, height: 0 };
    }

    const downscaled = clampByMaxEdge(baseDimensions);
    return fitWithin(downscaled, {
      width: Math.max(containerSize.width - 24, 1),
      height: Math.max(containerSize.height - 24, 1),
    });
  }, [baseDimensions, containerSize]);

  const onBeforeImageLoad = useCallback((img: HTMLImageElement) => {
    const width = img.naturalWidth;
    const height = img.naturalHeight;
    if (width <= 0 || height <= 0) return;

    setBeforeDimensions((prev) => {
      if (prev?.width === width && prev?.height === height) {
        return prev;
      }
      return { width, height };
    });
  }, []);

  const onAfterImageLoad = useCallback((img: HTMLImageElement) => {
    const width = img.naturalWidth;
    const height = img.naturalHeight;
    if (width <= 0 || height <= 0) return;

    setAfterDimensions((prev) => {
      if (prev?.width === width && prev?.height === height) {
        return prev;
      }
      return { width, height };
    });
  }, []);

  const resetDimensions = useCallback(() => {
    setBeforeDimensions(null);
    setAfterDimensions(null);
  }, []);

  return {
    containerRef,
    previewSize,
    beforeDimensions,
    afterDimensions,
    onBeforeImageLoad,
    onAfterImageLoad,
    resetDimensions,
  };
}
