import { cn } from "@gitru/ui/lib/utils";
import { useCallback, useEffect, useState } from "react";

export type CursorVariant = "primary" | "default";

export type TypeSegment = {
  text: string;
  className?: string;
  /** Delay before typing this segment (ms) */
  delay?: number;
  /** Typing speed for this segment (ms per character) */
  speed?: number;
  /** Show text instantly without typing animation */
  instant?: boolean;
  /** Control cursor during this segment: true = blink, false = solid, 'hide' = hidden */
  blink?: boolean | "hide";
  /** Cursor variant for this segment: 'primary' (bg-primary) or 'default' (bg-current) */
  cursor?: CursorVariant;
};

type TypeWriterProps = {
  segments: TypeSegment[];
  /** Default typing speed (ms per character) */
  defaultSpeed?: number;
  /** Delay before starting (ms) */
  startDelay?: number;
  /** Show cursor */
  showCursor?: boolean;
  /** Default cursor variant: 'primary' (bg-primary) or 'default' (bg-current) */
  defaultCursor?: CursorVariant;
  /** Cursor blink speed (ms) */
  cursorBlinkSpeed?: number;
  /** Callback when typing is complete */
  onComplete?: () => void;
  /** Loop the animation */
  loop?: boolean;
  /** Delay before looping (ms) */
  loopDelay?: number;
  className?: string;
};

export const TypeWriter = ({
  segments,
  defaultSpeed = 50,
  startDelay = 0,
  showCursor = true,
  defaultCursor = "default",
  cursorBlinkSpeed = 530,
  onComplete,
  loop = false,
  loopDelay = 2000,
  className,
}: TypeWriterProps) => {
  const [displayedSegments, setDisplayedSegments] = useState<
    { text: string; className?: string }[]
  >([]);
  const [currentSegmentIndex, setCurrentSegmentIndex] = useState(0);
  const [currentCharIndex, setCurrentCharIndex] = useState(0);
  const [isTyping, setIsTyping] = useState(false);
  const [showBlinkingCursor, setShowBlinkingCursor] = useState(true);
  const [isComplete, setIsComplete] = useState(false);
  const [cursorBlink, setCursorBlink] = useState<boolean | "hide">(true);
  const [cursorVariant, setCursorVariant] =
    useState<CursorVariant>(defaultCursor);

  const reset = useCallback(() => {
    setDisplayedSegments([]);
    setCurrentSegmentIndex(0);
    setCurrentCharIndex(0);
    setIsTyping(false);
    setIsComplete(false);
  }, []);

  // Start typing after initial delay
  useEffect(() => {
    const timer = setTimeout(() => {
      setIsTyping(true);
    }, startDelay);
    return () => clearTimeout(timer);
  }, [startDelay]);

  // Cursor blinking effect
  useEffect(() => {
    if (!showCursor || cursorBlink === "hide") return;
    if (cursorBlink === false) {
      setShowBlinkingCursor(true); // solid cursor
      return;
    }
    const interval = setInterval(() => {
      setShowBlinkingCursor((prev) => !prev);
    }, cursorBlinkSpeed);
    return () => clearInterval(interval);
  }, [showCursor, cursorBlinkSpeed, cursorBlink]);

  // Main typing effect
  useEffect(() => {
    if (!isTyping || currentSegmentIndex >= segments.length) {
      if (currentSegmentIndex >= segments.length && !isComplete) {
        setIsComplete(true);
        onComplete?.();

        if (loop) {
          const timer = setTimeout(() => {
            reset();
            setTimeout(() => setIsTyping(true), startDelay);
          }, loopDelay);
          return () => clearTimeout(timer);
        }
      }
      return;
    }

    const currentSegment = segments[currentSegmentIndex];
    const speed = currentSegment.speed ?? defaultSpeed;
    const delay = currentCharIndex === 0 ? (currentSegment.delay ?? 0) : 0;

    // Update cursor blink mode for current segment
    if (currentSegment.blink !== undefined) {
      setCursorBlink(currentSegment.blink);
    } else {
      setCursorBlink(true); // default to blinking
    }

    // Update cursor variant for current segment
    if (currentSegment.cursor !== undefined) {
      setCursorVariant(currentSegment.cursor);
    } else {
      setCursorVariant(defaultCursor);
    }

    // Handle instant segments - show all text at once
    if (currentSegment.instant) {
      const timer = setTimeout(() => {
        setDisplayedSegments((prev) => {
          const newSegments = [...prev];
          newSegments.push({
            text: currentSegment.text,
            className: currentSegment.className,
          });
          return newSegments;
        });
        setCurrentSegmentIndex((prev) => prev + 1);
        setCurrentCharIndex(0);
      }, delay);
      return () => clearTimeout(timer);
    }

    const timer = setTimeout(() => {
      if (currentCharIndex < currentSegment.text.length) {
        setDisplayedSegments((prev) => {
          const newSegments = [...prev];
          if (newSegments.length <= currentSegmentIndex) {
            newSegments.push({
              text: currentSegment.text[currentCharIndex],
              className: currentSegment.className,
            });
          } else {
            newSegments[currentSegmentIndex] = {
              text:
                newSegments[currentSegmentIndex].text +
                currentSegment.text[currentCharIndex],
              className: currentSegment.className,
            };
          }
          return newSegments;
        });
        setCurrentCharIndex((prev) => prev + 1);
      } else {
        setCurrentSegmentIndex((prev) => prev + 1);
        setCurrentCharIndex(0);
      }
    }, delay + speed);

    return () => clearTimeout(timer);
  }, [
    isTyping,
    currentSegmentIndex,
    currentCharIndex,
    segments,
    defaultSpeed,
    defaultCursor,
    onComplete,
    loop,
    loopDelay,
    reset,
    startDelay,
    isComplete,
  ]);

  return (
    <span className={className}>
      {displayedSegments.map((segment, index) => (
        <span key={index} className={segment.className}>
          {segment.text}
        </span>
      ))}
      {showCursor && cursorBlink !== "hide" && (
        <span
          className={cn(
            "h-full px-[5px] transition-opacity relative",
            cursorVariant === "primary" ? "bg-primary" : "bg-current",
            showBlinkingCursor ? "opacity-100" : "bg-transparent _opacity-0",
          )}
        >
          {showBlinkingCursor ? (
            ""
          ) : (
            <span
              className={cn(
                "absolute w-full left-0 right-0 bottom-0 h-1",
                cursorVariant === "primary" ? "bg-primary" : "bg-current",
              )}
            />
          )}
        </span>
      )}
    </span>
  );
};

// Preset class helpers for common styles
export const typeStyles = {
  primary: "text-primary",
  muted: "text-muted-foreground",
  red: "text-red-500",
  yellow: "text-amber-500",
  green: "text-green-500",
  blue: "text-blue-500",
  purple: "text-purple-500",
  orange: "text-orange-500",
  bold: "font-bold",
  italic: "italic",
} as const;

export default TypeWriter;
