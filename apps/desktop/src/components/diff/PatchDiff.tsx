"use client";

import {
  DIFFS_TAG_NAME,
  FileDiffMetadata,
  getSingularPatch,
} from "@pierre/diffs";
import {
  DiffBasePropsReact,
  renderDiffChildren,
  templateRender,
  useFileDiffInstance,
} from "@pierre/diffs/react";
import { useMemo } from "react";

export interface PatchDiffProps<LAnnotation>
  extends DiffBasePropsReact<LAnnotation> {
  patch: string;
}

export function PatchDiff<LAnnotation = undefined>({
  patch,
  options,
  lineAnnotations,
  selectedLines,
  className,
  style,
  prerenderedHTML,
  renderAnnotation,
  renderHeaderMetadata,
  renderHoverUtility,
}: PatchDiffProps<LAnnotation>): React.JSX.Element {
  const fileDiff = usePatch(patch);
  console.log(fileDiff);
  const { ref, getHoveredLine } = useFileDiffInstance({
    fileDiff,
    options,
    lineAnnotations,
    selectedLines,
    prerenderedHTML,
  });
  const children = renderDiffChildren({
    fileDiff,
    renderHeaderMetadata,
    renderAnnotation,
    lineAnnotations,
    renderHoverUtility,
    getHoveredLine,
  });
  return (
    // @ts-ignore
    <DIFFS_TAG_NAME ref={ref} className={className} style={style}>
      {templateRender(children, prerenderedHTML)}
    </DIFFS_TAG_NAME>
  );
}

function usePatch(patch: string): FileDiffMetadata {
  return useMemo<FileDiffMetadata>(() => getSingularPatch(patch), [patch]);
}
