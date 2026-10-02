import type { CommitMessage } from "@gitru/commands";
import { create } from "zustand";

export type CommitDraftMode = "create" | "amend";

type CommitDraftSnapshot = {
  title: string;
  description: string;
  coAuthors: CommitMessage["co_authors"];
  autofillKey: string | null;
};

type CommitDraftState = {
  /** Active repo/context key — draft clears when this changes. */
  repoKey: string | null;
  title: string;
  description: string;
  coAuthors: CommitMessage["co_authors"];
  mode: CommitDraftMode;
  amendCommitId: string | null;
  /** Draft to restore if the user cancels amend mode. */
  draftBeforeAmend: CommitDraftSnapshot | null;
  /** Last autofill identity — skip re-applying the same rebase step. */
  autofillKey: string | null;
  setTitle: (title: string) => void;
  setDescription: (description: string) => void;
  setCoAuthors: (coAuthors: CommitMessage["co_authors"]) => void;
  applyAutofill: (
    key: string,
    title: string,
    description: string,
    coAuthors?: CommitMessage["co_authors"],
  ) => void;
  beginAmend: (
    commitId: string,
    title: string,
    description: string,
    coAuthors: CommitMessage["co_authors"],
  ) => void;
  cancelAmend: () => void;
  switchRepo: (repoKey: string | null) => void;
  clear: () => void;
};

const emptyDraft = {
  title: "",
  description: "",
  coAuthors: [] as CommitMessage["co_authors"],
  mode: "create" as const,
  amendCommitId: null,
  draftBeforeAmend: null,
  autofillKey: null,
};

const CO_AUTHOR_TRAILER = /^Co-authored-by:\s*(.+?)\s*<([^<>]+)>\s*$/i;
const GIT_TRAILER = /^[A-Za-z0-9][A-Za-z0-9-]*:\s*\S.*$/;

function extractTrailingCoAuthorTrailers(message: string): {
  message: string;
  coAuthors: CommitMessage["co_authors"];
} {
  const lines = message.split("\n");

  while (lines.at(-1)?.trim() === "") lines.pop();
  let trailerStart = lines.length;
  let foundTrailer = false;

  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index];
    if (GIT_TRAILER.test(line)) {
      trailerStart = index;
      foundTrailer = true;
      continue;
    }
    if (foundTrailer && line.trim() === "") {
      trailerStart = index;
      continue;
    }
    break;
  }

  const trailingBlock = lines.slice(trailerStart);
  const coAuthors: CommitMessage["co_authors"] = [];
  for (const line of trailingBlock) {
    const match = line.match(CO_AUTHOR_TRAILER);
    if (match) coAuthors.push([match[1].trim(), match[2].trim()]);
  }
  if (coAuthors.length === 0) {
    return { message: lines.join("\n").trim(), coAuthors: [] };
  }

  const body = lines.slice(0, trailerStart).join("\n").trim();
  const remainingTrailers = trailingBlock
    .filter((line) => line.trim() && !CO_AUTHOR_TRAILER.test(line))
    .join("\n");

  return {
    message: [body, remainingTrailers].filter(Boolean).join("\n\n"),
    coAuthors,
  };
}

export function stripTrailingCoAuthorTrailers(message: string): string {
  return extractTrailingCoAuthorTrailers(message).message;
}

export function splitCommitMessage(message: string): {
  title: string;
  description: string;
  coAuthors: CommitMessage["co_authors"];
} {
  const parsed = extractTrailingCoAuthorTrailers(message);
  const cleaned = parsed.message;
  const coAuthors = parsed.coAuthors;
  if (!cleaned) return { title: "", description: "", coAuthors };
  const nl = cleaned.indexOf("\n");
  if (nl < 0) return { title: cleaned, description: "", coAuthors };
  return {
    title: cleaned.slice(0, nl).trim(),
    description: cleaned.slice(nl + 1).trim(),
    coAuthors,
  };
}

export function joinCommitMessage(
  title: string,
  description: string,
  coAuthors: CommitMessage["co_authors"] = [],
): string {
  const t = title.trim();
  const d = description.trim();
  const message = d ? (t ? `${t}\n\n${d}` : d) : t;
  const trailers = coAuthors
    .map(([name, email]) => `Co-authored-by: ${name.trim()} <${email.trim()}>`)
    .join("\n");

  if (!message) return trailers;
  if (!trailers) return message;

  const lastDescriptionLine = d.split("\n").at(-1) ?? "";
  const trailerSeparator =
    d && GIT_TRAILER.test(lastDescriptionLine) ? "\n" : "\n\n";
  return `${message}${trailerSeparator}${trailers}`;
}

export const useCommitDraftStore = create<CommitDraftState>((set, get) => ({
  repoKey: null,
  ...emptyDraft,
  setTitle: (title) => set({ title }),
  setDescription: (description) => set({ description }),
  setCoAuthors: (coAuthors) => set({ coAuthors }),
  applyAutofill: (key, title, description, coAuthors = []) => {
    if (get().autofillKey === key) return;
    set({
      amendCommitId: null,
      autofillKey: key,
      coAuthors,
      description,
      draftBeforeAmend: null,
      mode: "create",
      title,
    });
  },
  beginAmend: (commitId, title, description, coAuthors) => {
    const state = get();
    if (state.mode === "amend") return;
    set({
      amendCommitId: commitId,
      autofillKey: `amend:${commitId}`,
      coAuthors,
      description,
      draftBeforeAmend: {
        title: state.title,
        description: state.description,
        coAuthors: state.coAuthors,
        autofillKey: state.autofillKey,
      },
      mode: "amend",
      title,
    });
  },
  cancelAmend: () => {
    const state = get();
    if (state.mode !== "amend") return;
    set({
      ...(state.draftBeforeAmend ?? emptyDraft),
      amendCommitId: null,
      draftBeforeAmend: null,
      mode: "create",
    });
  },
  switchRepo: (repoKey) => {
    if (get().repoKey === repoKey) return;
    set({ repoKey, ...emptyDraft });
  },
  clear: () => set(emptyDraft),
}));
