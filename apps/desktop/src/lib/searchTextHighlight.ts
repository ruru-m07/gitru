import type { PickaxeSearchOptions } from "./pickaxe-search-options";

const HIGHLIGHT_NAME = "pickaxe";
const ACTIVE_HIGHLIGHT_NAME = "pickaxe-active";
const STYLE_ELEMENT_ID = "pickaxe-highlight-style";
const MARK_CLASS_NAME = "pickaxe-mark";
const ACTIVE_MARK_CLASS_NAME = "pickaxe-mark-active";

export type HighlightMatch =
  | {
      kind: "range";
      range: Range;
      cardRoot: HTMLElement;
    }
  | {
      kind: "mark";
      element: HTMLElement;
      cardRoot: HTMLElement;
    };

type CardHighlightEntry = {
  ranges: Range[];
  shadowRoots: Set<ShadowRoot>;
  cardRoot: HTMLElement;
};

type TextPosition = {
  node: Text;
  offset: number;
};

const cardHighlights = new Map<string, CardHighlightEntry>();

export const PICKAXE_HIGHLIGHTS_CHANGED_EVENT = "pickaxe-highlights-changed";

function notifyHighlightsChanged() {
  if (typeof window === "undefined") {
    return;
  }

  window.dispatchEvent(new Event(PICKAXE_HIGHLIGHTS_CHANGED_EVENT));
}

function isDebugEnabled() {
  return (
    import.meta.env.DEV &&
    typeof localStorage !== "undefined" &&
    localStorage.getItem("pickaxe-debug") === "1"
  );
}

function debugLog(message: string, data?: unknown) {
  if (!isDebugEnabled()) {
    return;
  }

  console.info(`[pickaxe-highlight] ${message}`, data ?? "");
}

function supportsCustomHighlight() {
  return typeof CSS !== "undefined" && "highlights" in CSS;
}

function escapeForRegex(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function buildSearchPattern(query: string, options: PickaxeSearchOptions) {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) {
    return null;
  }

  const { isRegex, matchCase, matchWholeWord } = options;

  if (isRegex) {
    if (normalizedQuery.startsWith("/") && normalizedQuery.length > 1) {
      const lastSlashIndex = normalizedQuery.lastIndexOf("/");
      if (lastSlashIndex > 0) {
        let pattern = normalizedQuery.slice(1, lastSlashIndex);
        let flags = normalizedQuery.slice(lastSlashIndex + 1) || "";
        if (!matchCase && !flags.includes("i")) {
          flags = `${flags}i`;
        }
        if (matchWholeWord) {
          pattern = `\\b(?:${pattern})\\b`;
        }
        try {
          return new RegExp(pattern, flags.includes("g") ? flags : `${flags}g`);
        } catch {
          return null;
        }
      }
    }

    try {
      let pattern = normalizedQuery;
      let flags = "g";
      if (!matchCase) {
        flags += "i";
      }
      if (matchWholeWord) {
        pattern = `\\b(?:${pattern})\\b`;
      }
      return new RegExp(pattern, flags);
    } catch {
      return null;
    }
  }

  let pattern = escapeForRegex(normalizedQuery);
  if (matchWholeWord) {
    pattern = `\\b${pattern}\\b`;
  }

  return new RegExp(pattern, matchCase ? "g" : "gi");
}

function getHighlightCss() {
  return `
    ::highlight(${HIGHLIGHT_NAME}) {
      background-color: rgba(250, 204, 21, 0.55);
      color: inherit;
    }

    ::highlight(${ACTIVE_HIGHLIGHT_NAME}) {
      background-color: rgba(249, 115, 22, 0.8);
      color: inherit;
      outline: 1px solid rgba(234, 88, 12, 0.95);
    }

    mark.${MARK_CLASS_NAME} {
      background-color: rgba(250, 204, 21, 0.55);
      color: inherit;
      padding: 0;
      border-radius: 2px;
    }

    mark.${ACTIVE_MARK_CLASS_NAME} {
      background-color: rgba(249, 115, 22, 0.8);
      color: inherit;
      outline: 1px solid rgba(234, 88, 12, 0.95);
      border-radius: 2px;
    }
  `;
}

function ensureHighlightStylesInDocument() {
  if (typeof document === "undefined") {
    return;
  }

  if (document.getElementById(STYLE_ELEMENT_ID)) {
    return;
  }

  const style = document.createElement("style");
  style.id = STYLE_ELEMENT_ID;
  style.textContent = getHighlightCss();
  document.head.appendChild(style);
}

