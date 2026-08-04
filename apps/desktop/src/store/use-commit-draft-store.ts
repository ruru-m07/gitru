import { create } from "zustand";

type CommitDraftState = {
  /** Active repo/context key — draft clears when this changes. */
  repoKey: string | null;
  title: string;
  description: string;
  /** Last autofill identity — skip re-applying the same rebase step. */
  autofillKey: string | null;
  setTitle: (title: string) => void;
  setDescription: (description: string) => void;
  applyAutofill: (key: string, title: string, description: string) => void;
  switchRepo: (repoKey: string | null) => void;
  clear: () => void;
};

export function splitCommitMessage(message: string): {
  title: string;
  description: string;
} {
  const cleaned = message
    .split("\n")
    .filter((line) => !line.startsWith("#"))
    .join("\n")
    .trim();
  if (!cleaned) return { title: "", description: "" };
  const nl = cleaned.indexOf("\n");
  if (nl < 0) return { title: cleaned, description: "" };
  return {
    title: cleaned.slice(0, nl).trim(),
    description: cleaned.slice(nl + 1).trim(),
  };
}

export function joinCommitMessage(title: string, description: string): string {
  const t = title.trim();
  const d = description.trim();
  if (!d) return t;
  if (!t) return d;
  return `${t}\n\n${d}`;
}

export const useCommitDraftStore = create<CommitDraftState>((set, get) => ({
  repoKey: null,
  title: "",
  description: "",
  autofillKey: null,
  setTitle: (title) => set({ title }),
  setDescription: (description) => set({ description }),
  applyAutofill: (key, title, description) => {
    if (get().autofillKey === key) return;
    set({ autofillKey: key, title, description });
  },
  switchRepo: (repoKey) => {
    if (get().repoKey === repoKey) return;
    set({ repoKey, title: "", description: "", autofillKey: null });
  },
  clear: () => set({ title: "", description: "", autofillKey: null }),
}));
