import type { HighlighterCore } from "shiki";
import {
  createHighlighterCore,
  createHighlighterCoreSync,
  createJavaScriptRegexEngine,
} from "shiki";

let initialized = false;

export let highlighter: null | HighlighterCore = null;

(async () => {
  try {
    if (initialized) {
      return;
    }
    if (!highlighter) {
      const hl = createHighlighterCoreSync({
        themes: [],
        langs: [],
        engine: createJavaScriptRegexEngine(),
      });

      if (!initialized) {
        highlighter = hl;
        initialized = true;
      }
    }
  } catch (error) {
    console.error("Failed to initialize Shiki highlighter:", error);
  }
})();
