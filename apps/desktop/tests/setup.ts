import * as matchers from "@testing-library/jest-dom/matchers";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, expect, vi } from "vitest";
import { installBrowserMocks } from "../../../tests/browser-mocks";
import { resetTauriMocks } from "./mocks/tauri";

resetTauriMocks();
installBrowserMocks();
expect.extend(matchers);

beforeEach(() => {
  installBrowserMocks();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  resetTauriMocks();
});
