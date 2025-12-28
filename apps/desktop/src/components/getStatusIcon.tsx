import {
  AlertTriangle,
  CornerUpRight,
  EyeOff,
  SquareDot,
  SquarePlus,
  SquareX,
} from "lucide-react";
import { FileStatusKind } from "@/tauri";

export function getStatusIcon(type: FileStatusKind[]) {
  const kinds = new Set(type);

  if (kinds.has("WorktreeUnreadable")) {
    return <AlertTriangle className="text-orange-500" size={20} />;
  }

  if (kinds.has("IndexDeleted") || kinds.has("WorktreeDeleted")) {
    return <SquareX className="text-red-500" size={20} />;
  }

  if (
    kinds.has("IndexRenamed") ||
    kinds.has("WorktreeRenamed") ||
    kinds.has("IndexTypechange") ||
    kinds.has("WorktreeTypechange")
  ) {
    return <CornerUpRight className="text-purple-500" size={20} />;
  }

  if (kinds.has("IndexModified") || kinds.has("WorktreeModified")) {
    return <SquareDot className="text-yellow-500" size={20} />;
  }

  if (kinds.has("IndexNew") || kinds.has("WorktreeNew")) {
    return <SquarePlus className="text-green-500" size={20} />;
  }

  return <EyeOff className="text-gray-400" size={20} />;
}
