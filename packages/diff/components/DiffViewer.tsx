import type { PatchDiffProps } from "@pierre/diffs/react";
import { PatchDiff } from "@pierre/diffs/react";
import React from "react";

export function DiffViewer<TAnnotation = undefined>({
  ...props
}: PatchDiffProps<TAnnotation>): React.JSX.Element {
  return <PatchDiff {...props} className="gitru-diff-content" />;
}
