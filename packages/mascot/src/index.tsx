"use client";

import { interpolate } from "flubber";
import type { SVGMotionProps } from "motion/react";
import { animate, motion, useMotionValue, useTransform } from "motion/react";
import type { FocusEvent, MouseEvent } from "react";
import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { MascotContext, MascotProvider } from "./context";
import { useMascot } from "./hook";
import { HeartSvg } from "./icons";
import type {
  MascotBehavior,
  MascotExpression,
  MascotEyesVariant,
  MascotInteraction,
  MascotMouthVariant,
  MascotParticlesConfig,
  MascotProps,
  MascotTransition,
} from "./types";

type HeartParticle = {
  id: number;
  x: number;
  y: number;
  dx: number;
  dy: number;
  rotate: number;
  size: number;
  delay: number;
};

const DEFAULT_TRANSITION: Required<MascotTransition> = {
  duration: 0.18,
  ease: [0.22, 1, 0.36, 1],
};

const DEFAULT_EXPRESSION_MAP: Record<MascotInteraction, MascotExpression> = {
  idle: { eyes: "open", mouth: "neutral" },
  hover: { eyes: "closed", mouth: "open" },
  press: { eyes: "closed", mouth: "open" },
  focus: { eyes: "open", mouth: "neutral" },
};

const DEFAULT_BEHAVIOR: Required<MascotBehavior> = {
  hover: true,
  press: true,
  focus: false,
  click: true,
};

const DEFAULT_PARTICLES: Required<MascotParticlesConfig> = {
  enabled: true,
  count: 6,
  ttlMs: 900,
  sizeRange: [30, 42],
  offset: { x: 0.4, y: 0.12 },
  drift: { x: 60, y: [90, 170] },
  rotationRange: [-18, 18],
  staggerMs: 40,
};

const buildEllipsePath = (cx: number, cy: number, rx: number, ry: number) =>
  `M ${cx - rx} ${cy} a ${rx} ${ry} 0 1 0 ${rx * 2} 0 a ${rx} ${ry} 0 1 0 ${-rx * 2} 0`;

const normalizeRange = (range: [number, number]) =>
  range[0] <= range[1] ? range : ([range[1], range[0]] as [number, number]);

const randomInRange = (range: [number, number]) => {
  const [min, max] = normalizeRange(range);
  return min + Math.random() * (max - min);
};

const resolveTransition = (
  transition?: MascotTransition,
): Required<MascotTransition> => ({
  ...DEFAULT_TRANSITION,
  ...transition,
});

const resolveBehavior = (
  behavior?: MascotBehavior,
): Required<MascotBehavior> => ({
  ...DEFAULT_BEHAVIOR,
  ...behavior,
});

const resolveParticles = (
  particles?: MascotParticlesConfig,
): Required<MascotParticlesConfig> => ({
  ...DEFAULT_PARTICLES,
  ...particles,
  offset: { ...DEFAULT_PARTICLES.offset, ...particles?.offset },
  drift: { ...DEFAULT_PARTICLES.drift, ...particles?.drift },
});

export const Mascot = (props: MascotProps) => {
  const context = useContext(MascotContext);
  const { interaction, defaultInteraction, onInteractionChange, ...svgProps } =
    props;

  if (context) {
    return <MascotSvg {...svgProps} />;
  }

  return (
    <MascotProvider
      interaction={interaction}
      defaultInteraction={defaultInteraction}
      onInteractionChange={onInteractionChange}
    >
      <MascotSvg {...svgProps} />
    </MascotProvider>
  );
};

type MascotSvgProps = Omit<
  MascotProps,
  "interaction" | "defaultInteraction" | "onInteractionChange"
>;

