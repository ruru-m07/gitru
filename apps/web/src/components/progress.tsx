import { DiffViewer, type DiffViewStyle } from "@gitru/diff";
import React from "react";
import "./diff-viewer.css";

import patch1 from "./unified-diff.patch?raw";
import patch2 from "./unified-diff-2.patch?raw";
import patch3 from "./unified-diff-3.patch?raw";

const Progress = () => {
  const [viewStyle, setViewStyle] = React.useState<DiffViewStyle>("split");
  const [showLineNumbers, setShowLineNumbers] = React.useState(true);
  const [highlightInline, setHighlightInline] = React.useState(true);

  const patches = [patch1, patch2, patch3];

  const [autoSwitch, setAutoSwitch] = React.useState(false);
  const [patchIndex, setPatchIndex] = React.useState(0);

  const selectedPatch = patches[patchIndex];

  React.useEffect(() => {
    if (!autoSwitch) return;

    const id = setInterval(() => {
      setPatchIndex((i) => (i + 1) % patches.length);
    }, 1); // 1ms = aggressive stress test

    return () => clearInterval(id);
  }, [autoSwitch]);

  if (!selectedPatch) {
    return <div>No diff to display</div>;
  }

  return (
    <div className="relative min-h-screen w-full">
      <div className="max-w-screen mx-auto">
        <h1 className="text-2xl font-mono text-center mb-8">
          @gitru/diff Demo
        </h1>

        {/* Controls */}
        <div className="flex gap-4 mb-6 justify-center flex-wrap">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={autoSwitch}
              onChange={(e) => setAutoSwitch(e.target.checked)}
            />
            <span className="text-sm">Auto switch patches (perf test)</span>
          </label>

          {patches.map((_, idx) => (
            <button
              key={idx}
              onClick={() => {
                setPatchIndex(idx);
              }}
              className={
                idx === patchIndex
                  ? "font-bold underline bg-red-200"
                  : "font-normal underline"
              }
            >
              Patch {idx + 1}
            </button>
          ))}

          <label className="flex items-center gap-2">
            <span className="text-sm">View Style:</span>
            <select
              value={viewStyle}
              onChange={(e) => setViewStyle(e.target.value as DiffViewStyle)}
              className="px-2 py-1 rounded border bg-background"
            >
              <option value="split">Split</option>
              <option value="unified">Unified</option>
            </select>
          </label>

          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={showLineNumbers}
              onChange={(e) => setShowLineNumbers(e.target.checked)}
            />
            <span className="text-sm">Line Numbers</span>
          </label>

          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={highlightInline}
              onChange={(e) => setHighlightInline(e.target.checked)}
            />
            <span className="text-sm">Inline Diff</span>
          </label>
        </div>

        {/* Diff Viewer - uses patch prop, parses internally */}
        <div className="border rounded-lg overflow-hidden">
          <DiffViewer
            patch={selectedPatch}
            options={{
              style: viewStyle,
              showLineNumbers,
              highlightInlineDiff: highlightInline,
              theme: "vesper",
              showDiffIndicators: true,
              wrapLines: false,
            }}
            onLineClick={(line, side) => {
              console.log("Clicked line:", line, "side:", side);
            }}
          />
        </div>
      </div>
    </div>
  );
};

export default Progress;
