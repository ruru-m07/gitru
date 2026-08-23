import { create } from "zustand";
import { persist } from "zustand/middleware";

interface LastPageState {
  lastPage: string;
  setLastPage: (page: string) => void;
}

export const useLastPageStore = create<LastPageState>()(
  persist(
    (set) => ({
      lastPage: "/",
      setLastPage: (page) => set({ lastPage: page }),
    }),
    {
      name: "last-page",
    },
  ),
);
