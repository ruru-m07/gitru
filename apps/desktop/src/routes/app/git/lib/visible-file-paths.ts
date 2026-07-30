import type { GetStatusResponse } from "@gitru/commands";

import {
  getGitStagePathspecs,
  getGitUnstagePathspecsForFiles,
} from "@/lib/git-pathspec";

export const getVisibleStagePaths = (files: GetStatusResponse["files"]) =>
  getGitStagePathspecs(files);

export const getVisibleUnstagePaths = (files: GetStatusResponse["files"]) =>
  getGitUnstagePathspecsForFiles(files);
