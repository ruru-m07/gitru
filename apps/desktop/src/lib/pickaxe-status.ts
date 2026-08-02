import type { FileStatusKind } from "@gitru/commands";
import type { FileDiffMetadata } from "@pierre/diffs";
import type { PickaxeHit } from "@/hooks/use-pickaxe";

export function fileDiffTypeToStatus(
  type: FileDiffMetadata["type"],
): FileStatusKind[] {
  switch (type) {
    case "new":
      return ["IndexNew"];
    case "deleted":
      return ["IndexDeleted"];
    case "rename-pure":
    case "rename-changed":
      return ["IndexRenamed"];
    case "change":
    default:
      return ["IndexModified"];
  }
}

export function inferPickaxeHitStatus(hit: PickaxeHit): FileStatusKind[] {
  const patch = hit.patch ?? "";

  if (
    /^deleted file mode/m.test(patch) ||
    /^\+\+\+ \/dev\/null$/m.test(patch)
  ) {
    return ["IndexDeleted"];
  }

  if (/^new file mode/m.test(patch) || /^--- \/dev\/null$/m.test(patch)) {
    return ["IndexNew"];
  }

  if (hit.fileNewPath || /^rename from /m.test(patch)) {
    return ["IndexRenamed"];
  }

  return ["IndexModified"];
}
