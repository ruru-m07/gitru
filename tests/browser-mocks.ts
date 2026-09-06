import { vi } from "vitest";

class ResizeObserverMock implements ResizeObserver {
  disconnect() {}
  observe() {}
  unobserve() {}
}

class IntersectionObserverMock implements IntersectionObserver {
  readonly root = null;
  readonly rootMargin = "0px";
  readonly thresholds = [];

  disconnect() {}
  observe() {}
  takeRecords(): IntersectionObserverEntry[] {
    return [];
  }
  unobserve() {}
}

const createMemoryStorage = (): Storage => {
  const values = new Map<string, string>();

  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => values.delete(key),
    setItem: (key, value) => values.set(key, String(value)),
  };
};

export const installBrowserMocks = () => {
  vi.stubGlobal("localStorage", createMemoryStorage());
  vi.stubGlobal("sessionStorage", createMemoryStorage());
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string): MediaQueryList => {
      const eventTarget = new EventTarget();
      const mediaQueryList = eventTarget as MediaQueryList;

      Object.defineProperties(mediaQueryList, {
        matches: { configurable: true, value: false },
        media: { configurable: true, value: query },
        onchange: { configurable: true, value: null, writable: true },
      });

      mediaQueryList.addListener = (listener) => {
        if (listener) {
          eventTarget.addEventListener("change", listener as EventListener);
        }
      };
      mediaQueryList.removeListener = (listener) => {
        if (listener) {
          eventTarget.removeEventListener("change", listener as EventListener);
        }
      };

      return mediaQueryList;
    }),
    writable: true,
  });
  vi.stubGlobal("IntersectionObserver", IntersectionObserverMock);
  vi.stubGlobal("ResizeObserver", ResizeObserverMock);
};
