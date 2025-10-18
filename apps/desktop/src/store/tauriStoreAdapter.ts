import { Store } from "@tauri-apps/plugin-store";

export const createTauriStorage = () => {
  const storePromise = Store.load("app-state.json");

  return {
    getItem: async (name: string) => {
      const store = await storePromise;
      return (await store.get<string>(name)) ?? null;
    },
    setItem: async (name: string, value: string) => {
      const store = await storePromise;
      await store.set(name, value);
      await store.save();
    },
    removeItem: async (name: string) => {
      const store = await storePromise;
      await store.delete(name);
      await store.save();
    },
  };
};
