import { GraphRow } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { Tag, Tags } from "lucide-react";

type GraphLaneProps = {
  row: GraphRow;
  maxLane: number;
};

// ═══════════════════════════════════════════════════════════════════
// LAYOUT CONSTANTS
// ═══════════════════════════════════════════════════════════════════

/** Horizontal spacing between lane centers */
const LANE_W = 20 * 1.5;
/** Row height – each commit occupies this many px vertically */
const ROW_H = 36 * 1.5;
/** Commit dot radius */
const DOT_R = 4.5 * 3;
/** Line stroke width */
const LINE_W = 4;

// ═══════════════════════════════════════════════════════════════════
// CURVE SETTINGS - Tweak these to adjust path smoothness
// ═══════════════════════════════════════════════════════════════════

/** Radius for single-lane shift S-curves */
const SHIFT_CURVE_R = 2;
/** Start Y position for curves (distance from top) */
const CURVE_START_Y = 6;
/**
 * Bezier control point factor for single-lane smooth curves.
 * 0.25 = control point at 25% of horizontal distance
 * Higher values = sharper curves, Lower values = gentler curves
 */
const BEZIER_CTRL_FACTOR = 0.25;
/**
 * Bezier midpoint factor for single-lane smooth curves.
 * 0.5 = curve reaches midpoint at 50% of horizontal distance
 */
const BEZIER_MID_FACTOR = 0.5;

// ═══════════════════════════════════════════════════════════════════
// COLOR PALETTE
// ═══════════════════════════════════════════════════════════════════

// const PALETTE = [
//   "oklch(0.73 0.20 0)",
//   "oklch(0.73 0.20 36)",
//   "oklch(0.73 0.20 72)",
//   "oklch(0.73 0.20 108)",
//   "oklch(0.73 0.20 144)",
//   "oklch(0.73 0.20 180)",
//   "oklch(0.73 0.20 216)",
//   "oklch(0.73 0.20 252)",
//   "oklch(0.73 0.20 288)",
//   "oklch(0.73 0.20 324)",
// ] as const;

const PALETTE = [
  // "#A8ACFF",
  // "#5DBFFF",
  // "#00D7FF",
  // "#00E4C3",
  // "#72DA5A",
  // "#DBBE00",
  // "#FF9A00",
  // "#FF8289",
  // "#FF81B9",
  "oklch(0.773 0.118 281.135)",
  "oklch(0.772 0.130 240.067)",
  "oklch(0.811 0.146 217.709)",
  "oklch(0.822 0.152 177.146)",
  "oklch(0.798 0.193 140.004)",
  "oklch(0.802 0.166 98.070)",
  "oklch(0.774 0.174 65.052)",
  "oklch(0.749 0.152 17.942)",
  "oklch(0.761 0.164 353.728)",
] as const;

// ═══════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════

/** Convert lane index to X coordinate */
const laneToX = (laneIndex: number): number => LANE_W * (laneIndex + 1);

/**
 * Build a smooth single-lane curve path (used for 1-lane shifts and convergences)
 * Creates an S-curve from inX to outX at midY height
 */
const buildSingleLaneCurve = (
  inX: number,
  outX: number,
  startY: number,
  midY: number,
  endY?: number,
): string => {
  const dx = outX - inX;
  const curve = Math.min(Math.abs(dx) * 0.5, SHIFT_CURVE_R);

  if (endY === undefined) {
    // Convergence: ends at midY horizontally
    return [
      `M ${inX} 0`,
      `V ${startY}`,
      `C ${inX} ${midY - curve},`,
      `${inX + dx * BEZIER_CTRL_FACTOR} ${midY},`,
      `${inX + dx * BEZIER_MID_FACTOR} ${midY}`,
      `H ${outX}`,
    ].join(" ");
  } else {
    // Smooth S-curve with symmetrical control points
    const dy = endY - startY;
    const midPointY = startY + dy * 0.5;

    // Control point distance from start/end (30-40% of total vertical distance works well)
    const ctrlDistance = Math.abs(dy) * 0.35;

    return [
      `M ${inX} 0`,
      `V ${startY}`,
      // Single smooth S-curve using cubic Bezier
      // First control point: extends vertically from start
      // Second control point: extends vertically into end
      `C ${inX} ${startY + ctrlDistance},`,
      `${outX} ${endY - ctrlDistance},`,
      `${outX} ${endY}`,
    ].join(" ");
  }
};

