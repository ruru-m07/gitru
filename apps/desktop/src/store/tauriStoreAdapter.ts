import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Store } from "@tauri-apps/plugin-store";

const isTauriRuntime = () =>
  typeof window !== "undefined" &&
  typeof (window as Window & { __TAURI_INTERNALS__?: unknown })
    .__TAURI_INTERNALS__ !== "undefined";

const isChildTabWebview = () => {
  if (!isTauriRuntime()) {
    return false;
  }

  try {
    return getCurrentWebview().label.startsWith("tab-webview:");
  } catch {
    return false;
  }
};

export const createTauriStorage = () => {
  const isReadOnlyStorage = isChildTabWebview();
  const storePromise = Store.load("app-state.json");

  return {
    getItem: async (name: string) => {
      const store = await storePromise;
      return (await store.get<string>(name)) ?? null;
    },
    setItem: async (name: string, value: string) => {
      if (isReadOnlyStorage) {
        return;
      }

      const store = await storePromise;
      await store.set(name, value);
      await store.save();
    },
    removeItem: async (name: string) => {
      if (isReadOnlyStorage) {
        return;
      }

      const store = await storePromise;
      await store.delete(name);
      await store.save();
    },
  };
};