const MascotSvg = ({
  expression,
  expressionMap,
  behavior,
  transition,
  particles,
  className,
  svgProps,
  onClick,
}: MascotSvgProps) => {
  const { interaction, setInteraction } = useMascot();
  const svgRef = useRef<SVGSVGElement | null>(null);
  const heartIdRef = useRef(0);
  const [hearts, setHearts] = useState<HeartParticle[]>([]);
  const [isPointerOver, setIsPointerOver] = useState(false);
  const [isFocused, setIsFocused] = useState(false);

  const resolvedTransition = useMemo(
    () => resolveTransition(transition),
    [transition],
  );
  const resolvedBehavior = useMemo(() => resolveBehavior(behavior), [behavior]);
  const resolvedParticles = useMemo(
    () => resolveParticles(particles),
    [particles],
  );

  const resolvedExpression = useMemo(() => {
    if (expression) return expression;

    const map: Record<MascotInteraction, MascotExpression> = {
      ...DEFAULT_EXPRESSION_MAP,
      ...(expressionMap ?? {}),
    };

    return map[interaction] ?? DEFAULT_EXPRESSION_MAP.idle;
  }, [expression, expressionMap, interaction]);

  const svgPropsResolved = svgProps ?? {};
  const { className: svgClassName, ...restSvgProps } = svgPropsResolved;
  const motionSvgProps = restSvgProps as Omit<
    SVGMotionProps<SVGSVGElement>,
    "children"
  >;
  const mergedClassName = [
    "select-none will-change-transform origin-bottom scale-100 transition-transform",
    resolvedBehavior.click
      ? "cursor-pointer active:scale-y-98 active:scale-x-99"
      : "cursor-default",
    className,
    svgClassName,
  ]
    .filter(Boolean)
    .join(" ");

  const spawnHearts = useCallback(
    (x: number, y: number) => {
      const { count, ttlMs, sizeRange, drift, rotationRange, staggerMs } =
        resolvedParticles;
      const sizeBounds = normalizeRange(sizeRange);
      const rotationBounds = normalizeRange(rotationRange);
      const liftBounds = normalizeRange(drift.y);

      setHearts((prev) => {
        const next = [...prev];
        for (let i = 0; i < count; i++) {
          const id = ++heartIdRef.current;
          next.push({
            id,
            x,
            y,
            dx: (Math.random() * 2 - 1) * drift.x,
            dy: -randomInRange(liftBounds),
            rotate: randomInRange(rotationBounds),
            size: Math.floor(randomInRange(sizeBounds)),
            delay: (staggerMs / 1000) * i,
          });

          window.setTimeout(
            () => {
              setHearts((items) => items.filter((p) => p.id !== id));
            },
            ttlMs + i * 60,
          );
        }
        return next;
      });
    },
    [resolvedParticles],
  );

  const handleClick = useCallback(
    (event: MouseEvent<SVGSVGElement>) => {
      restSvgProps.onClick?.(event);
      onClick?.(event);
      if (event.defaultPrevented) return;
      if (!resolvedBehavior.click || !resolvedParticles.enabled) return;

      const svg = svgRef.current;
      if (!svg) return;

      const rect = svg.getBoundingClientRect();
      const x = rect.left + rect.width * resolvedParticles.offset.x;
      const y = rect.top + rect.height * resolvedParticles.offset.y;

      spawnHearts(x, y);
    },
    [
      onClick,
      resolvedBehavior.click,
      resolvedParticles,
      restSvgProps,
      spawnHearts,
    ],
  );

  const handleMouseEnter = useCallback(
    (event: MouseEvent<SVGSVGElement>) => {
      restSvgProps.onMouseEnter?.(event);
      setIsPointerOver(true);
      if (resolvedBehavior.hover) {
        setInteraction("hover");
      }
    },
    [resolvedBehavior.hover, restSvgProps, setInteraction],
  );

  const handleMouseLeave = useCallback(
    (event: MouseEvent<SVGSVGElement>) => {
      restSvgProps.onMouseLeave?.(event);
      setIsPointerOver(false);
      if (resolvedBehavior.hover) {
        if (isFocused && resolvedBehavior.focus) {
          setInteraction("focus");
        } else {
          setInteraction("idle");
        }
      }
    },
    [
      isFocused,
      resolvedBehavior.focus,
      resolvedBehavior.hover,
      restSvgProps,
      setInteraction,
    ],
  );

  const handleMouseDown = useCallback(
    (event: MouseEvent<SVGSVGElement>) => {
      restSvgProps.onMouseDown?.(event);
      if (resolvedBehavior.press) {
        setInteraction("press");
      }
    },
    [resolvedBehavior.press, restSvgProps, setInteraction],
  );

  const handleMouseUp = useCallback(
    (event: MouseEvent<SVGSVGElement>) => {
      restSvgProps.onMouseUp?.(event);
      if (!resolvedBehavior.press) return;

      if (isPointerOver && resolvedBehavior.hover) {
        setInteraction("hover");
        return;
      }

      if (isFocused && resolvedBehavior.focus) {
        setInteraction("focus");
        return;
      }

      setInteraction("idle");
    },
    [
      isFocused,
      isPointerOver,
      resolvedBehavior.focus,
      resolvedBehavior.hover,
      resolvedBehavior.press,
      restSvgProps,
      setInteraction,
    ],
  );

  const handleFocus = useCallback(
    (event: FocusEvent<SVGSVGElement>) => {
      restSvgProps.onFocus?.(event);
      if (!resolvedBehavior.focus) return;
      setIsFocused(true);
      if (!isPointerOver) {
        setInteraction("focus");
      }
    },
    [isPointerOver, resolvedBehavior.focus, restSvgProps, setInteraction],
  );

  const handleBlur = useCallback(
    (event: FocusEvent<SVGSVGElement>) => {
      restSvgProps.onBlur?.(event);
      setIsFocused(false);
      if (resolvedBehavior.hover) {
        setInteraction(isPointerOver ? "hover" : "idle");
        return;
      }
      if (resolvedBehavior.focus) {
        setInteraction("idle");
      }
    },
    [
      isPointerOver,
      resolvedBehavior.focus,
      resolvedBehavior.hover,
      restSvgProps,
      setInteraction,
    ],
  );

  return (
    <>
      <div className="pointer-events-none fixed inset-0 z-50">
        {hearts.map((p) => (
          <motion.div
            key={p.id}
            className="absolute"
            initial={{ x: p.x, y: p.y, scale: 0.6, opacity: 0 }}
            animate={{
              x: p.x + p.dx,
              y: p.y + p.dy,
              scale: 1,
              opacity: [0, 1, 1, 0],
              rotate: p.rotate,
            }}
            transition={{
              duration: 0.9,
              ease: [0.22, 1, 0.36, 1],
              delay: p.delay,
            }}
          >
            <HeartSvg size={p.size} />
          </motion.div>
        ))}
      </div>

      <motion.svg
        ref={svgRef}
        {...motionSvgProps}
        onClick={handleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onFocus={handleFocus}
        onBlur={handleBlur}
        className={mergedClassName}
        width="512"
        height="512"
        viewBox="0 0 512 512"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        data-name="mascot-svg"
      >
        <g clipPath="url(#clip0_309_3022)">
          <MascotBody />
          <MascotEar />
          <MascotEyes
            variant={resolvedExpression.eyes}
            transition={resolvedTransition}
          />
          <MascotBlush />
          <MascotMouth
            variant={resolvedExpression.mouth}
            transition={resolvedTransition}
          />
          <MascotLegs />
        </g>
        <defs>
          <clipPath id="clip0_309_3022">
            <rect
              width="512"
              height="494"
              fill="white"
              transform="translate(0 9)"
            />
          </clipPath>
        </defs>
      </motion.svg>
    </>
  );
};

const EYE_VARIANTS: Record<
  MascotEyesVariant,
  {
    leftPath: string;
    rightPath: string;
    sparkleOpacity: number;
    sparkleScale: number;
  }
> = {
  open: {
    leftPath: buildEllipsePath(188.768, 207.03, 36.7914, 45.5902),
    rightPath: buildEllipsePath(322.112, 207.03, 36.7914, 45.5902),
    sparkleOpacity: 1,
    sparkleScale: 1,
  },
  closed: {
    leftPath:
      "M179 166C200.539 166 218 184.101 218 206.429C218 208.259 215.433 208.446 214.716 206.762C208.674 192.587 194.961 182.691 179.01 182.691C163.058 182.691 149.344 192.587 143.302 206.762C142.58 208.456 140 208.27 140 206.429C140 184.1 157.461 166 179 166Z",
    rightPath:
      "M333.695 166C355.234 166 372.696 184.1 372.696 206.429C372.696 208.26 370.129 208.446 369.412 206.762C363.37 192.587 349.657 182.691 333.706 182.691C317.755 182.691 304.04 192.587 297.998 206.762C297.276 208.456 294.695 208.27 294.695 206.429C294.695 184.101 312.156 166 333.695 166Z",
    sparkleOpacity: 0,
    sparkleScale: 0.6,
  },
};

const useMorphVariant = <T,>(
  variant: T,
  transition: Required<MascotTransition>,
) => {
  const progress = useMotionValue(1);
  const previousVariant = useRef(variant);
  const [fromVariant, setFromVariant] = useState(variant);
  const [toVariant, setToVariant] = useState(variant);

  useEffect(() => {
    if (variant === previousVariant.current) return;
    setFromVariant(previousVariant.current);
    setToVariant(variant);
    previousVariant.current = variant;
    progress.set(0);
    const controls = animate(progress, 1, transition);

    return () => controls.stop();
  }, [progress, transition, variant]);

  return { progress, fromVariant, toVariant };
};

type MascotEyesProps = {
  variant: MascotEyesVariant;
  transition: Required<MascotTransition>;
};

const MascotEyes = ({ variant, transition }: MascotEyesProps) => {
  const { progress, fromVariant, toVariant } = useMorphVariant(
    variant,
    transition,
  );
  const from = EYE_VARIANTS[fromVariant];
  const to = EYE_VARIANTS[toVariant];

  const leftInterpolator = useMemo(
    () => interpolate(from.leftPath, to.leftPath, { maxSegmentLength: 2 }),
    [from.leftPath, to.leftPath],
  );

  const rightInterpolator = useMemo(
    () => interpolate(from.rightPath, to.rightPath, { maxSegmentLength: 2 }),
    [from.rightPath, to.rightPath],
  );

  const leftD = useTransform(progress, (t) => leftInterpolator(t));
  const rightD = useTransform(progress, (t) => rightInterpolator(t));
  const sparkleOpacity = useTransform(
    progress,
    [0, 1],
    [from.sparkleOpacity, to.sparkleOpacity],
  );
  const sparkleScale = useTransform(
    progress,
    [0, 1],
    [from.sparkleScale, to.sparkleScale],
  );

  return (
    <g>
      {/* Left */}
      <motion.path d={leftD} fill="#222222" />
      <motion.g
        style={{
          opacity: sparkleOpacity,
          transformOrigin: "180.918px 195.03px",
          scale: sparkleScale,
        }}
      >
        <ellipse
          cx="180.918"
          cy="195.03"
          rx="15.0784"
          ry="16.1965"
          fill="white"
        />
      </motion.g>

      {/* Right */}
      <motion.path d={rightD} fill="#222222" />
      <motion.g
        style={{
          opacity: sparkleOpacity,
          transformOrigin: "314.262px 195.03px",
          scale: sparkleScale,
        }}
      >
        <ellipse
          cx="314.262"
          cy="195.03"
          rx="15.0784"
          ry="16.1965"
          fill="white"
        />
      </motion.g>
    </g>
  );
};

const MascotBlush = () => {
  return (
    <g>
      {/* Left */}
      <path
        d="M150.638 253.151C151.067 253.222 151.491 253.297 151.907 253.377L140.975 272.211C140.509 272.208 140.047 272.198 139.589 272.185L150.638 253.151ZM153.615 253.743C154.031 253.84 154.438 253.942 154.838 254.049L144.335 272.142C143.862 272.164 143.385 272.183 142.904 272.194L153.615 253.743ZM147.558 252.735C147.999 252.782 148.436 252.834 148.868 252.89L137.717 272.103C137.264 272.075 136.816 272.041 136.372 272.003L147.558 252.735ZM156.479 254.528C156.88 254.656 157.271 254.788 157.65 254.925L147.814 271.87C147.325 271.924 146.829 271.973 146.328 272.015L156.479 254.528ZM144.377 252.494C144.832 252.516 145.283 252.545 145.729 252.577L134.561 271.817C134.121 271.765 133.687 271.709 133.259 271.647L144.377 252.494ZM159.214 255.539C159.596 255.704 159.965 255.874 160.319 256.049L151.441 271.342C150.933 271.435 150.415 271.524 149.889 271.603L159.214 255.539ZM142.493 252.431L131.503 271.363C131.077 271.287 130.657 271.204 130.244 271.118L141.098 252.423C141.203 252.422 141.309 252.42 141.415 252.42C141.776 252.42 142.136 252.424 142.493 252.431ZM161.775 256.848C162.134 257.066 162.471 257.29 162.786 257.521L155.271 270.468C154.737 270.617 154.188 270.756 153.625 270.888L161.775 256.848ZM128.551 270.73C128.138 270.627 127.734 270.518 127.339 270.404L137.712 252.533C138.188 252.504 138.669 252.481 139.154 252.463L128.551 270.73ZM125.713 269.896C125.316 269.761 124.931 269.62 124.557 269.476L134.203 252.857C134.695 252.796 135.194 252.743 135.698 252.694L125.713 269.896ZM164.071 258.612C164.389 258.931 164.668 259.257 164.904 259.592L159.447 268.994C158.871 269.249 158.262 269.49 157.624 269.72L164.071 258.612ZM123.014 268.826C122.636 268.651 122.274 268.47 121.927 268.284L130.535 253.455C131.048 253.352 131.571 253.254 132.104 253.165L123.014 268.826ZM165.747 261.447C165.809 261.734 165.843 262.024 165.843 262.317C165.843 264.215 164.523 265.988 162.237 267.494L165.747 261.447ZM120.501 267.434C120.15 267.199 119.823 266.956 119.521 266.709L126.646 254.435C127.187 254.268 127.746 254.112 128.32 253.964L120.501 267.434ZM118.293 265.517C117.989 265.156 117.736 264.785 117.535 264.407L122.337 256.137C122.928 255.837 123.562 255.553 124.233 255.283L118.293 265.517ZM117 262.023C117.099 260.647 117.891 259.341 119.241 258.161L117 262.023Z"
        fill="#FF6200"
      />
      {/* Right */}
      <path
        d="M379.216 253.149C379.645 253.22 380.069 253.295 380.485 253.375L369.552 272.21C369.087 272.207 368.625 272.197 368.166 272.184L379.216 253.149ZM382.193 253.741C382.609 253.838 383.017 253.94 383.417 254.047L372.912 272.143C372.439 272.165 371.962 272.184 371.48 272.195L382.193 253.741ZM376.136 252.734C376.578 252.781 377.014 252.833 377.446 252.889L366.294 272.102C365.842 272.074 365.394 272.04 364.95 272.002L376.136 252.734ZM385.059 254.525C385.459 254.653 385.85 254.784 386.229 254.921L376.391 271.871C375.901 271.925 375.406 271.974 374.905 272.016L385.059 254.525ZM372.955 252.494C373.41 252.516 373.861 252.544 374.308 252.576L363.139 271.816C362.699 271.764 362.265 271.708 361.836 271.646L372.955 252.494ZM387.794 255.535C388.176 255.7 388.545 255.869 388.898 256.044L380.018 271.345C379.509 271.437 378.991 271.526 378.465 271.604L387.794 255.535ZM371.07 252.431L360.082 271.361C359.656 271.284 359.236 271.203 358.823 271.116L369.675 252.423C369.784 252.422 369.894 252.42 370.004 252.42C370.361 252.42 370.717 252.425 371.07 252.431ZM390.355 256.843C390.714 257.061 391.051 257.285 391.366 257.515L383.845 270.472C383.312 270.621 382.763 270.759 382.2 270.891L390.355 256.843ZM357.129 270.728C356.717 270.624 356.313 270.515 355.917 270.401L366.289 252.533C366.765 252.504 367.246 252.481 367.731 252.463L357.129 270.728ZM354.292 269.894C353.895 269.758 353.51 269.618 353.136 269.473L362.779 252.859C363.271 252.798 363.771 252.745 364.275 252.696L354.292 269.894ZM392.653 258.605C392.971 258.924 393.25 259.25 393.486 259.584L388.021 269C387.445 269.255 386.836 269.496 386.198 269.725L392.653 258.605ZM351.593 268.822C351.215 268.647 350.854 268.465 350.507 268.279L359.11 253.458C359.624 253.354 360.147 253.256 360.68 253.167L351.593 268.822ZM394.332 261.434C394.396 261.725 394.431 262.019 394.431 262.317C394.431 264.22 393.105 265.997 390.808 267.506L394.332 261.434ZM349.081 267.428C348.73 267.193 348.404 266.951 348.102 266.703L355.222 254.438C355.762 254.272 356.321 254.115 356.896 253.967L349.081 267.428ZM346.874 265.509C346.57 265.147 346.318 264.776 346.118 264.397L350.909 256.145C351.501 255.844 352.134 255.558 352.807 255.288L346.874 265.509ZM345.59 262C345.696 260.639 346.479 259.348 347.808 258.18L345.59 262Z"
        fill="#FF6200"
      />
    </g>
  );
};

const MOUTH_VARIANTS: Record<
  MascotMouthVariant,
  {
    mouthPath: string;
    tonguePath: string;
    outlineStrokeWidth: number;
    tongueStrokeWidth: number;
  }
> = {
  neutral: {
    mouthPath:
      "M254.645 282.896C255.122 282.904 255.495 282.914 255.745 282.922C255.87 282.926 255.964 282.93 256.025 282.933C256.056 282.934 256.078 282.935 256.092 282.936C256.099 282.936 256.104 282.936 256.106 282.937H256.107L256.212 282.941L256.316 282.938H256.333C256.347 282.938 256.37 282.938 256.401 282.938C256.464 282.936 256.561 282.935 256.688 282.934C256.941 282.931 257.319 282.929 257.804 282.933C258.774 282.939 260.167 282.966 261.842 283.046C265.203 283.207 269.643 283.582 274.049 284.427C278.502 285.281 282.682 286.571 285.678 288.438C288.616 290.268 290.051 292.388 290.051 295.001C290.051 305.509 286.016 313.338 279.9 318.581C273.729 323.872 265.257 326.676 256.247 326.676C247.175 326.676 238.832 324.113 232.808 318.986C226.852 313.919 222.887 306.117 222.887 295.001C222.887 292.089 224.382 289.872 227.225 288.045C230.156 286.16 234.255 284.899 238.641 284.102C242.977 283.313 247.351 283.019 250.665 282.925C252.315 282.878 253.689 282.882 254.645 282.896Z",
    tonguePath: buildEllipsePath(256.259, 314.786, 26.7435, 9.01277),
    outlineStrokeWidth: 5.77266,
    tongueStrokeWidth: 0,
  },
  open: {
    mouthPath:
      "M256 281C267.184 281 277 291.307 277 305C277 318.693 267.184 329 256 329C244.816 329 235 318.693 235 305C235 291.307 244.816 281 256 281Z",
    tonguePath:
      "M256 281C267.184 281 277 291.307 277 305C277 318.693 267.184 329 256 329C244.816 329 235 318.693 235 305C235 291.307 244.816 281 256 281Z",
    outlineStrokeWidth: 8,
    tongueStrokeWidth: 0,
  },
};

type MascotMouthProps = {
  variant: MascotMouthVariant;
  transition: Required<MascotTransition>;
};

const MascotMouth = ({ variant, transition }: MascotMouthProps) => {
  const { progress, fromVariant, toVariant } = useMorphVariant(
    variant,
    transition,
  );
  const from = MOUTH_VARIANTS[fromVariant];
  const to = MOUTH_VARIANTS[toVariant];

  const mouthInterpolator = useMemo(
    () => interpolate(from.mouthPath, to.mouthPath, { maxSegmentLength: 2 }),
    [from.mouthPath, to.mouthPath],
  );

  const tongueInterpolator = useMemo(
    () => interpolate(from.tonguePath, to.tonguePath, { maxSegmentLength: 2 }),
    [from.tonguePath, to.tonguePath],
  );

  const mouthD = useTransform(progress, (t) => mouthInterpolator(t));
  const tongueD = useTransform(progress, (t) => tongueInterpolator(t));
  const mouthOutlineStrokeWidth = useTransform(
    progress,
    [0, 1],
    [from.outlineStrokeWidth, to.outlineStrokeWidth],
  );
  const tongueStrokeWidth = useTransform(
    progress,
    [0, 1],
    [from.tongueStrokeWidth, to.tongueStrokeWidth],
  );

  return (
    <g>
      <motion.path d={mouthD} fill="#222222" />

      {/* Tongue (morphs) */}
      <motion.path
        d={tongueD}
        fill="#FF6200"
        stroke="#222222"
        style={{ strokeWidth: tongueStrokeWidth }}
      />

      {/* Outline (always visible) */}
      <motion.path
        d={mouthD}
        fill="none"
        stroke="#222222"
        style={{ strokeWidth: mouthOutlineStrokeWidth }}
      />
    </g>
  );
};

const MascotLegs = () => {
  return (
    <g>
      {/* Left */}
      <ellipse
        cx="161.668"
        cy="449.684"
        rx="49.7342"
        ry="53.5599"
        transform="rotate(30.231 161.668 449.684)"
        fill="#222222"
      />
      {/* Right */}
      <ellipse
        cx="49.7342"
        cy="53.5599"
        rx="49.7342"
        ry="53.5599"
        transform="matrix(-0.864002 0.503488 0.503488 0.864002 366.469 378.367)"
        fill="#222222"
      />
    </g>
  );
};

const MascotEar = () => {
  return (
    <g>
      {/* Left */}
      <path
        d="M89.5569 10.263C91.0017 9.8295 92.4816 9.52291 93.9798 9.34666C95.9378 9.1163 97.9157 9.10992 99.8751 9.32764L102.43 9.61154L107.322 10.8346L114.661 13.2808L117.949 14.7105C120.442 15.7943 122.852 17.0594 125.16 18.4954L135.453 24.8999L148.601 34.3787L160.832 45.0806L172.451 57.3113L182.541 69.542L187.128 75.3516L191.409 82.0785L149.824 90.64L148.295 88.1939L147.927 87.6781C147.155 86.5967 146.468 85.4565 145.874 84.2679C145.247 83.0143 144.518 81.8147 143.693 80.6812L142.486 79.0209L139.122 74.7401L136.065 70.7651L132.395 66.4844L128.726 62.8152L125.057 59.4517L121.694 56.394L118.024 53.6421L116.169 52.2508C114.962 51.3456 113.676 50.5508 112.327 49.8761L109.205 48.3149C107.766 47.5955 106.179 47.221 104.571 47.221C102.965 47.221 101.385 47.6244 99.9756 48.393C99.1705 48.8322 98.4317 49.3849 97.7832 50.0334L97.3114 50.5052C96.4468 51.3698 95.6971 52.3422 95.081 53.3984L91.7283 59.1459L88.3649 66.4844L85.6129 75.6574L82.861 84.8304L82.0966 89.417L81.3322 94.0035L80.1091 103.177L79.4976 107.763L78.886 112.655L78.5803 117.242V122.134V126.109L34.8555 173.197V162.324L35.467 151.488L37.6074 130.084L41.2766 108.68L45.5573 87.8881L52.8958 65.5671L61.4573 43.5518L66.6553 33.4614L69.8143 28.1411C71.5772 25.172 73.6257 22.3822 75.9307 19.8112L79.0945 16.2824C79.9712 15.3044 80.9678 14.4409 82.0607 13.7124C82.7968 13.2217 83.5732 12.7944 84.3816 12.4351L85.9187 11.7519L88.6706 10.5288L89.5569 10.263Z"
        fill="#222222"
      />
      {/* Right */}
      <path
        d="M422.908 10.263C421.463 9.8295 419.983 9.52291 418.485 9.34666C416.527 9.1163 414.549 9.10992 412.59 9.32764L410.035 9.61154L405.142 10.8346L397.804 13.2808L394.516 14.7105C392.023 15.7943 389.613 17.0594 387.305 18.4954L377.012 24.8999L363.864 34.3787L351.633 45.0806L340.014 57.3113L329.923 69.542L325.337 75.3516L321.056 82.0785L362.641 90.64L364.169 88.1939L364.538 87.6781C365.31 86.5967 365.997 85.4565 366.591 84.2679C367.218 83.0143 367.947 81.8147 368.772 80.6812L369.979 79.0209L373.343 74.7401L376.4 70.7651L380.069 66.4844L383.739 62.8152L387.408 59.4517L390.771 56.394L394.441 53.6421L396.296 52.2508C397.503 51.3456 398.789 50.5508 400.138 49.8761L403.26 48.3149C404.699 47.5955 406.286 47.221 407.894 47.221C409.499 47.221 411.08 47.6244 412.489 48.393C413.294 48.8322 414.033 49.3849 414.682 50.0334L415.153 50.5052C416.018 51.3698 416.768 52.3422 417.384 53.3984L420.737 59.1459L424.1 66.4844L426.852 75.6574L429.604 84.8304L430.368 89.417L431.133 94.0035L432.356 103.177L432.967 107.763L433.579 112.655L433.885 117.242V122.134V126.109L477.609 173.197V162.324L476.998 151.488L474.857 130.084L471.188 108.68L466.907 87.8881L459.569 65.5671L451.008 43.5518L445.81 33.4614L442.651 28.1411C440.888 25.172 438.839 22.3822 436.534 19.8112L433.37 16.2824C432.494 15.3044 431.497 14.4409 430.404 13.7124C429.668 13.2217 428.892 12.7944 428.083 12.4351L426.546 11.7519L423.794 10.5288L422.908 10.263Z"
        fill="#222222"
      />
    </g>
  );
};

const MascotBody = () => {
  return (
    <g>
      {/* Body */}
      <path
        d="M256.233 68.9297C397.747 68.9297 512.467 130.083 512.467 302.842C512.467 354.087 512.467 469.534 258.094 470.094C257.474 470.096 256.854 470.097 256.233 470.097C255.613 470.097 254.992 470.096 254.373 470.094C0 469.534 0 354.086 0 302.842C0 130.083 114.72 68.9297 256.233 68.9297Z"
        fill="#222222"
      />
      {/* cut-of */}
      <path
        d="M256.845 116.02C443.669 116.02 454.983 203.309 454.983 267.986C454.983 268.744 454.971 269.496 454.946 270.241C452.901 333.073 364.991 350.238 256.845 350.238C256.641 350.238 256.437 350.237 256.234 350.237C256.03 350.237 255.826 350.238 255.622 350.238C146.194 350.238 57.4844 332.664 57.4844 267.986C57.4844 203.309 68.7978 116.02 255.622 116.02C255.826 116.02 256.03 116.021 256.234 116.021C256.437 116.021 256.641 116.02 256.845 116.02Z"
        fill="white"
      />
    </g>
  );
};

export { MascotProvider } from "./context";
export { useMascot } from "./hook";
export type {
  MascotBehavior,
  MascotExpression,
  MascotExpressionMap,
  MascotEyesVariant,
  MascotInteraction,
  MascotMouthVariant,
  MascotParticlesConfig,
  MascotProps,
  MascotTransition,
} from "./types";
