import type { FileStatusKind } from "@gitru/commands";
import type { FileDiffMetadata } from "@pierre/diffs";
import type { TimelineSearchHit } from "@/hooks/useTimelineSearch";

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

export function inferTimelineHitStatus(hit: TimelineSearchHit): FileStatusKind[] {
  const patch = hit.patch ?? "";

  if (/^deleted file mode/m.test(patch) || /^\+\+\+ \/dev\/null$/m.test(patch)) {
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