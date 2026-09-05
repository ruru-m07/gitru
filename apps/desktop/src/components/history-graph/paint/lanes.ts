import type { ColumnSlot } from "@gitru/dom-virtual";
import {
  buildConvergencePath,
  buildShiftPath,
  DOT_R,
  graphWidth,
  LANE_W,
  LINE_W,
  laneToX,
  ROW_H,
} from "../geometry";
import type { ProcessedRow } from "../helper";
import { PALETTE } from "../palette";

const SVG_NS = "http://www.w3.org/2000/svg";

function svgElement<K extends keyof SVGElementTagNameMap>(
  name: K,
  attributes: Record<string, string | number | undefined>,
) {
  const element = document.createElementNS(SVG_NS, name);
  for (const [key, value] of Object.entries(attributes)) {
    if (value !== undefined) element.setAttribute(key, String(value));
  }
  return element;
}

function strokeAttributes(color: string, dashed: boolean) {
  return {
    fill: "none",
    stroke: color,
    "stroke-width": LINE_W,
    "stroke-linecap": "round",
    "stroke-dasharray": dashed ? "4 3" : undefined,
  };
}

export function laneContentWidth(maxLane: number) {
  return graphWidth(maxLane);
}

export function createLanesSlot(
  getMaxLane: () => number,
): ColumnSlot<ProcessedRow> {
  const element = document.createElement("div");
  element.className = "relative";
  Object.assign(element.style, {
    contain: "layout paint",
    height: `${ROW_H}px`,
  });

  return {
    element,
    paint(processedRow) {
      const row = processedRow.row;
      element.dataset.cell = "";
      element.dataset.cellId = row.oid;
      delete element.dataset.rowHovered;
      element.replaceChildren();

      const input = row.input_swimlanes;
      const output = row.output_swimlanes;
      const inputIndex = input.findIndex((lane) => lane.id === row.oid);
      const circleIndex = inputIndex === -1 ? input.length : inputIndex;
      const cx = laneToX(circleIndex);
      const cy = ROW_H / 2;
      const isStash = row.type === "Stash";

      const background = document.createElement("span");
      background.className = "absolute bottom-0 right-0 top-0 z-0 mt-0.5";
      background.style.left = `${cx}px`;
      background.style.height = `${ROW_H - 4}px`;
      background.style.backgroundColor = `color-mix(in oklab, ${processedRow.color} 15%, var(--color-background))`;
      element.appendChild(background);

      if (row.refs.length > 0) {
        const connector = document.createElement("span");
        connector.className = "absolute left-0 z-0 h-0.5";
        connector.style.top = `${cy - 1}px`;
        connector.style.width = `${cx}px`;
        connector.style.backgroundColor = `color-mix(in oklab, ${processedRow.color} 50%, var(--color-background))`;
        element.appendChild(connector);
      }

      const svg = svgElement("svg", {
        width: graphWidth(getMaxLane()),
        height: ROW_H,
        "aria-hidden": "true",
        class: "relative z-10 shrink-0 overflow-visible",
      });
      let outputIndex = 0;

      for (let index = 0; index < input.length; index++) {
        const inputLane = input[index];
        if (!inputLane) continue;
        const color = PALETTE[inputLane.color % PALETTE.length] ?? PALETTE[0];

        if (inputLane.id === row.oid) {
          if (index !== circleIndex) {
            svg.appendChild(
              svgElement("path", {
                d: buildConvergencePath(index, circleIndex),
                ...strokeAttributes(color, inputLane.is_stash),
              }),
            );
          } else {
            outputIndex++;
          }
          continue;
        }

        const outputLane = output[outputIndex];
        if (!outputLane || inputLane.id !== outputLane.id) continue;
        const outputColor =
          PALETTE[outputLane.color % PALETTE.length] ?? PALETTE[0];
        svg.appendChild(
          index === outputIndex
            ? svgElement("line", {
                x1: laneToX(index),
                y1: 0,
                x2: laneToX(index),
                y2: ROW_H,
                ...strokeAttributes(outputColor, outputLane.is_stash),
              })
            : svgElement("path", {
                d: buildShiftPath(index, outputIndex),
                ...strokeAttributes(outputColor, outputLane.is_stash),
              }),
        );
        outputIndex++;
      }

      for (let index = 1; index < row.parents.length; index++) {
        const parent = row.parents[index];
        if (!parent) continue;
        let parentOutputIndex = -1;
        for (let laneIndex = output.length - 1; laneIndex >= 0; laneIndex--) {
          if (output[laneIndex]?.id === parent.oid) {
            parentOutputIndex = laneIndex;
            break;
          }
        }
        if (parentOutputIndex < 0) continue;
        const parentX = laneToX(parentOutputIndex);
        if (parentX === cx) continue;
        const direction = Math.sign(parentOutputIndex - circleIndex);
        const arcStartX = parentX - direction * LANE_W;
        const parentColor =
          PALETTE[output[parentOutputIndex]!.color % PALETTE.length] ??
          PALETTE[0];
        svg.appendChild(
          svgElement("path", {
            d: [
              `M ${arcStartX} ${cy}`,
              `C ${arcStartX} ${cy}, ${parentX} ${ROW_H - LANE_W}, ${parentX} ${ROW_H}`,
              `M ${arcStartX} ${cy}`,
              `H ${cx}`,
            ].join(" "),
            ...strokeAttributes(parentColor, isStash),
          }),
        );
      }

      if (inputIndex >= 0) {
        const inputLane = input[inputIndex]!;
        const color = PALETTE[inputLane.color % PALETTE.length] ?? PALETTE[0];
        svg.appendChild(
          svgElement("line", {
            x1: cx,
            y1: 0,
            x2: cx,
            y2: cy,
            ...strokeAttributes(color, isStash || inputLane.is_stash),
          }),
        );
      }

      if (row.parents.length > 0) {
        svg.appendChild(
          svgElement("line", {
            x1: cx,
            y1: cy,
            x2: cx,
            y2: ROW_H,
            ...strokeAttributes(processedRow.color, isStash),
          }),
        );
      }

      if (row.parents.length > 1) {
        svg.append(
          svgElement("circle", {
            cx,
            cy,
            r: DOT_R + 1,
            fill: "none",
            stroke: processedRow.color,
            "stroke-width": 2,
          }),
          svgElement("circle", {
            cx,
            cy,
            r: DOT_R - 1,
            fill: "none",
            stroke: processedRow.color,
            "stroke-width": 2,
          }),
        );
      } else {
        svg.appendChild(
          svgElement("circle", {
            cx,
            cy,
            r: DOT_R + 1,
            fill: "var(--color-background)",
            stroke: processedRow.color,
            "stroke-width": 2,
          }),
        );
      }

      const foreignObject = svgElement("foreignObject", {
        x: cx - DOT_R,
        y: cy - DOT_R - 1,
        width: DOT_R * 2,
        height: DOT_R * 2,
      });
      const avatarRoot = document.createElement("div");
      avatarRoot.className =
        "relative inline-flex size-6 shrink-0 select-none items-center justify-center overflow-visible rounded-full border bg-background align-middle text-xs font-medium";
      const image = document.createElement("img");
      image.alt = row.commit.authors.author.name;
      image.className = "size-6 rounded-full object-cover";
      image.src = `https://avatars.githubusercontent.com/u/e?email=${encodeURIComponent(row.commit.authors.author.email)}&s=24`;
      image.addEventListener("error", () => image.remove(), { once: true });
      avatarRoot.appendChild(image);

      if (isStash) {
        const stash = document.createElement("span");
        stash.className =
          "absolute -bottom-1.5 -right-1.5 inline-flex size-4.5 shrink-0 select-none items-center justify-center whitespace-nowrap rounded-full border border-input bg-background text-[.625rem] font-medium text-foreground outline";
        stash.style.outlineColor = processedRow.color;
        stash.textContent = "◌";
        stash.dataset.tooltip = row.stashes.map((item) => item.name).join(", ");
        stash.dataset.tooltipMutedLabel = "Stash:";
        avatarRoot.appendChild(stash);
      }
      foreignObject.appendChild(avatarRoot);
      svg.appendChild(foreignObject);
      element.appendChild(svg);
    },
  };
}