function ensureHighlightStylesInShadowRoot(shadowRoot: ShadowRoot) {
  if (shadowRoot.getElementById(STYLE_ELEMENT_ID)) {
    return;
  }

  const style = document.createElement("style");
  style.id = STYLE_ELEMENT_ID;
  style.textContent = getHighlightCss();
  shadowRoot.appendChild(style);
}

function collectShadowRoots(root: HTMLElement) {
  const shadowRoots = new Set<ShadowRoot>();

  const visit = (element: Element) => {
    if (element.shadowRoot) {
      shadowRoots.add(element.shadowRoot);
      for (const child of element.shadowRoot.querySelectorAll("*")) {
        visit(child);
      }
    }

    for (const child of element.children) {
      visit(child);
    }
  };

  visit(root);
  return shadowRoots;
}

function shouldAcceptTextNode(node: Node) {
  const parent = node.parentElement;
  if (!parent) {
    return false;
  }

  if (parent.closest("style, script, noscript")) {
    return false;
  }

  if (parent.closest(`mark.${MARK_CLASS_NAME}`)) {
    return false;
  }

  return true;
}

function collectTextNodes(root: ParentNode) {
  const nodes: Text[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      return shouldAcceptTextNode(node)
        ? NodeFilter.FILTER_ACCEPT
        : NodeFilter.FILTER_REJECT;
    },
  });

  while (walker.nextNode()) {
    nodes.push(walker.currentNode as Text);
  }

  return nodes;
}

