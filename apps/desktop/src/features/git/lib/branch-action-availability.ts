import type { BranchInfo } from "@gitru/commands";

type BranchIdentity = Pick<BranchInfo, "is_protected" | "is_remote" | "name">;

export function branchDeleteBlockReason(
  branch: BranchIdentity,
  currentBranchName?: string,
  currentUpstream?: string,
): string | null {
  if (!branch.is_remote && branch.name === currentBranchName) {
    return "Check out another branch before deleting the current branch.";
  }
  if (branch.is_protected) {
    return "Change the remote default branch before deleting this protected branch.";
  }
  if (branch.is_remote && branch.name === currentUpstream) {
    return "Unset or change the current branch upstream before deleting it.";
  }
  return null;
}
