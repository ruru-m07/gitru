import type { FileStatus } from "@gitru/commands";
import type {
  FileSelectionIdentity,
  SelectionSource,
} from "@/store/useAppStore";

export type ResolvedFileSelection =
  | { state: "none" }
  | {
      state: "valid";
      identity: FileSelectionIdentity;
      file: FileStatus;
    }
  | {
      state: "stale";
      identity: FileSelectionIdentity;
      reason: "missing" | "stash_removed" | "source_mismatch";
    };

type ResolveSelectionParams = {
  selection: FileSelectionIdentity | null;
  files: FileStatus[];
  context: {
    source: SelectionSource;
    stashReference?: string | null;
    availableStashReferences?: string[];
  };
};

const matchesSelectionIdentity = (
  selection: FileSelectionIdentity,
  file: FileStatus,
) => {
  if (file.path === selection.filePath) {
    return true;
  }

  if (selection.fileNewPath && file.path === selection.fileNewPath) {
    return true;
  }

  if (selection.fileNewPath && file.new_path === selection.fileNewPath) {
    return true;
  }

  if (file.new_path && file.new_path === selection.filePath) {
    return true;
  }

  return false;
};

export const resolveFileSelection = ({
  selection,
  files,
  context,
}: ResolveSelectionParams): ResolvedFileSelection => {
  if (!selection) {
    return { state: "none" };
  }

  if (selection.source !== context.source) {
    return {
      state: "stale",
      identity: selection,
      reason: "source_mismatch",
    };
  }

  if (selection.source === "stash") {
    if (!selection.stashReference || !context.stashReference) {
      return {
        state: "stale",
        identity: selection,
        reason: "source_mismatch",
      };
    }

    if (selection.stashReference !== context.stashReference) {
      return {
        state: "stale",
        identity: selection,
        reason: "source_mismatch",
      };
    }

    if (
      context.availableStashReferences &&
      !context.availableStashReferences.includes(selection.stashReference)
    ) {
      return {
        state: "stale",
        identity: selection,
        reason: "stash_removed",
      };
    }
  }

  const match = files.find((file) => matchesSelectionIdentity(selection, file));

  if (!match) {
    return {
      state: "stale",
      identity: selection,
      reason: "missing",
    };
  }

  return {
    state: "valid",
    identity: {
      ...selection,
      filePath: match.path,
      fileNewPath: match.new_path,
    },
    file: match,
  };
};
