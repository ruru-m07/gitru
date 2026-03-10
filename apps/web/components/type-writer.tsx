import { cn } from "@/lib/cn";
import { useCallback, useEffect, useRef, useState } from "react";

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
  /** Make this segment an input field. The text value is used as placeholder */
  input?: boolean;
  /** Input field ID for accessing values later */
  inputId?: string;
  /** Called when input value changes */
  onInputChange?: (value: string) => void;
  /** Called when Enter is pressed on this input */
  onInputSubmit?: (value: string) => void;
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
  /** Callback with all input values when all inputs are submitted */
  onAllInputsSubmit?: (values: Record<string, string>) => void;
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
  onAllInputsSubmit,
  className,
}: TypeWriterProps) => {
  const [displayedSegments, setDisplayedSegments] = useState<
    { text: string; className?: string; isInput?: boolean; inputId?: string }[]
  >([]);
  const [currentSegmentIndex, setCurrentSegmentIndex] = useState(0);
  const [currentCharIndex, setCurrentCharIndex] = useState(0);
  const [isTyping, setIsTyping] = useState(false);
  const [showBlinkingCursor, setShowBlinkingCursor] = useState(true);
  const [isComplete, setIsComplete] = useState(false);
  const [cursorBlink, setCursorBlink] = useState<boolean | "hide">(true);
  const [cursorVariant, setCursorVariant] =
    useState<CursorVariant>(defaultCursor);

  // Input state
  const [activeInputIndex, setActiveInputIndex] = useState<number | null>(null);
  const [inputValues, setInputValues] = useState<Record<string, string>>({});
  const [_waitingForInput, setWaitingForInput] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

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

    // Handle input segments - pause typing and wait for user input
    if (currentSegment.input) {
      const timer = setTimeout(() => {
        const inputId =
          currentSegment.inputId || `input-${currentSegmentIndex}`;
        setDisplayedSegments((prev) => {
          const newSegments = [...prev];
          newSegments.push({
            text: "",
            className: currentSegment.className,
            isInput: true,
            inputId,
          });
          return newSegments;
        });
        setActiveInputIndex(displayedSegments.length);
        setWaitingForInput(true);
        setIsTyping(false);
        // Focus the input after it renders
        setTimeout(() => inputRef.current?.focus(), 50);
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

  // Handle input change
  const handleInputChange = (value: string, inputId: string) => {
    // If user types a closing quote, submit the input instead
    if (value.endsWith('"')) {
      const cleanValue = value.slice(0, -1); // Remove the trailing "
      setInputValues((prev) => ({ ...prev, [inputId]: cleanValue }));
      setDisplayedSegments((prev) => {
        const newSegments = [...prev];
        if (activeInputIndex !== null && newSegments[activeInputIndex]) {
          newSegments[activeInputIndex] = {
            ...newSegments[activeInputIndex],
            text: cleanValue,
          };
        }
        return newSegments;
      });
      handleInputSubmit(inputId);
      return;
    }

    setInputValues((prev) => ({ ...prev, [inputId]: value }));
    // Update the displayed segment text
    setDisplayedSegments((prev) => {
      const newSegments = [...prev];
      if (activeInputIndex !== null && newSegments[activeInputIndex]) {
        newSegments[activeInputIndex] = {
          ...newSegments[activeInputIndex],
          text: value,
        };
      }
      return newSegments;
    });
    // Call the segment's onChange if provided
    const currentSegment = segments[currentSegmentIndex];
    currentSegment?.onInputChange?.(value);
  };

  // Handle input submit (Enter key)
  const handleInputSubmit = (inputId: string) => {
    const value = inputValues[inputId] || "";
    const currentSegment = segments[currentSegmentIndex];
    currentSegment?.onInputSubmit?.(value);

    // Update input values with current value
    const updatedInputValues = { ...inputValues, [inputId]: value };
    setInputValues(updatedInputValues);

    // Move to next segment
    setWaitingForInput(false);
    setActiveInputIndex(null);
    setCurrentSegmentIndex((prev) => prev + 1);
    setCurrentCharIndex(0);
    setIsTyping(true);

    // Check if all inputs are done
    const inputSegments = segments.filter((s) => s.input);
    const allInputIds = inputSegments.map(
      (s) => s.inputId || `input-${segments.indexOf(s)}`,
    );
    const allFilled = allInputIds.every(
      (id) =>
        updatedInputValues[id] !== undefined && updatedInputValues[id] !== "",
    );
    if (allFilled) {
      // Delay slightly to let the last segment render
      setTimeout(() => {
        onAllInputsSubmit?.(updatedInputValues);
      }, 100);
    }
  };

  return (
    <span className={className}>
      {displayedSegments.map((segment, index) => (
        <span key={index} className={segment.className}>
          {segment.isInput && activeInputIndex === index ? (
            <>
              <span className="text-primary">{segment.text}</span>
              <input
                ref={inputRef}
                type="text"
                value={inputValues[segment.inputId || ""] || ""}
                onChange={(e) =>
                  handleInputChange(e.target.value, segment.inputId || "")
                }
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handleInputSubmit(segment.inputId || "");
                  }
                }}
                className="bg-transparent border-none outline-none w-0 h-0 absolute opacity-0"
                autoFocus
              />
            </>
          ) : segment.isInput ? (
            <span className="text-primary">{segment.text}</span>
          ) : (
            segment.text
          )}
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