function withGlobalPattern(pattern: RegExp) {
  return new RegExp(
    pattern.source,
    pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`,
  );
}

function collectRangesInRoot(root: ParentNode, pattern: RegExp) {
  const textNodes = collectTextNodes(root);
  if (textNodes.length === 0) {
    return [] as Range[];
  }

  let combinedText = "";
  const positions: TextPosition[] = [];

  for (const node of textNodes) {
    const text = node.textContent ?? "";
    for (let offset = 0; offset < text.length; offset += 1) {
      positions.push({ node, offset });
    }
    combinedText += text;
  }

  if (!combinedText) {
    return [] as Range[];
  }

  const ranges: Range[] = [];
  const globalPattern = withGlobalPattern(pattern);
  let match = globalPattern.exec(combinedText);

  while (match) {
    const matchedText = match[0];
    if (!matchedText) {
      globalPattern.lastIndex += 1;
      match = globalPattern.exec(combinedText);
      continue;
    }

    const startIndex = match.index;
    const endIndex = startIndex + matchedText.length - 1;
    const startPosition = positions[startIndex];
    const endPosition = positions[endIndex];

    if (startPosition && endPosition) {
      const range = document.createRange();
      range.setStart(startPosition.node, startPosition.offset);
      range.setEnd(endPosition.node, endPosition.offset + 1);
      ranges.push(range);
    }

    match = globalPattern.exec(combinedText);
  }

  return ranges;
}

function clearMarkHighlights(root: HTMLElement) {
  for (const shadowRoot of collectShadowRoots(root)) {
    for (const mark of shadowRoot.querySelectorAll(`mark.${MARK_CLASS_NAME}`)) {
      const parent = mark.parentNode;
      if (!parent) {
        continue;
      }

      while (mark.firstChild) {
        parent.insertBefore(mark.firstChild, mark);
      }
      parent.removeChild(mark);
      parent.normalize();
    }
  }
}

function wrapRangeWithMark(range: Range) {
  const mark = document.createElement("mark");
  mark.className = MARK_CLASS_NAME;

  try {
    range.surroundContents(mark);
    return;
  } catch {
    const fragment = range.extractContents();
    mark.appendChild(fragment);
    range.insertNode(mark);
  }
}

function applyMarkHighlights(root: HTMLElement, pattern: RegExp) {
  clearMarkHighlights(root);

  for (const shadowRoot of collectShadowRoots(root)) {
    ensureHighlightStylesInShadowRoot(shadowRoot);
    const ranges = collectRangesInRoot(shadowRoot, pattern);

    for (const range of [...ranges].reverse()) {
      wrapRangeWithMark(range);
    }
  }
}

export function collectSearchRanges(
  root: HTMLElement,
  query: string,
  options: PickaxeSearchOptions,
) {
  const pattern = buildSearchPattern(query, options);
  if (!pattern) {
    return { ranges: [] as Range[], shadowRoots: new Set<ShadowRoot>() };
  }

  const shadowRoots = collectShadowRoots(root);
  const ranges = [
    ...collectRangesInRoot(root, pattern),
    ...Array.from(shadowRoots).flatMap((shadowRoot) =>
      collectRangesInRoot(shadowRoot, pattern),
    ),
  ];

  return { ranges, shadowRoots };
}

function syncGlobalHighlight() {
  if (!supportsCustomHighlight()) {
    return;
  }

  const allRanges = Array.from(cardHighlights.values()).flatMap(
    (entry) => entry.ranges,
  );
  const allShadowRoots = new Set<ShadowRoot>();
  for (const entry of cardHighlights.values()) {
    for (const shadowRoot of entry.shadowRoots) {
      allShadowRoots.add(shadowRoot);
    }
  }

  for (const shadowRoot of allShadowRoots) {
    ensureHighlightStylesInShadowRoot(shadowRoot);
  }

  if (allRanges.length === 0) {
    CSS.highlights.delete(HIGHLIGHT_NAME);
    CSS.highlights.delete(ACTIVE_HIGHLIGHT_NAME);
    notifyHighlightsChanged();
    return;
  }

  ensureHighlightStylesInDocument();
  CSS.highlights.set(HIGHLIGHT_NAME, new Highlight(...allRanges));
  notifyHighlightsChanged();
}

function compareRangePosition(left: Range, right: Range) {
  try {
    return left.compareBoundaryPoints(Range.START_TO_START, right);
  } catch {
    return 0;
  }
}

function isRangeValid(range: Range) {
  try {
    const startContainer = range.startContainer;
    if (!startContainer || !("isConnected" in startContainer)) {
      return false;
    }

    return startContainer.isConnected && range.toString().length > 0;
  } catch {
    return false;
  }
}

function sortHighlightMatches(matches: HighlightMatch[]) {
  matches.sort((left, right) => {
    const cardOrder = compareElementPosition(left.cardRoot, right.cardRoot);
    if (cardOrder !== 0) {
      return cardOrder;
    }

    if (left.kind === "range" && right.kind === "range") {
      return compareRangePosition(left.range, right.range);
    }

    if (left.kind === "mark" && right.kind === "mark") {
      return compareElementPosition(left.element, right.element);
    }

    return 0;
  });

  return matches;
}

function compareElementPosition(left: Element, right: Element) {
  const leftRange = document.createRange();
  leftRange.selectNode(left);
  const rightRange = document.createRange();
  rightRange.selectNode(right);
  return compareRangePosition(leftRange, rightRange);
}

function getPickaxeCardRoots() {
  return Array.from(document.querySelectorAll(".pickaxe-diff-root")).filter(
    (node): node is HTMLElement => node instanceof HTMLElement,
  );
}

function collectMarkMatches() {
  const matches: HighlightMatch[] = [];

  for (const cardRoot of getPickaxeCardRoots()) {
    for (const shadowRoot of collectShadowRoots(cardRoot)) {
      for (const mark of shadowRoot.querySelectorAll(
        `mark.${MARK_CLASS_NAME}`,
      )) {
        if (!(mark instanceof HTMLElement)) {
          continue;
        }

        matches.push({
          kind: "mark",
          element: mark,
          cardRoot,
        });
      }
    }
  }

  return sortHighlightMatches(matches);
}

export function collectAllHighlightMatches(
  query: string,
  options: PickaxeSearchOptions,
) {
  const pattern = buildSearchPattern(query, options);
  if (!pattern) {
    return [] as HighlightMatch[];
  }

  const markMatches = collectMarkMatches();
  if (markMatches.length > 0) {
    return sortHighlightMatches(markMatches);
  }

  const matches: HighlightMatch[] = [];

  for (const cardRoot of getPickaxeCardRoots()) {
    const { ranges } = collectSearchRanges(cardRoot, query, options);
    for (const range of ranges) {
      if (!isRangeValid(range)) {
        continue;
      }

      matches.push({
        kind: "range",
        range,
        cardRoot,
      });
    }
  }

  return sortHighlightMatches(matches);
}

export function clearActiveHighlightMatch() {
  if (supportsCustomHighlight()) {
    CSS.highlights.delete(ACTIVE_HIGHLIGHT_NAME);
  }

  if (typeof document === "undefined") {
    return;
  }

  for (const mark of document.querySelectorAll(
    `mark.${ACTIVE_MARK_CLASS_NAME}`,
  )) {
    mark.classList.remove(ACTIVE_MARK_CLASS_NAME);
  }
}

export function setActiveHighlightMatch(match: HighlightMatch | null) {
  clearActiveHighlightMatch();

  if (!match) {
    return;
  }

  if (match.kind === "range" && supportsCustomHighlight()) {
    if (!isRangeValid(match.range)) {
      return;
    }

    for (const shadowRoot of collectShadowRoots(match.cardRoot)) {
      ensureHighlightStylesInShadowRoot(shadowRoot);
    }
    ensureHighlightStylesInDocument();

    try {
      CSS.highlights.set(ACTIVE_HIGHLIGHT_NAME, new Highlight(match.range));
    } catch {
      return;
    }
    return;
  }

  if (match.kind === "mark") {
    for (const shadowRoot of collectShadowRoots(match.cardRoot)) {
      ensureHighlightStylesInShadowRoot(shadowRoot);
    }
    match.element.classList.add(ACTIVE_MARK_CLASS_NAME);
  }
}

export function scrollHighlightMatchIntoView(
  match: HighlightMatch,
  scrollContainer: HTMLElement | null,
) {
  const targetRect =
    match.kind === "range"
      ? match.range.getBoundingClientRect()
      : match.element.getBoundingClientRect();

  if (targetRect.width === 0 && targetRect.height === 0) {
    return;
  }

  match.cardRoot.scrollIntoView({
    block: "nearest",
    behavior: "smooth",
  });

  if (!scrollContainer) {
    return;
  }

  const containerRect = scrollContainer.getBoundingClientRect();
  const isAbove = targetRect.top < containerRect.top + 48;
  const isBelow = targetRect.bottom > containerRect.bottom - 48;

  if (!isAbove && !isBelow) {
    return;
  }

  const nextTop =
    scrollContainer.scrollTop +
    (targetRect.top - containerRect.top) -
    scrollContainer.clientHeight / 2 +
    targetRect.height / 2;

  scrollContainer.scrollTo({
    top: Math.max(0, nextTop),
    behavior: "smooth",
  });
}

export function setCardSearchHighlight(
  cardId: string,
  root: HTMLElement | null,
  query: string,
  options: PickaxeSearchOptions,
) {
  if (!root || !query.trim()) {
    clearCardSearchHighlight(cardId);
    return;
  }

  const pattern = buildSearchPattern(query, options);
  if (!pattern) {
    clearCardSearchHighlight(cardId);
    return;
  }

  const { ranges, shadowRoots } = collectSearchRanges(root, query, options);

  debugLog("apply", {
    cardId,
    query,
    options,
    supportsCustomHighlight: supportsCustomHighlight(),
    shadowRootCount: shadowRoots.size,
    rangeCount: ranges.length,
  });

  if (supportsCustomHighlight() && ranges.length > 0) {
    clearMarkHighlights(root);
    cardHighlights.set(cardId, { ranges, shadowRoots, cardRoot: root });
    syncGlobalHighlight();
    return;
  }

  clearCardSearchHighlight(cardId);
  applyMarkHighlights(root, pattern);
  notifyHighlightsChanged();
}

export function clearCardSearchHighlight(cardId: string) {
  const entry = cardHighlights.get(cardId);
  if (!entry) {
    return;
  }

  cardHighlights.delete(cardId);
  syncGlobalHighlight();
  notifyHighlightsChanged();
}

export function clearAllSearchHighlights() {
  cardHighlights.clear();
  clearActiveHighlightMatch();
  if (supportsCustomHighlight()) {
    CSS.highlights.delete(HIGHLIGHT_NAME);
    CSS.highlights.delete(ACTIVE_HIGHLIGHT_NAME);
  }

  if (typeof document === "undefined") {
    return;
  }

  for (const root of document.querySelectorAll(".pickaxe-diff-root")) {
    if (root instanceof HTMLElement) {
      clearMarkHighlights(root);
    }
  }
}

export function getSearchHighlightDebugInfo(
  root: HTMLElement | null,
  query: string,
  options: PickaxeSearchOptions,
) {
  if (!root || !query.trim()) {
    return {
      supportsCustomHighlight: supportsCustomHighlight(),
      shadowRootCount: 0,
      rangeCount: 0,
    };
  }

  const { ranges, shadowRoots } = collectSearchRanges(root, query, options);
  return {
    supportsCustomHighlight: supportsCustomHighlight(),
    shadowRootCount: shadowRoots.size,
    rangeCount: ranges.length,
  };
}
