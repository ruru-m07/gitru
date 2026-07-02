import { useEffect, useId } from "react";
import {
  clearCardSearchHighlight,
  setCardSearchHighlight,
} from "@/lib/searchTextHighlight";

export function useSearchTextHighlight(
  containerRef: React.RefObject<HTMLElement | null>,
  query: string,
  isRegex: boolean,
  enabled = true,
) {
  const cardId = useId();

  useEffect(() => {
    if (!enabled) {
      clearCardSearchHighlight(cardId);
      return;
    }

    const container = containerRef.current;
    if (!container || !query.trim()) {
      clearCardSearchHighlight(cardId);
      return;
    }

    let frame = 0;
    const observedShadowRoots = new WeakSet<ShadowRoot>();
    const retryTimeouts: number[] = [];

    const observeShadowRoots = () => {
      for (const host of container.querySelectorAll("diffs-container")) {
        const shadowRoot = host.shadowRoot;
        if (!shadowRoot || observedShadowRoots.has(shadowRoot)) {
          continue;
        }

        observedShadowRoots.add(shadowRoot);
        observer.observe(shadowRoot, {
          childList: true,
          subtree: true,
          characterData: true,
        });
      }
    };

    const apply = () => {
      if (!containerRef.current) {
        return;
      }

      observeShadowRoots();
      setCardSearchHighlight(
        cardId,
        containerRef.current,
        query,
        isRegex,
      );
    };

    const scheduleApply = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(apply);
    };

    const observer = new MutationObserver(scheduleApply);
    observer.observe(container, {
      childList: true,
      subtree: true,
      characterData: true,
    });

    scheduleApply();

    for (const delay of [250, 750, 1500]) {
      retryTimeouts.push(window.setTimeout(scheduleApply, delay));
    }

    return () => {
      cancelAnimationFrame(frame);
      for (const timeoutId of retryTimeouts) {
        window.clearTimeout(timeoutId);
      }
      observer.disconnect();
      clearCardSearchHighlight(cardId);
    };
  }, [cardId, containerRef, enabled, isRegex, query]);
}