/**
 * Build a multi-lane curve path (for shifts/convergences spanning 2+ lanes)
 * Uses 1-lane curve + straight section + optional exit curve
 */
const buildMultiLaneCurve = (
  inX: number,
  outX: number,
  startY: number,
  midY: number,
  dir: number,
  endY?: number,
): string => {
  const firstCurveX = inX + dir * LANE_W;
  const curve = SHIFT_CURVE_R;

  if (endY === undefined) {
    // Convergence: entry curve + horizontal straight
    return [
      `M ${inX} 0`,
      `V ${startY}`,
      `C ${inX} ${midY - curve},`,
      `${inX + dir * LANE_W * 0.5} ${midY},`,
      `${firstCurveX} ${midY}`,
      `H ${outX}`,
    ].join(" ");
  } else {
    // Smooth S-curve with BOTH vertical and horizontal control points
    const dy = endY - startY;
    const dx = outX - inX;

    // Control point distances
    const ctrlDistanceY = Math.abs(dy) * 0.7; // Vertical control
    const ctrlDistanceX = Math.abs(dx) * 0; // Horizontal control

    return [
      `M ${inX} 0`,
      `V ${startY}`,
      // S-curve with diagonal flow
      // First control point: extends vertically DOWN and horizontally TOWARD target
      // Second control point: extends vertically UP and horizontally FROM source
      `C ${inX + ctrlDistanceX * Math.sign(dx)} ${startY + ctrlDistanceY * Math.sign(dy)},`,
      `${outX - ctrlDistanceX * Math.sign(dx)} ${endY - ctrlDistanceY * Math.sign(dy)},`,
      `${outX} ${endY}`,
    ].join(" ");
  }
};

/**
 * Build path for a lane converging to the commit dot.
 * Handles both single-lane and multi-lane convergence.
 */
const buildConvergencePath = (
  inLane: number,
  outLane: number,
  midY: number,
): string => {
  const inX = laneToX(inLane);
  const outX = laneToX(outLane);
  const laneDelta = outLane - inLane;
  const absDelta = Math.abs(laneDelta);
  const dir = Math.sign(laneDelta);

  if (absDelta === 1) {
    return buildSingleLaneCurve(inX, outX, CURVE_START_Y, midY);
  } else {
    return buildMultiLaneCurve(inX, outX, CURVE_START_Y, midY, dir);
  }
};

/**
 * Build path for a lane shifting columns (pass-through with column change).
 * Handles both single-lane and multi-lane shifts.
 */
const buildShiftPath = (
  inLane: number,
  outLane: number,
  midY: number,
): string => {
  const inX = laneToX(inLane);
  const outX = laneToX(outLane);
  const laneDelta = outLane - inLane;
  const absDelta = Math.abs(laneDelta);
  const dir = Math.sign(laneDelta);

  if (absDelta === 1) {
    return buildSingleLaneCurve(inX, outX, CURVE_START_Y, midY, ROW_H);
  } else {
    return buildMultiLaneCurve(inX, outX, CURVE_START_Y, midY, dir, ROW_H);
  }
};

// ═══════════════════════════════════════════════════════════════════
// MAIN COMPONENT
// ═══════════════════════════════════════════════════════════════════

/**
 * Renders a Git graph lane for a single commit row.
 * Follows VS Code's SCM history graph rendering algorithm.
 *
 * Draws:
 * - Vertical pass-through lanes
 * - Converging lanes (merging into commit)
 * - Shifting lanes (column changes)
 * - Parent branches (secondary parents)
 * - Commit dot (single or double circle for merges)
 */
