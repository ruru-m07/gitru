import { useReducedMotion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { useAppStore } from "@/store/use-app-store";

import { TAB_MAX_WIDTH_PX, TAB_RESIZE_DURATION_MS } from "./constants";
import { resolveTabStripLayout, type TabStripLayout } from "./tab-strip-layout";

type UseTabStripLayoutOptions = {
  isTabDragInProgress: boolean;
  tabCount: number;
};

const INITIAL_LAYOUT: TabStripLayout = {
  railWidth: 0,
  tabWidth: TAB_MAX_WIDTH_PX,
};

const layoutsMatch = (left: TabStripLayout, right: TabStripLayout) =>
  left.railWidth === right.railWidth && left.tabWidth === right.tabWidth;

export const useTabStripLayout = ({
  isTabDragInProgress,
  tabCount,
}: UseTabStripLayoutOptions) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const controlsRef = useRef<HTMLDivElement>(null);
  const tabCountRef = useRef(tabCount);
  const previousTabCountRef = useRef(tabCount);
  const animationFrameRef = useRef<number | null>(null);
  const animationTimerRef = useRef<number | null>(null);
  const isAnimatingRef = useRef(false);
  const shouldReduceMotion = useReducedMotion();
  const [isHydrated, setIsHydrated] = useState(() =>
    useAppStore.persist.hasHydrated(),
  );
  const [layout, setLayout] = useState(INITIAL_LAYOUT);
  const [shouldAnimateWidth, setShouldAnimateWidth] = useState(false);

  tabCountRef.current = tabCount;

  const applyLayout = (nextLayout: TabStripLayout) => {
    setLayout((currentLayout) =>
      layoutsMatch(currentLayout, nextLayout) ? currentLayout : nextLayout,
    );
  };

  const readLayout = () => {
    const container = containerRef.current;
    const controls = controlsRef.current;

    if (!container || !controls) {
      return null;
    }

    return resolveTabStripLayout({
      containerWidth: container.clientWidth,
      controlsWidth: controls.getBoundingClientRect().width,
      tabCount: tabCountRef.current,
    });
  };

  const stopAnimation = () => {
    if (animationFrameRef.current !== null) {
      window.cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }

    if (animationTimerRef.current !== null) {
      window.clearTimeout(animationTimerRef.current);
      animationTimerRef.current = null;
    }

    isAnimatingRef.current = false;
    setShouldAnimateWidth(false);
  };

  const scheduleAnimationEnd = () => {
    if (animationTimerRef.current !== null) {
      window.clearTimeout(animationTimerRef.current);
    }

    animationTimerRef.current = window.setTimeout(() => {
      animationTimerRef.current = null;
      isAnimatingRef.current = false;
      setShouldAnimateWidth(false);
    }, TAB_RESIZE_DURATION_MS);
  };

  useEffect(() => {
    if (isHydrated) {
      return;
    }

    let hydrationFrame: number | null = null;
    const markHydratedAfterPaint = () => {
      hydrationFrame = window.requestAnimationFrame(() => {
        setIsHydrated(true);
      });
    };

    if (useAppStore.persist.hasHydrated()) {
      markHydratedAfterPaint();

      return () => {
        if (hydrationFrame !== null) {
          window.cancelAnimationFrame(hydrationFrame);
        }
      };
    }

    const unsubscribe = useAppStore.persist.onFinishHydration(
      markHydratedAfterPaint,
    );

    return () => {
      unsubscribe();
      if (hydrationFrame !== null) {
        window.cancelAnimationFrame(hydrationFrame);
      }
    };
  }, [isHydrated]);

  useLayoutEffect(() => {
    const container = containerRef.current;
    const controls = controlsRef.current;

    if (!container || !controls) {
      return;
    }

    const updateForResize = () => {
      stopAnimation();
      const nextLayout = readLayout();

      if (nextLayout) {
        applyLayout(nextLayout);
      }
    };

    updateForResize();

    const observer = new ResizeObserver(updateForResize);
    observer.observe(container);
    observer.observe(controls);

    return () => {
      observer.disconnect();
      stopAnimation();
    };
  }, []);

  useLayoutEffect(() => {
    const didTabCountChange = previousTabCountRef.current !== tabCount;
    previousTabCountRef.current = tabCount;

    if (!didTabCountChange) {
      if (isTabDragInProgress || shouldReduceMotion) {
        stopAnimation();
      }
      return;
    }

    const nextLayout = readLayout();

    if (!nextLayout) {
      return;
    }

    if (!isHydrated || isTabDragInProgress || shouldReduceMotion) {
      stopAnimation();
      applyLayout(nextLayout);
      return;
    }

    if (animationFrameRef.current !== null) {
      window.cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }

    if (isAnimatingRef.current) {
      applyLayout(nextLayout);
      scheduleAnimationEnd();
    } else {
      animationFrameRef.current = window.requestAnimationFrame(() => {
        animationFrameRef.current = null;
        isAnimatingRef.current = true;
        setShouldAnimateWidth(true);
        applyLayout(nextLayout);
        scheduleAnimationEnd();
      });
    }
  }, [isHydrated, isTabDragInProgress, shouldReduceMotion, tabCount]);

  return {
    containerRef,
    controlsRef,
    layout,
    shouldAnimateWidth,
  };
};
