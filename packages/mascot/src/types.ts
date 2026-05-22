import type { MouseEventHandler, SVGProps } from "react";

export type MascotEyesVariant = "open" | "closed";
export type MascotMouthVariant = "neutral" | "open";

export type MascotExpression = {
  eyes: MascotEyesVariant;
  mouth: MascotMouthVariant;
};

export type MascotInteraction = "idle" | "hover" | "press" | "focus";

export type MascotExpressionMap = Partial<
  Record<MascotInteraction, MascotExpression>
>;

export type MascotBehavior = {
  hover?: boolean;
  press?: boolean;
  focus?: boolean;
  click?: boolean;
};

export type MascotTransition = {
  duration?: number;
  ease?: [number, number, number, number];
};

export type MascotParticlesConfig = {
  enabled?: boolean;
  count?: number;
  ttlMs?: number;
  sizeRange?: [number, number];
  offset?: { x: number; y: number };
  drift?: { x: number; y: [number, number] };
  rotationRange?: [number, number];
  staggerMs?: number;
};

export type MascotProps = {
  interaction?: MascotInteraction;
  defaultInteraction?: MascotInteraction;
  onInteractionChange?: (interaction: MascotInteraction) => void;
  expression?: MascotExpression;
  expressionMap?: MascotExpressionMap;
  behavior?: MascotBehavior;
  transition?: MascotTransition;
  particles?: MascotParticlesConfig;
  className?: string;
  svgProps?: Omit<
    SVGProps<SVGSVGElement>,
    | "onAnimationStart"
    | "onAnimationEnd"
    | "onAnimationIteration"
    | "onDragStart"
    | "onDrag"
    | "onDragEnd"
  >;
  onClick?: MouseEventHandler<SVGSVGElement>;
};