export const GraphLane = ({ row, maxLane }: GraphLaneProps) => {
  const width = Math.max((maxLane + 2) * LANE_W + 8, LANE_W * 3);
  const isStash = row.type === "Stash";

  const input = row.input_swimlanes;
  const output = row.output_swimlanes;

  // Find commit position in input swimlanes (or at end if new)
  const inputIdx = input.findIndex((n) => n.id === row.oid);
  const circleIdx = inputIdx !== -1 ? inputIdx : input.length;

  // Commit dot coordinates
  const cx = laneToX(circleIdx);
  const cy = ROW_H / 2;

  // Determine commit dot color
  const circleColor =
    circleIdx < output.length
      ? PALETTE[output[circleIdx].color % PALETTE.length]
      : circleIdx < input.length
        ? PALETTE[input[circleIdx].color % PALETTE.length]
        : PALETTE[0];

  // ───────────────────────────────────────────────────────────────
  // Build SVG paths for all lanes
  // ───────────────────────────────────────────────────────────────

  const paths: React.ReactNode[] = [];
  let outputSwimlaneIndex = 0;

  for (let index = 0; index < input.length; index++) {
    const color = PALETTE[input[index].color % PALETTE.length];
    const isDashed = input[index].is_stash;

    if (input[index].id === row.oid) {
      // Lane targets this commit
      if (index !== circleIdx) {
        // Converging lane (not primary) - draw arc to commit
        const d = buildConvergencePath(index, circleIdx, cy);

        paths.push(
          <path
            key={`conv-${index}`}
            d={d}
            fill="none"
            stroke={color}
            strokeWidth={LINE_W}
            strokeLinecap="round"
            strokeDasharray={isDashed ? "4 3" : undefined}
          />,
        );
      } else {
        // Primary lane - increment output index (vertical lines drawn separately)
        outputSwimlaneIndex++;
      }
    } else {
      // Lane does not target this commit - pass through or shift
      if (
        outputSwimlaneIndex < output.length &&
        input[index].id === output[outputSwimlaneIndex].id
      ) {
        const outColor =
          PALETTE[output[outputSwimlaneIndex].color % PALETTE.length];
        const outDashed = output[outputSwimlaneIndex].is_stash;

        if (index === outputSwimlaneIndex) {
          const x = Math.round(laneToX(index));
          // Same column - straight vertical line
          paths.push(
            <line
              key={`pass-${index}`}
              x1={x}
              y1={0}
              x2={laneToX(index)}
              y2={ROW_H}
              stroke={outColor}
              strokeWidth={LINE_W}
              strokeLinecap="round"
              strokeDasharray={outDashed ? "4 3" : undefined}
            />,
          );
        } else {
          // Column shift - draw S-curve
          const d = buildShiftPath(index, outputSwimlaneIndex, cy);

          paths.push(
            <path
              key={`shift-${index}`}
              d={d}
              fill="none"
              stroke={outColor}
              strokeWidth={LINE_W}
              strokeLinecap="round"
              strokeDasharray={outDashed ? "4 3" : undefined}
            />,
          );
        }

        outputSwimlaneIndex++;
      }
    }
  }

  // ───────────────────────────────────────────────────────────────
  // Secondary parent branches
  // ───────────────────────────────────────────────────────────────

  for (let i = 1; i < row.parents.length; i++) {
    const parentOid = row.parents[i].oid;

    // Find parent in output swimlanes
    let parentOutIdx = -1;
    for (let j = output.length - 1; j >= 0; j--) {
      if (output[j].id === parentOid) {
        parentOutIdx = j;
        break;
      }
    }
    if (parentOutIdx === -1) continue;

    const parentX = laneToX(parentOutIdx);
    const parentColor = PALETTE[output[parentOutIdx].color % PALETTE.length];

    if (parentX === cx) continue; // Same column, already drawn

    // Arc from midY horizontal down to parent's output lane
    const arcStartX =
      parentOutIdx > circleIdx ? parentX - LANE_W : parentX + LANE_W;
    const sweepFlag = parentOutIdx > circleIdx ? 1 : 0;

    const d = [
      `M ${arcStartX} ${cy}`,
      `A ${LANE_W} ${LANE_W} 0 0 ${sweepFlag} ${parentX} ${ROW_H}`,
      `M ${arcStartX} ${cy}`,
      `H ${cx}`,
    ].join(" ");

    paths.push(
      <path
        key={`parent-${parentOid}-${i}`}
        d={d}
        fill="none"
        stroke={parentColor}
        strokeWidth={LINE_W}
        strokeLinecap="round"
        strokeDasharray={isStash ? "4 3" : undefined}
      />,
    );
  }

  // ───────────────────────────────────────────────────────────────
  // Vertical lines to/from commit dot
  // ───────────────────────────────────────────────────────────────

  // Line from top to commit dot (if commit existed in input)
  if (inputIdx !== -1) {
    const topColor = PALETTE[input[inputIdx].color % PALETTE.length];
    paths.push(
      <line
        key="to-dot"
        x1={cx}
        y1={0}
        x2={cx}
        y2={cy}
        stroke={topColor}
        strokeWidth={LINE_W}
        strokeLinecap="round"
        strokeDasharray={
          isStash || input[inputIdx].is_stash ? "4 3" : undefined
        }
      />,
    );
  }

  // Line from commit dot to bottom (if has parents)
  if (row.parents.length > 0) {
    paths.push(
      <line
        key="from-dot"
        x1={cx}
        y1={cy}
        x2={cx}
        y2={ROW_H}
        stroke={circleColor}
        strokeWidth={LINE_W}
        strokeLinecap="round"
        strokeDasharray={isStash ? "4 3" : undefined}
      />,
    );
  }

  // ───────────────────────────────────────────────────────────────
  // Render SVG
  // ───────────────────────────────────────────────────────────────

  return (
    <svg
      width={width}
      height={ROW_H}
      className="shrink-0 overflow-visible"
      aria-hidden="true"
      style={{
        // render in GPU layer for smoother animations
        transform: "translateZ(0)",
        willChange: "transform",
      }}
    >
      {paths}

      {/* Commit dot - double circle for merges, solid for regular commits */}
      {row.parents.length > 1 ? (
        <g>
          <circle
            cx={cx}
            cy={cy}
            r={DOT_R + 2}
            fill="none"
            stroke={circleColor}
            strokeWidth={1.5}
          />
          <circle
            cx={cx}
            cy={cy}
            r={DOT_R - 1}
            fill="none"
            stroke={circleColor}
            strokeWidth={1.5}
          />
        </g>
      ) : (
        // <g>
        //   <circle
        //     cx={cx}
        //     cy={cy}
        //     r={DOT_R + 2}
        //     fill="none"
        //     stroke={circleColor}
        //     strokeWidth={4}
        //   />
        //   <circle
        //     cx={cx}
        //     cy={cy}
        //     r={DOT_R + 1}
        //     // fill={circleColor}
        //     fill={"var(--color-background, #1e1e1e)"}
        //     stroke="var(--color-background, #1e1e1e)"
        //     strokeWidth={3}
        //   />
        // </g>
        <g>
          <circle
            cx={cx}
            cy={cy}
            r={DOT_R + 2}
            fill="var(--color-background, #1e1e1e)"
            shapeRendering="crispEdges"
            stroke={circleColor}
            strokeWidth={3}
          />
        </g>
      )}
      <foreignObject x={cx - 12} y={cy - 12.3} width={24} height={24}>
        <div className="relative">
          <Avatar className="rounded-full size-6 overflow-visible">
            <AvatarImage
              alt="User"
              className={"rounded-full"}
              src={`https://avatars.githubusercontent.com/u/e?email=${row.commit.authors.author.email}&s=24`}
            />
            <AvatarFallback>AV</AvatarFallback>
          </Avatar>
          {/* Render Tags */}
          {row.tags.length > 0 && (
            <Tooltip>
              <TooltipTrigger>
                <Badge
                  className="-end-1.5 -bottom-1.5 absolute size-4.5 rounded-full outline-1 outline-solid bg-background"
                  style={{
                    outlineColor: circleColor,
                  }}
                  size="sm"
                  variant={"outline"}
                >
                  {row.tags.length === 1 ? <Tag /> : <Tags />}
                </Badge>
              </TooltipTrigger>
              <TooltipPopup>
                {row.tags.map((tag) => (
                  <div key={tag.name} className="text-sm">
                    <span className="text-muted-foreground">Tag:</span>{" "}
                    {tag.name.replace("refs/tags/", "")}
                  </div>
                ))}
              </TooltipPopup>
            </Tooltip>
          )}

          {/* Render Stash */}

          {isStash && (
            <Tooltip>
              <TooltipTrigger>
                <Badge
                  className="-end-1.5 -bottom-1.5 absolute size-4.5 rounded-full outline-1 outline-solid bg-background"
                  style={{
                    outlineColor: circleColor,
                  }}
                  size="sm"
                  variant={"outline"}
                >
                  S
                </Badge>
              </TooltipTrigger>
              <TooltipPopup>
                <div className="text-sm">
                  <span className="text-muted-foreground">Stash:</span>{" "}
                  {row.stashes.map((s) => s.name).join(", ")}
                </div>
              </TooltipPopup>
            </Tooltip>
          )}
        </div>
      </foreignObject>
    </svg>
  );
};
