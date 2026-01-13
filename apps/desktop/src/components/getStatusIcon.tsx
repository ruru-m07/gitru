import { Conflict, Deleted, Modifed, New, Renamed, Unknown } from "@gitru/icon";
import { FileStatusKind } from "@/tauri";

export function getStatusIcon(type: FileStatusKind[], size: number = 20) {
  const kinds = new Set(type);

  if (kinds.has("WorktreeUnreadable")) {
    return (
      <Conflict
        style={{
          height: `${size}px`,
          width: `${size}px`,
        }}
      />
    );
  }

  if (kinds.has("IndexDeleted") || kinds.has("WorktreeDeleted")) {
    return (
      <Deleted
        style={{
          height: `${size}px`,
          width: `${size}px`,
        }}
      />
    );
  }

  if (
    kinds.has("IndexRenamed") ||
    kinds.has("WorktreeRenamed") ||
    kinds.has("IndexTypechange") ||
    kinds.has("WorktreeTypechange")
  ) {
    return (
      <Renamed
        style={{
          height: `${size}px`,
          width: `${size}px`,
        }}
      />
    );
  }

  if (kinds.has("IndexModified") || kinds.has("WorktreeModified")) {
    return (
      <Modifed
        style={{
          height: `${size}px`,
          width: `${size}px`,
        }}
      />
    );
  }

  if (kinds.has("IndexNew") || kinds.has("WorktreeNew")) {
    return (
      <New
        style={{
          height: `${size}px`,
          width: `${size}px`,
        }}
      />
    );
  }

  return (
    <Unknown
      style={{
        height: `${size}px`,
        width: `${size}px`,
      }}
    />
  );
}
