import type { InlineDiffSegment } from "../types";

/**
 * Compute word-level inline diff between two strings.
 * Returns an array of segments with added/removed markers.
 */
export function computeInlineDiff(
  oldText: string,
  newText: string,
): InlineDiffSegment[] {
  if (oldText === newText) {
    return [{ text: oldText }];
  }

  if (!oldText) {
    return [{ text: newText, added: true }];
  }

  if (!newText) {
    return [{ text: oldText, removed: true }];
  }

  // Split into words, keeping whitespace and punctuation
  const oldWords = tokenize(oldText);
  const newWords = tokenize(newText);

  // Compute LCS of words
  const lcs = computeWordLCS(oldWords, newWords);

  // Build result segments
  const segments: InlineDiffSegment[] = [];
  let oldIdx = 0;
  let newIdx = 0;
  let lcsIdx = 0;

  while (oldIdx < oldWords.length || newIdx < newWords.length) {
    const lcsItem = lcs[lcsIdx];

    // Check if current words match LCS
    const matchOld = lcsItem !== undefined && oldWords[oldIdx] === lcsItem.word;
    const matchNew = lcsItem !== undefined && newWords[newIdx] === lcsItem.word;

    if (matchOld && matchNew) {
      // Both match - context
      pushSegment(segments, { text: oldWords[oldIdx] ?? "" });
      oldIdx++;
      newIdx++;
      lcsIdx++;
    } else if (
      newIdx < newWords.length &&
      (lcsItem === undefined || newWords[newIdx] !== lcsItem.word)
    ) {
      // Addition
      pushSegment(segments, { text: newWords[newIdx] ?? "", added: true });
      newIdx++;
    } else if (oldIdx < oldWords.length) {
      // Deletion
      pushSegment(segments, { text: oldWords[oldIdx] ?? "", removed: true });
      oldIdx++;
    }
  }

  return mergeAdjacentSegments(segments);
}

/**
 * Tokenize text into words, keeping whitespace attached
 */
function tokenize(text: string): string[] {
  const tokens: string[] = [];
  let current = "";

  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (char === undefined) continue;

    if (/\s/.test(char)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      // Group consecutive whitespace
      let whitespace = char;
      while (i + 1 < text.length && /\s/.test(text[i + 1] ?? "")) {
        whitespace += text[++i] ?? "";
      }
      tokens.push(whitespace);
    } else if (/[^\w]/.test(char)) {
      // Punctuation gets its own token
      if (current) {
        tokens.push(current);
        current = "";
      }
      tokens.push(char);
    } else {
      current += char;
    }
  }

  if (current) {
    tokens.push(current);
  }

  return tokens;
}

interface LCSWord {
  word: string;
  oldIdx: number;
  newIdx: number;
}

/**
 * Compute LCS of word arrays
 */
function computeWordLCS(oldWords: string[], newWords: string[]): LCSWord[] {
  const m = oldWords.length;
  const n = newWords.length;

  // Build LCS table
  const dp: number[][] = Array(m + 1)
    .fill(null)
    .map(() => Array(n + 1).fill(0) as number[]);

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (oldWords[i - 1] === newWords[j - 1]) {
        dp[i]![j] = (dp[i - 1]?.[j - 1] ?? 0) + 1;
      } else {
        dp[i]![j] = Math.max(dp[i - 1]?.[j] ?? 0, dp[i]?.[j - 1] ?? 0);
      }
    }
  }

  // Backtrack
  const lcs: LCSWord[] = [];
  let i = m;
  let j = n;

  while (i > 0 && j > 0) {
    if (oldWords[i - 1] === newWords[j - 1]) {
      lcs.unshift({
        word: oldWords[i - 1] ?? "",
        oldIdx: i - 1,
        newIdx: j - 1,
      });
      i--;
      j--;
    } else if ((dp[i - 1]?.[j] ?? 0) > (dp[i]?.[j - 1] ?? 0)) {
      i--;
    } else {
      j--;
    }
  }

  return lcs;
}

/**
 * Push segment, merging with previous if same type
 */
function pushSegment(
  segments: InlineDiffSegment[],
  segment: InlineDiffSegment,
): void {
  const prev = segments[segments.length - 1];

  if (
    prev &&
    prev.added === segment.added &&
    prev.removed === segment.removed
  ) {
    prev.text += segment.text;
  } else {
    segments.push(segment);
  }
}

/**
 * Merge adjacent segments of the same type
 */
function mergeAdjacentSegments(
  segments: InlineDiffSegment[],
): InlineDiffSegment[] {
  if (segments.length <= 1) return segments;

  const result: InlineDiffSegment[] = [];

  for (const segment of segments) {
    const prev = result[result.length - 1];

    if (
      prev &&
      prev.added === segment.added &&
      prev.removed === segment.removed
    ) {
      prev.text += segment.text;
    } else {
      result.push({ ...segment });
    }
  }

  return result;
}

/**
 * Render inline diff segments to HTML with highlighting classes
 */
export function renderInlineDiffHtml(
  segments: InlineDiffSegment[],
  options: {
    addedClass?: string;
    removedClass?: string;
  } = {},
): string {
  const {
    addedClass = "diff-inline--added",
    removedClass = "diff-inline--removed",
  } = options;

  return segments
    .map((segment) => {
      const escaped = escapeHtmlInline(segment.text);

      if (segment.added) {
        return `<span class="${addedClass}">${escaped}</span>`;
      }
      if (segment.removed) {
        return `<span class="${removedClass}">${escaped}</span>`;
      }
      return escaped;
    })
    .join("");
}

function escapeHtmlInline(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}
