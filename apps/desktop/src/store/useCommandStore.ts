import type { BranchInfo } from "@gitru/commands";
import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";

export type CommandViewType =
  | "root"
  | "branch-list"
  | "branch-confirm"
  | "create-branch";

export interface ViewStackItem<T = unknown> {
  id: string;
  type: CommandViewType;
  props?: T;
  searchQuery: string;
  placeholder?: string;
}

export interface BranchConfirmProps {
  targetBranch: BranchInfo;
  currentBranch: string;
  hasChanges: boolean;
}

export interface CreateBranchProps {
  hasChanges: boolean;
  suggestedName?: string;
}

interface CommandState {
  isOpen: boolean;
  setIsOpen: (isOpen: boolean) => void;

  viewStack: ViewStackItem[];

  currentView: ViewStackItem;

  searchQuery: string;
  setSearchQuery: (query: string) => void;

  push: <T>(
    type: CommandViewType,
    options?: {
      props?: T;
      placeholder?: string;
      preserveSearch?: boolean;
    },
  ) => void;
  pop: () => void;
  replace: <T>(
    type: CommandViewType,
    options?: {
      props?: T;
      placeholder?: string;
    },
  ) => void;
  reset: () => void;

  getPlaceholder: () => string;
}

const DEFAULT_PLACEHOLDER = "Search for apps and commands...";

const createRootView = (): ViewStackItem => ({
  id: "root",
  type: "root",
  searchQuery: "",
  placeholder: DEFAULT_PLACEHOLDER,
});

export const useCommandStore = create<CommandState>()(
  subscribeWithSelector((set, get) => ({
    isOpen: false,
    setIsOpen: (isOpen) => {
      if (isOpen) {
        set({ isOpen: true });
      } else {
        set({
          isOpen: false,
          viewStack: [createRootView()],
          searchQuery: "",
          currentView: createRootView(),
        });
      }
    },

    viewStack: [createRootView()],

    currentView: createRootView(),

    searchQuery: "",
    setSearchQuery: (query) => {
      const stack = get().viewStack;
      if (stack.length === 0) return;

      const newStack = [...stack];
      const updatedCurrentView = {
        ...newStack[newStack.length - 1],
        searchQuery: query,
      };
      newStack[newStack.length - 1] = updatedCurrentView;

      set({
        searchQuery: query,
        viewStack: newStack,
        currentView: updatedCurrentView,
      });
    },

    push: (type, options = {}) => {
      const { props, placeholder, preserveSearch = false } = options;
      const currentStack = get().viewStack;
      const currentQuery = preserveSearch ? get().searchQuery : "";

      const newView: ViewStackItem = {
        id: `${type}-${Date.now()}`,
        type,
        props,
        searchQuery: currentQuery,
        placeholder,
      };

      set({
        viewStack: [...currentStack, newView],
        searchQuery: currentQuery,
        currentView: newView,
      });
    },

    pop: () => {
      const stack = get().viewStack;
      if (stack.length <= 1) {
        get().setIsOpen(false);
        return;
      }

      const newStack = stack.slice(0, -1);
      const previousView = newStack[newStack.length - 1];

      set({
        viewStack: newStack,
        searchQuery: previousView.searchQuery,
        currentView: previousView,
      });
    },

    replace: (type, options = {}) => {
      const { props, placeholder } = options;
      const stack = get().viewStack;
      if (stack.length === 0) return;

      const newView: ViewStackItem = {
        id: `${type}-${Date.now()}`,
        type,
        props,
        searchQuery: "",
        placeholder,
      };

      const newStack = [...stack.slice(0, -1), newView];
      set({
        viewStack: newStack,
        searchQuery: "",
        currentView: newView,
      });
    },

    reset: () => {
      const rootView = createRootView();
      set({
        viewStack: [rootView],
        searchQuery: "",
        currentView: rootView,
      });
    },

    getPlaceholder: () => {
      const view = get().currentView;
      return view?.placeholder ?? DEFAULT_PLACEHOLDER;
    },
  })),
);

export const selectIsOpen = (state: CommandState) => state.isOpen;
export const selectCurrentView = (state: CommandState) => state.currentView;
export const selectSearchQuery = (state: CommandState) => state.searchQuery;
export const selectCanGoBack = (state: CommandState) =>
  state.viewStack.length > 1;
export const selectViewStack = (state: CommandState) => state.viewStack;
