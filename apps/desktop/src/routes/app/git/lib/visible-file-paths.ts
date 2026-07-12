import type { GetStatusResponse } from "@gitru/commands";

export const getVisibleFilePaths = (files: GetStatusResponse["files"]) =>
  Array.from(
    new Set(
      files.flatMap((file) =>
        file.new_path ? [file.path, file.new_path] : [file.path],
      ),
    ),
  );