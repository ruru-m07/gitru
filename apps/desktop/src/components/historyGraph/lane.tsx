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

/**
 * LANE_W - Horizontal spacing between lane centers
 * ROW_H - Vertical height of each commit row
 * DOT_R - Radius of the commit dot
 * LINE_W - Stroke width for lane lines
 * SHIFT_CURVE_R - Radius for single-lane shift curves
 * CURVE_START_Y - Y position where curves start (distance from top)
 * BEZIER_CTRL_FACTOR - Control point factor for single-lane curves (0.25 = 25% of horizontal distance)
 * BEZIER_MID_FACTOR - Midpoint factor for single-lane curves (0.5 = midpoint at 50% of horizontal distance)
 * PALETTE - Array of colors for lanes, indexed by lane color value modulo palette length
 */
const LANE_W = 20 * 1.5;
const ROW_H = 36 * 1.5;
const DOT_R = 4.5 * 3;
const LINE_W = 4;
const SHIFT_CURVE_R = 2;
const CURVE_START_Y = 6;
const BEZIER_CTRL_FACTOR = 0.25;
const BEZIER_MID_FACTOR = 0.5;

const PALETTE = [
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

// ? Convert lane index to X coordinate
const laneToX = (laneIndex: number): number => LANE_W * (laneIndex + 1);

/**
 * ? Build a smooth single-lane curve path (used for 1-lane shifts and convergences)
 * ? Creates an S-curve from inX to outX at midY height
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
    // ? convergence - ends at midY horizontally
    return [
      `M ${inX} 0`,
      `V ${startY}`,
      `C ${inX} ${midY - curve},`,
      `${inX + dx * BEZIER_CTRL_FACTOR} ${midY},`,
      `${inX + dx * BEZIER_MID_FACTOR} ${midY}`,
      `H ${outX}`,
    ].join(" ");
  } else {
    // ? smooth `S` curve with symmetrical control points
    const dy = endY - startY;

    const ctrlDistance = Math.abs(dy) * 0.35;

    return [
      `M ${inX} 0`,
      `V ${startY}`,
      `C ${inX} ${startY + ctrlDistance},`,
      `${outX} ${endY - ctrlDistance},`,
      `${outX} ${endY}`,
    ].join(" ");
  }
};

/**
 * ? Build a multi-lane curve path (for shifts/convergences spanning 2+ lanes)
 * ? Uses 1-lane curve + straight section + optional exit curve
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

const GraphLane = ({ row, maxLane }: GraphLaneProps) => {
  const width = Math.max((maxLane + 2) * LANE_W + 8, LANE_W * 3);
  const isStash = row.type === "Stash";

  const input = row.input_swimlanes;
  const output = row.output_swimlanes;

  // Find commit position in input swimlanes (or at end if new)
  const inputIdx = input.findIndex((n) => n.id === row.oid);
  const circleIdx = inputIdx !== -1 ? inputIdx : input.length;

  /**
   * ? Commit dot coordinates
   * ? X is based on lane index (or next available lane if new commit)
   * ? Y is centered in the row
   * ? This is the anchor point for drawing all paths and the commit circle
   * ? Paths will connect to this point for convergences, shifts, and parent branches
   * ? The commit dot is drawn last to ensure it appears on top of all paths
   * ? For merges, the dot is a double circle to visually indicate multiple parents
   * ? The dot color is determined by the commit's lane color (or next lane if new)
   */
  const cx = laneToX(circleIdx);
  const cy = ROW_H / 2;

  // Determine commit dot color
  const circleColor =
    circleIdx < output.length
      ? PALETTE[output[circleIdx].color % PALETTE.length]
      : circleIdx < input.length
        ? PALETTE[input[circleIdx].color % PALETTE.length]
        : PALETTE[0];

  /**
   * ? paths will be a array of path/line elements
   * ? Draw lane paths in 3 passes to ensure correct layering:
   * * 1) Pass-through and converging lanes (behind commit dot)
   * * 2) Parent branches (behind commit dot but above pass-throughs)
   * * 3) Vertical lines to/from commit dot (on top)
   *
   */
  const paths: React.ReactNode[] = [];
  let outputSwimlaneIndex = 0;

  // ! draw pass-through lanes and converging lanes
  for (let index = 0; index < input.length; index++) {
    const color = PALETTE[input[index].color % PALETTE.length];
    const isDashed = input[index].is_stash;

    /**
     * ? If lane targets this commit (matches row OID), it's a convergence lane that merges into the commit dot.
     * * If it's not the primary lane (not at circleIdx), draw a curve converging into the commit dot.
     * * The curve type (single-lane or multi-lane) is determined by how many lanes it spans.
     * * Converging lanes visually indicate branches merging into this commit.
     * * They are drawn behind the commit dot to show they feed into it.
     * * The lane color is used for the stroke to maintain visual consistency.
     * ? If lane does not target this commit, it's a pass-through lane that either continues straight or shifts columns.
     * * If the lane exists in the output swimlanes at the current output index, it means it continues through this commit.
     * * If it's in the same column (index matches output index), draw a straight vertical line.
     * * If it shifts columns (index does not match output index), draw a curve that shifts to the new column.
     * * The curve type is determined by how many lanes it shifts across.
     * * Pass-through lanes visually indicate branches that continue through this commit without merging.
     * * They are drawn behind the commit dot to show they pass under it.
     * * The lane color is used for the stroke to maintain visual consistency.
     * * The output swimlane index is incremented when a lane is processed to keep track of which output lane we're at.
     */
    if (input[index].id === row.oid) {
      if (index !== circleIdx) {
        const d = buildConvergencePath(index, circleIdx, cy);

        // ! this path responsible for drawing converging lanes that merge into the commit dot
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
        outputSwimlaneIndex++;
      }
    } else {
      if (
        outputSwimlaneIndex < output.length &&
        input[index].id === output[outputSwimlaneIndex].id
      ) {
        const outColor =
          PALETTE[output[outputSwimlaneIndex].color % PALETTE.length];
        const outDashed = output[outputSwimlaneIndex].is_stash;

        // ? reast of lanes that pass through without merging
        // ! the else block is responsible for drawing column shifts for pass-through lanes
        if (index === outputSwimlaneIndex) {
          const x = Math.round(laneToX(index));
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

  // ! Secondary parent branches
  // ? more like those merge lanes but for non-primary parents
  // ? drawn behind commit dot but above pass-throughs
  for (let i = 1; i < row.parents.length; i++) {
    const parentOid = row.parents[i].oid;

    // * Find parent in output swimlanes
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

    // * Same column, already drawn
    if (parentX === cx) continue;

    // *Arc from midY horizontal down to parent's output lane
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

  // ! Vertical lines to/from commit dot
  // ? ↑ Line from top to commit dot (if commit existed in input)
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

  // ? ↓ Line from commit dot to bottom (if has parents)
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

  return (
    <svg
      width={width}
      height={ROW_H}
      className="shrink-0 overflow-visible"
      aria-hidden="true"
      style={{
        transform: "translateZ(0)",
        willChange: "transform",
      }}
    >
      {paths}

      {/* // ! Commit dot - double circle for merges, solid for regular commits */}
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
            <AvatarFallback></AvatarFallback>
          </Avatar>
          {/* // ! Render Tags */}
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

          {/* // ! Render Stash */}
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
                  ◌
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

export default GraphLane;
