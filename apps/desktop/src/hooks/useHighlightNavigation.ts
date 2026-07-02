import { useCallback, useEffect, useRef, useState } from "react";
import {
  clearActiveHighlightMatch,
  collectAllHighlightMatches,
  scrollHighlightMatchIntoView,
  setActiveHighlightMatch,
  TIMELINE_SEARCH_HIGHLIGHTS_CHANGED_EVENT,
} from "@/lib/searchTextHighlight";

const MATCH_REFRESH_DEBOUNCE_MS = 200;
const NAVIGATION_SETTLE_MS = 150;

export function useHighlightNavigation(
  query: string,
  isRegex: boolean,
  scrollContainerRef: React.RefObject<HTMLElement | null>,
  enabled = true,
) {
  const [activeIndex, setActiveIndex] = useState(-1);
  const [matchCount, setMatchCount] = useState(0);
  const activeIndexRef = useRef(activeIndex);
  const debounceRef = useRef<number | undefined>(undefined);
  activeIndexRef.current = activeIndex;

  const collectMatches = useCallback(() => {
    return collectAllHighlightMatches(query, isRegex);
  }, [isRegex, query]);

  const refreshMatchCount = useCallback(() => {
    const matches = collectMatches();
    setMatchCount(matches.length);

    if (matches.length === 0) {
      setActiveIndex(-1);
      clearActiveHighlightMatch();
      return matches;
    }

    const currentIndex = activeIndexRef.current;
    if (currentIndex >= 0 && currentIndex < matches.length) {
      setActiveHighlightMatch(matches[currentIndex]);
    } else if (currentIndex >= matches.length) {
      setActiveIndex(-1);
      clearActiveHighlightMatch();
    }

    return matches;
  }, [collectMatches]);

  const scheduleRefreshMatchCount = useCallback(() => {
    if (debounceRef.current !== undefined) {
      window.clearTimeout(debounceRef.current);
    }

    debounceRef.current = window.setTimeout(() => {
      refreshMatchCount();
    }, MATCH_REFRESH_DEBOUNCE_MS);
  }, [refreshMatchCount]);

  const goToMatch = useCallback(
    (index: number) => {
      const initialMatches = collectMatches();
      setMatchCount(initialMatches.length);

      if (initialMatches.length === 0) {
        setActiveIndex(-1);
        clearActiveHighlightMatch();
        return;
      }

      const normalizedIndex =
        ((index % initialMatches.length) + initialMatches.length) %
        initialMatches.length;
      const targetMatch = initialMatches[normalizedIndex];

      targetMatch.cardRoot.scrollIntoView({
        block: "center",
        behavior: "auto",
      });

      window.setTimeout(() => {
        const freshMatches = collectMatches();
        setMatchCount(freshMatches.length);

        if (freshMatches.length === 0) {
          setActiveIndex(-1);
          clearActiveHighlightMatch();
          return;
        }

        const resolvedIndex = Math.min(normalizedIndex, freshMatches.length - 1);
        const resolvedMatch = freshMatches[resolvedIndex];

        setActiveIndex(resolvedIndex);
        setActiveHighlightMatch(resolvedMatch);
        scrollHighlightMatchIntoView(
          resolvedMatch,
          scrollContainerRef.current,
        );
      }, NAVIGATION_SETTLE_MS);
    },
    [collectMatches, scrollContainerRef],
  );

  const goToNext = useCallback(() => {
    const matches = collectMatches();
    if (matches.length === 0) {
      return;
    }

    const nextIndex =
      activeIndexRef.current < 0 ? 0 : activeIndexRef.current + 1;
    goToMatch(nextIndex);
  }, [collectMatches, goToMatch]);

  const goToPrevious = useCallback(() => {
    const matches = collectMatches();
    if (matches.length === 0) {
      return;
    }

    const previousIndex =
      activeIndexRef.current <= 0
        ? matches.length - 1
        : activeIndexRef.current - 1;
    goToMatch(previousIndex);
  }, [collectMatches, goToMatch]);

  useEffect(() => {
    if (!enabled || !query.trim()) {
      setActiveIndex(-1);
      setMatchCount(0);
      clearActiveHighlightMatch();
      return;
    }

    setActiveIndex(-1);
    clearActiveHighlightMatch();

    const timeouts = [300, 800, 1500, 2500, 4000].map((delay) =>
      window.setTimeout(() => {
        refreshMatchCount();
      }, delay),
    );

    const handleHighlightsChanged = () => {
      scheduleRefreshMatchCount();
    };

    window.addEventListener(
      TIMELINE_SEARCH_HIGHLIGHTS_CHANGED_EVENT,
      handleHighlightsChanged,
    );

    const observer = new MutationObserver(() => {
      scheduleRefreshMatchCount();
    });

    if (scrollContainerRef.current) {
      observer.observe(scrollContainerRef.current, {
        childList: true,
        subtree: true,
        characterData: true,
      });
    }

    return () => {
      if (debounceRef.current !== undefined) {
        window.clearTimeout(debounceRef.current);
      }

      for (const timeoutId of timeouts) {
        window.clearTimeout(timeoutId);
      }

      window.removeEventListener(
        TIMELINE_SEARCH_HIGHLIGHTS_CHANGED_EVENT,
        handleHighlightsChanged,
      );
      observer.disconnect();
    };
  }, [
    enabled,
    isRegex,
    query,
    refreshMatchCount,
    scheduleRefreshMatchCount,
    scrollContainerRef,
  ]);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "g") {
        return;
      }

      event.preventDefault();
      if (event.shiftKey) {
        goToPrevious();
      } else {
        goToNext();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [enabled, goToNext, goToPrevious]);

  return {
    activeIndex,
    matchCount,
    goToNext,
    goToPrevious,
    hasMatches: matchCount > 0,
  };
}