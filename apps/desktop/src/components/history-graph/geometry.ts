export const LANE_W = 20;
export const ROW_H = 32;
export const DOT_R = 12;
export const LINE_W = 2;
const SHIFT_CURVE_R = 2;

export const laneToX = (laneIndex: number) => LANE_W * (laneIndex + 1);

function singleLaneCurve(
  inX: number,
  outX: number,
  midY: number,
  endY?: number,
) {
  const dx = outX - inX;
  if (endY === undefined) {
    const curve = Math.min(Math.abs(dx) * 0.5, SHIFT_CURVE_R);
    return [
      `M ${inX} 0`,
      "V 0",
      `C ${inX} ${midY - curve},`,
      `${inX + dx * 0.25} ${midY},`,
      `${inX + dx * 0.5} ${midY}`,
      `H ${outX}`,
    ].join(" ");
  }

  const controlDistance = Math.abs(endY) * 0.35;
  return [
    `M ${inX} 0`,
    "V 0",
    `C ${inX} ${controlDistance},`,
    `${outX} ${endY - controlDistance},`,
    `${outX} ${endY}`,
  ].join(" ");
}

function multiLaneCurve(
  inX: number,
  outX: number,
  midY: number,
  direction: number,
  endY?: number,
) {
  const firstCurveX = inX + direction * LANE_W;
  if (endY === undefined) {
    return [
      `M ${inX} 0`,
      "V 0",
      `C ${inX} ${midY - SHIFT_CURVE_R},`,
      `${inX + direction * LANE_W * 0.5} ${midY},`,
      `${firstCurveX} ${midY}`,
      `H ${outX}`,
    ].join(" ");
  }

  const lastCurveX = outX - direction * LANE_W;
  const controlOffset = (direction * LANE_W) / 2;
  return [
    `M ${inX} 0`,
    "V 0",
    `C ${inX} ${midY}, ${inX + controlOffset} ${midY}, ${firstCurveX} ${midY}`,
    `H ${lastCurveX}`,
    `C ${outX - controlOffset} ${midY}, ${outX} ${midY}, ${outX} ${endY}`,
  ].join(" ");
}

function laneCurve(inLane: number, outLane: number, endY?: number) {
  const inX = laneToX(inLane);
  const outX = laneToX(outLane);
  const delta = outLane - inLane;
  return Math.abs(delta) === 1
    ? singleLaneCurve(inX, outX, ROW_H / 2, endY)
    : multiLaneCurve(inX, outX, ROW_H / 2, Math.sign(delta), endY);
}

export const buildConvergencePath = (inLane: number, outLane: number) =>
  laneCurve(inLane, outLane);

export const buildShiftPath = (inLane: number, outLane: number) =>
  laneCurve(inLane, outLane, ROW_H);

export const graphWidth = (maxLane: number) =>
  Math.max((maxLane + 2) * LANE_W + 8, LANE_W * 3);
