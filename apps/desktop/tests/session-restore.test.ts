import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  embedded: false,
  flush: vi.fn(),
  historyState: {
    __TSR_index: 0,
    __TSR_key: "current",
    key: "current",
  },
  lastPage: "/",
  replace: vi.fn(),
}));

vi.mock("../src/bootstrap/create-router", () => ({
  router: {
    history: {
      flush: mocks.flush,
      location: {
        state: mocks.historyState,
      },
      replace: mocks.replace,
    },
  },
}));

vi.mock("../src/store/use-last-page-store", () => ({
  useLastPageStore: {
    getState: () => ({ lastPage: mocks.lastPage }),
  },
}));

vi.mock("../src/bootstrap/runtime-utils", () => ({
  getRoutePathname: (routePath: string) =>
    new URL(routePath, window.location.origin).pathname,
  HOST_SHELL_ROUTE: "/app",
  isDesktopHostRuntime: () => !mocks.embedded,
  isEmbeddedRuntime: () => mocks.embedded,
}));

import { redirectToLastPage } from "../src/bootstrap/session-restore";

describe("startup session restoration", () => {
  beforeEach(() => {
    mocks.embedded = false;
    mocks.lastPage = "/";
    window.history.replaceState(null, "", "/");
  });

  test("replaces the host location with the persisted app shell", () => {
    mocks.lastPage = "/app";

    expect(redirectToLastPage()).toBeUndefined();

    expect(mocks.replace).toHaveBeenCalledWith("/app", mocks.historyState, {
      ignoreBlocker: true,
    });
    expect(mocks.flush).toHaveBeenCalledOnce();
  });

  test("canonicalizes a host workspace route to the app shell", () => {
    window.history.replaceState(null, "", "/app/git?view=history#details");

    redirectToLastPage();

    expect(mocks.replace).toHaveBeenCalledWith("/app", mocks.historyState, {
      ignoreBlocker: true,
    });
    expect(mocks.flush).toHaveBeenCalledOnce();
  });

  test("restores a persisted non-workspace route", () => {
    mocks.lastPage = "/auth/onboarding?source=launch";

    redirectToLastPage();

    expect(mocks.replace).toHaveBeenCalledWith(
      "/auth/onboarding?source=launch",
      mocks.historyState,
      { ignoreBlocker: true },
    );
    expect(mocks.flush).toHaveBeenCalledOnce();
  });

  test("leaves embedded runtimes on their current route", () => {
    mocks.embedded = true;
    mocks.lastPage = "/app";
    window.history.replaceState(null, "", "/app/git?embedded=1");

    redirectToLastPage();

    expect(mocks.replace).not.toHaveBeenCalled();
    expect(mocks.flush).not.toHaveBeenCalled();
  });

  test("ignores persisted embedded routes in the host runtime", () => {
    mocks.lastPage = "/app/git?embedded=1";

    redirectToLastPage();

    expect(mocks.replace).not.toHaveBeenCalled();
    expect(mocks.flush).not.toHaveBeenCalled();
  });
});
