import { waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => {
  return {
    createRoot: vi.fn(),
    initializeQueryBridge: vi.fn(),
    redirectToLastPage: vi.fn(() => Promise.resolve()),
    render: vi.fn(),
  };
});

vi.mock("react-dom/client", () => ({
  default: {
    createRoot: mocks.createRoot,
  },
}));

vi.mock("react-scan", () => ({
  scan: vi.fn(),
}));

vi.mock("../src/bootstrap/app-root", () => ({
  AppRoot: () => null,
}));

vi.mock("../src/bootstrap/query-bridge", () => ({
  initializeQueryBridge: mocks.initializeQueryBridge,
}));

vi.mock("../src/bootstrap/runtime-utils", () => ({
  enableDevDiagnostics: () => false,
}));

vi.mock("../src/bootstrap/session-restore", () => ({
  redirectToLastPage: mocks.redirectToLastPage,
}));

let resolveRestore!: () => void;

describe("desktop startup", () => {
  beforeEach(() => {
    const restorePromise = new Promise<void>((resolve) => {
      resolveRestore = resolve;
    });

    vi.resetModules();
    document.body.innerHTML = '<div id="root"></div>';
    mocks.createRoot.mockReturnValue({ render: mocks.render });
    mocks.redirectToLastPage.mockReset();
    mocks.redirectToLastPage.mockReturnValue(restorePromise);
  });

  test("mounts React without waiting for session restoration", async () => {
    const rootElement = document.getElementById("root");
    const loadMain = import("../src/main");

    try {
      await waitFor(() => {
        expect(mocks.redirectToLastPage).toHaveBeenCalledOnce();
      });

      expect(mocks.createRoot).toHaveBeenCalledWith(rootElement);
      expect(mocks.render).toHaveBeenCalledOnce();
      expect(mocks.initializeQueryBridge).toHaveBeenCalledOnce();
    } finally {
      resolveRestore();
      await loadMain;
    }
  });

  test("mounts React when session restoration throws", async () => {
    const restoreError = new Error("invalid persisted route");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.redirectToLastPage.mockImplementationOnce(() => {
      throw restoreError;
    });

    await import("../src/main");

    expect(consoleError).toHaveBeenCalledWith(
      "Failed to restore the last page",
      restoreError,
    );
    expect(mocks.createRoot).toHaveBeenCalledWith(
      document.getElementById("root"),
    );
    expect(mocks.render).toHaveBeenCalledOnce();
    expect(mocks.initializeQueryBridge).toHaveBeenCalledOnce();
  });
});
